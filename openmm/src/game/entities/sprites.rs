//! Sprite loading and resolution for world entities.
//!
//! Resolves DSFT group IDs to sprite roots, loads directional sprite frames,
//! and provides the animation update system. Reusable by any entity type
//! (NPCs, monsters, decorations with animations).

use std::collections::HashMap;

use bevy::prelude::*;
use image::{DynamicImage, GenericImageView, RgbaImage};

use lod::LodManager;

use crate::game::entities::AnimationState;
use crate::game::player::PlayerCamera;

/// Cache for loaded sprite materials to avoid duplicate texture loading.
#[derive(Resource, Default, Clone)]
pub struct SpriteCache {
    /// Maps "root_name + frame_letter + direction" to material handle
    materials: HashMap<String, Handle<StandardMaterial>>,
    /// Maps cache key to (width, height)
    dimensions: HashMap<String, (f32, f32)>,
}

impl SpriteCache {
    /// Pre-decode a list of (sprite_root, variant) pairs into the cache.
    /// Call during loading screen to avoid decoding during gameplay.
    pub fn preload(
        &mut self,
        roots: &[(&str, u8)],
        lod_manager: &LodManager,
        images: &mut Assets<Image>,
        materials: &mut Assets<StandardMaterial>,
    ) {
        for &(root, variant) in roots {
            load_sprite_frames(root, lod_manager, images, materials, &mut Some(self), variant, 0, 0);
        }
    }
}

/// Build a cache key for a sprite root with optional variant and minimum size.
/// Format: "root" or "root@v2" or "root@64x128" or "root@64x128@v2"
fn cache_key(root: &str, variant: u8, min_w: u32, min_h: u32) -> String {
    let has_size = min_w > 0 || min_h > 0;
    let has_variant = variant > 1;
    match (has_size, has_variant) {
        (false, false) => root.to_string(),
        (false, true) => format!("{}@v{}", root, variant),
        (true, false) => format!("{}@{}x{}", root, min_w, min_h),
        (true, true) => format!("{}@{}x{}@v{}", root, min_w, min_h, variant),
    }
}

/// Preloaded sprite frames for an entity.
/// `states[state_idx][frame_idx]` = array of 5 material handles (directions 0-4).
#[derive(Component)]
pub struct SpriteSheet {
    /// states[0]=standing, states[1]=walking (if available)
    pub states: Vec<Vec<[Handle<StandardMaterial>; 5]>>,
    /// Width and height per state (sprites can differ between standing/walking).
    pub state_dimensions: Vec<(f32, f32)>,
    pub current_frame: usize,
    pub current_state: usize,
    pub frame_timer: f32,
    pub frame_duration: f32,
    /// Last applied (state, frame, direction) -- skip material swap when unchanged.
    last_applied: (usize, usize, usize),
}

impl SpriteSheet {
    pub fn new(
        states: Vec<Vec<[Handle<StandardMaterial>; 5]>>,
        state_dimensions: Vec<(f32, f32)>,
    ) -> Self {
        Self {
            states,
            state_dimensions,
            current_frame: 0,
            current_state: 0,
            frame_timer: 0.0,
            frame_duration: 0.15,
            last_applied: (usize::MAX, usize::MAX, usize::MAX),
        }
    }
}

/// Load a complete entity's sprite set (standing + walking) using the cache.
/// Returns (states, quad_width, quad_height) where the quad uses the max
/// dimensions across both states so neither gets stretched.
pub fn load_entity_sprites(
    standing_root: &str,
    walking_root: &str,
    lod_manager: &LodManager,
    images: &mut Assets<Image>,
    materials: &mut Assets<StandardMaterial>,
    cache: &mut Option<&mut SpriteCache>,
    variant: u8,
) -> (Vec<Vec<[Handle<StandardMaterial>; 5]>>, f32, f32) {
    // Load both at native size first to determine max dimensions
    let (standing_native, sw, sh) = load_sprite_frames(
        standing_root, lod_manager, images, materials, cache, variant, 0, 0);
    let (walking_native, ww, wh) = load_sprite_frames(
        walking_root, lod_manager, images, materials, cache, variant, 0, 0);

    if standing_native.is_empty() {
        return (Vec::new(), 0.0, 0.0);
    }

    let qw = sw.max(ww);
    let qh = sh.max(wh);
    let target_w = qw as u32;
    let target_h = qh as u32;

    // Reload with padding to uniform size (skip reload if already at max)
    let standing = if (sw as u32) < target_w || (sh as u32) < target_h {
        load_sprite_frames(standing_root, lod_manager, images, materials, cache, variant, target_w, target_h).0
    } else {
        standing_native
    };
    let walking = if !walking_native.is_empty() && ((ww as u32) < target_w || (wh as u32) < target_h) {
        load_sprite_frames(walking_root, lod_manager, images, materials, cache, variant, target_w, target_h).0
    } else {
        walking_native
    };

    let mut states = vec![standing];
    if !walking.is_empty() {
        states.push(walking);
    }

    (states, qw, qh)
}

/// Load sprite frames for a single animation (e.g. standing or walking).
///
/// `variant` controls tinting: 0/1 = none, 2 = blue, 3 = red.
/// `min_w`/`min_h` enforce minimum padding dimensions (used to pad standing
/// sprites to match walking sprite size). Pass 0 for no minimum.
pub fn load_sprite_frames(
    root: &str,
    lod_manager: &LodManager,
    images: &mut Assets<Image>,
    materials: &mut Assets<StandardMaterial>,
    cache: &mut Option<&mut SpriteCache>,
    variant: u8,
    min_w: u32,
    min_h: u32,
) -> (Vec<[Handle<StandardMaterial>; 5]>, f32, f32) {
    let root = root.trim_end_matches(|c: char| c.is_ascii_digit());
    let key = cache_key(root, variant, min_w, min_h);

    if let Some(c) = cache.as_ref() {
        if let Some(&(w, h)) = c.dimensions.get(&key) {
            let frames = rebuild_from_cache(&key, c);
            if !frames.is_empty() {
                return (frames, w, h);
            }
        }
    }

    // Try progressively shorter root names (e.g. "gobla" -> "gobl" -> "gob")
    let mut try_root = root;
    while try_root.len() >= 3 {
        let (frames, w, h) = decode_sprite_frames(
            try_root, lod_manager, images, materials, variant, min_w, min_h);
        if !frames.is_empty() {
            store_in_cache(&key, &frames, w, h, cache);
            return (frames, w, h);
        }
        try_root = &try_root[..try_root.len() - 1];
    }
    (Vec::new(), 0.0, 0.0)
}

fn store_in_cache(
    key: &str,
    frames: &[[Handle<StandardMaterial>; 5]],
    w: f32, h: f32,
    cache: &mut Option<&mut SpriteCache>,
) {
    if let Some(cache) = cache.as_mut() {
        cache.dimensions.insert(key.to_string(), (w, h));
        for (fi, dirs) in frames.iter().enumerate() {
            let frame_letter = (b'a' + fi as u8) as char;
            for (di, mat) in dirs.iter().enumerate() {
                let mat_key = format!("{}{}{}", key, frame_letter, di);
                cache.materials.insert(mat_key, mat.clone());
            }
        }
    }
}

fn rebuild_from_cache(
    key: &str,
    cache: &SpriteCache,
) -> Vec<[Handle<StandardMaterial>; 5]> {
    let mut frames = Vec::new();
    for fi in 0..6 {
        let frame_letter = (b'a' + fi) as char;
        let key0 = format!("{}{}0", key, frame_letter);
        if let Some(mat0) = cache.materials.get(&key0) {
            let mut dirs: [Handle<StandardMaterial>; 5] = Default::default();
            for di in 0..5 {
                let mat_key = format!("{}{}{}", key, frame_letter, di);
                dirs[di] = cache.materials.get(&mat_key).cloned().unwrap_or_else(|| mat0.clone());
            }
            frames.push(dirs);
        } else {
            break;
        }
    }
    frames
}

/// Decode sprite frames from the LOD, apply variant tinting, and pad to uniform size.
fn decode_sprite_frames(
    root: &str,
    lod_manager: &LodManager,
    images: &mut Assets<Image>,
    materials: &mut Assets<StandardMaterial>,
    variant: u8,
    min_w: u32,
    min_h: u32,
) -> (Vec<[Handle<StandardMaterial>; 5]>, f32, f32) {
    // First pass: collect all raw sprites and find max dimensions.
    let mut raw_sprites: Vec<Vec<Option<DynamicImage>>> = Vec::new();
    let mut max_w = min_w;
    let mut max_h = min_h;

    for frame_char in b'a'..=b'f' {
        let frame_letter = frame_char as char;
        let test0 = format!("{}{}0", root, frame_letter);
        let test_nodir = format!("{}{}", root, frame_letter);

        let has_frame = lod_manager.sprite(&test0).is_some()
            || lod_manager.sprite(&test_nodir).is_some();
        if !has_frame {
            break;
        }

        let mut dir_imgs: Vec<Option<DynamicImage>> = Vec::with_capacity(5);
        for dir in 0..5u8 {
            let name = format!("{}{}{}", root, frame_letter, dir);
            // For variant B/C, use the DSFT palette_id directly.
            // The variant encodes a palette offset from the base palette stored
            // in the sprite file header. But DSFT palette IDs don't always match
            // file header palette IDs, so we pass the exact palette.
            let img = if variant > 1 {
                let pal_offset = (variant - 1) as u16;
                load_sprite_with_palette_offset(lod_manager, &name, &test_nodir, pal_offset)
            } else {
                lod_manager.sprite(&name)
                    .or_else(|| lod_manager.sprite(&test_nodir))
            };
            if let Some(ref i) = img {
                max_w = max_w.max(i.width());
                max_h = max_h.max(i.height());
            }
            dir_imgs.push(img);
        }
        raw_sprites.push(dir_imgs);
    }

    if raw_sprites.is_empty() || max_w == 0 {
        return (Vec::new(), 0.0, 0.0);
    }

    // Second pass: tint, pad to uniform size, and create materials.
    let mut frames = Vec::new();
    for dir_imgs in raw_sprites {
        let mut dir_materials: [Handle<StandardMaterial>; 5] = Default::default();
        for (dir, img_opt) in dir_imgs.into_iter().enumerate() {
            if let Some(img) = img_opt {
                // Palette swap handles variant coloring — no tinting needed

                // Pad to uniform size: center horizontally, align bottom vertically
                let img = if img.width() != max_w || img.height() != max_h {
                    let mut padded = RgbaImage::new(max_w, max_h);
                    let x_off = (max_w - img.width()) / 2;
                    let y_off = max_h - img.height();
                    for py in 0..img.height() {
                        for px in 0..img.width() {
                            padded.put_pixel(px + x_off, py + y_off, img.get_pixel(px, py));
                        }
                    }
                    DynamicImage::ImageRgba8(padded)
                } else {
                    img
                };
                let bevy_img = crate::assets::dynamic_to_bevy_image(img);
                let tex = images.add(bevy_img);
                dir_materials[dir] = materials.add(StandardMaterial {
                    unlit: true,
                    base_color_texture: Some(tex),
                    alpha_mode: AlphaMode::Mask(0.5),
                    double_sided: true,
                    cull_mode: None,
                    perceptual_roughness: 1.0,
                    reflectance: 0.0,
                    ..default()
                });
            } else if dir > 0 {
                dir_materials[dir] = dir_materials[0].clone();
            }
        }
        frames.push(dir_materials);
    }

    (frames, max_w as f32, max_h as f32)
}

/// Load a sprite with a palette offset applied (for monster variant palette swaps).
/// Reads the base sprite's palette_id from its header, adds the offset, and decodes
/// with the variant palette. Falls back to normal sprite() if palette not found.
fn load_sprite_with_palette_offset(
    lod_manager: &LodManager,
    name: &str,
    fallback: &str,
    palette_offset: u16,
) -> Option<DynamicImage> {
    // Try the primary name first, then the fallback (no-direction variant)
    let sprite_name = if lod_manager.try_get_bytes(format!("sprites/{}", name.to_lowercase())).is_ok() {
        name
    } else if lod_manager.try_get_bytes(format!("sprites/{}", fallback.to_lowercase())).is_ok() {
        fallback
    } else {
        return None;
    };

    // Read the base palette_id from the sprite header (offset 20, u16 LE)
    let sprite_data = lod_manager.try_get_bytes(format!("sprites/{}", sprite_name.to_lowercase())).ok()?;
    if sprite_data.len() < 22 { return None; }
    let base_palette_id = u16::from_le_bytes([sprite_data[20], sprite_data[21]]);
    let variant_palette_id = base_palette_id + palette_offset;

    // Try with variant palette, fall back to normal decode
    lod_manager.sprite_with_palette(sprite_name, variant_palette_id)
        .or_else(|| lod_manager.sprite(sprite_name))
}

/// Update sprite sheets based on camera angle, entity facing, and animation state.
/// Only processes visible entities within draw distance. Skips material swap when
/// the displayed (state, frame, direction) hasn't changed.
pub fn update_sprite_sheets(
    time: Res<Time>,
    cfg: Res<crate::config::GameConfig>,
    camera_query: Query<&GlobalTransform, With<PlayerCamera>>,
    mut query: Query<(
        &mut SpriteSheet,
        &mut MeshMaterial3d<StandardMaterial>,
        &mut Transform,
        &GlobalTransform,
        &AnimationState,
        &super::actor::Actor,
        &Visibility,
    )>,
) {
    let Ok(camera_gt) = camera_query.single() else {
        return;
    };
    let cam_pos = camera_gt.translation();
    let dt = time.delta_secs();

    for (mut sprites, mut mat_handle, mut transform, global_transform, anim_state, actor, vis) in
        query.iter_mut()
    {
        if *vis == Visibility::Hidden {
            continue;
        }
        let actor_pos = global_transform.translation();

        // Skip animation for entities far from camera
        if cam_pos.distance_squared(actor_pos) > cfg.draw_distance * cfg.draw_distance {
            continue;
        }

        let state_idx = match anim_state {
            AnimationState::Walking
                if sprites.states.len() > 1 && !sprites.states[1].is_empty() => 1,
            _ => 0,
        };
        let frame_count = sprites.states[state_idx].len();
        if frame_count == 0 {
            continue;
        }

        // Reset frame when state changes
        if state_idx != sprites.current_state {
            sprites.current_state = state_idx;
            sprites.current_frame = 0;
            sprites.frame_timer = 0.0;
        }

        // Advance animation timer
        sprites.frame_timer += dt;
        if sprites.frame_timer >= sprites.frame_duration {
            sprites.frame_timer -= sprites.frame_duration;
            sprites.current_frame = (sprites.current_frame + 1) % frame_count;
        }

        // Pick directional frame based on camera angle relative to actor facing.
        // MM6 sprites have 5 pre-rendered views (0=front, 1-4=rotations).
        // Octants 5-7 mirror views 3-1 via negative X scale.
        let dir_to_camera = cam_pos - actor_pos;
        let camera_angle = dir_to_camera.x.atan2(dir_to_camera.z);
        let relative = (actor.facing_yaw - camera_angle).rem_euclid(std::f32::consts::TAU);
        let octant = ((relative + std::f32::consts::FRAC_PI_8)
            / std::f32::consts::FRAC_PI_4) as usize % 8;

        let (direction, mirrored) = match octant {
            0 => (0, false),
            1 => (1, false),
            2 => (2, false),
            3 => (3, false),
            4 => (4, false),
            5 => (3, true),
            6 => (2, true),
            7 => (1, true),
            _ => (0, false),
        };

        // Only swap material when the displayed frame actually changed
        let current_key = (state_idx, sprites.current_frame, direction);
        if current_key != sprites.last_applied {
            sprites.last_applied = current_key;
            let new_mat = sprites.states[state_idx][sprites.current_frame][direction].clone();
            *mat_handle = MeshMaterial3d(new_mat);
        }

        // Billboard: always face camera. Only rotate around Y axis.
        // The directional texture already shows the correct view angle.
        let face_angle = dir_to_camera.x.atan2(dir_to_camera.z);
        transform.rotation = Quat::from_rotation_y(face_angle);
        // Mirror for octants 5-7 via negative X scale, otherwise keep scale at 1.
        transform.scale = Vec3::new(if mirrored { -1.0 } else { 1.0 }, 1.0, 1.0);
    }
}
