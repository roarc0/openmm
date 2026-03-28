//! Sprite loading and resolution for world entities.
//!
//! Resolves DSFT group IDs to sprite roots, loads directional sprite frames,
//! and provides the animation update system. Reusable by any entity type
//! (NPCs, monsters, decorations with animations).

use std::collections::HashMap;

use bevy::{asset::RenderAssetUsages, prelude::*};
use image::{DynamicImage, GenericImageView, RgbaImage};

use lod::LodManager;

use crate::game::entities::AnimationState;
use crate::game::player::PlayerCamera;


/// Cache for loaded sprite materials to avoid duplicate texture loading.
#[derive(Resource, Default, Clone)]
pub struct SpriteCache {
    /// Maps "root_name + frame_letter + direction" → material handle
    materials: HashMap<String, Handle<StandardMaterial>>,
    /// Maps "root_name" → (width, height)
    dimensions: HashMap<String, (f32, f32)>,
}

impl SpriteCache {
    /// Pre-decode a list of (sprite_root, hue_shift) pairs into the cache.
    /// Call during loading screen to avoid decoding during gameplay.
    pub fn preload(
        &mut self,
        roots: &[(&str, f32)],
        lod_manager: &LodManager,
        images: &mut Assets<Image>,
        materials: &mut Assets<StandardMaterial>,
    ) {
        for &(root, hue) in roots {
            load_sprite_frames_cached(root, lod_manager, images, materials, &mut Some(self), hue);
        }
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
    /// Last applied (state, frame, direction) — skip material swap when unchanged.
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
    hue_shift_deg: f32,
) -> (Vec<Vec<[Handle<StandardMaterial>; 5]>>, f32, f32) {
    // Load walking first (usually wider) to get target dimensions
    let (walking, ww, wh) = load_sprite_frames_cached(
        walking_root, lod_manager, images, materials, cache, hue_shift_deg);

    let target_w = ww as u32;
    let target_h = wh as u32;

    // Load standing, padded to at least walking dimensions
    let (standing, sw, sh) = load_sprite_frames_with_min_size(
        standing_root, lod_manager, images, materials, cache, hue_shift_deg,
        target_w, target_h);
    if standing.is_empty() {
        return (Vec::new(), 0.0, 0.0);
    }

    let qw = sw.max(ww);
    let qh = sh.max(wh);

    let mut states = vec![standing];
    if !walking.is_empty() {
        states.push(walking);
    }

    (states, qw, qh)
}

/// Like load_sprite_frames_cached but enforces minimum padding dimensions.
/// Used to pad standing sprites to match walking sprite size.
fn load_sprite_frames_with_min_size(
    root: &str,
    lod_manager: &LodManager,
    images: &mut Assets<Image>,
    materials: &mut Assets<StandardMaterial>,
    cache: &mut Option<&mut SpriteCache>,
    hue_shift_deg: f32,
    min_w: u32,
    min_h: u32,
) -> (Vec<[Handle<StandardMaterial>; 5]>, f32, f32) {
    if min_w == 0 && min_h == 0 {
        return load_sprite_frames_cached(root, lod_manager, images, materials, cache, hue_shift_deg);
    }
    let root = root.trim_end_matches(|c: char| c.is_ascii_digit());

    // Cache key includes min size to avoid returning smaller cached version
    let cache_key = format!("{}@{}x{}{}",
        root, min_w, min_h,
        if hue_shift_deg.abs() > 0.1 { format!("@h{}", hue_shift_deg as i32) } else { String::new() });

    if let Some(c) = cache.as_ref() {
        if let Some(&(w, h)) = c.dimensions.get(&cache_key) {
            let frames = rebuild_from_cache(&cache_key, c, w, h);
            if !frames.is_empty() {
                return (frames, w, h);
            }
        }
    }

    let mut try_root = root;
    while try_root.len() >= 3 {
        let (frames, w, h) = load_frames_with_root_padded(
            try_root, lod_manager, images, materials, hue_shift_deg, min_w, min_h);
        if !frames.is_empty() {
            store_in_cache(&cache_key, &frames, w, h, cache);
            return (frames, w, h);
        }
        try_root = &try_root[..try_root.len() - 1];
    }
    (Vec::new(), 0.0, 0.0)
}

/// Load sprite frames with an optional cache for sharing materials.
/// `hue_shift_deg` rotates the hue of the sprite (in degrees), preserving skin tones.
/// Use 0.0 for no shift, ~120 for blue, ~240 for red/green variants.
pub fn load_sprite_frames_cached(
    root: &str,
    lod_manager: &LodManager,
    images: &mut Assets<Image>,
    materials: &mut Assets<StandardMaterial>,
    cache: &mut Option<&mut SpriteCache>,
    hue_shift_deg: f32,
) -> (Vec<[Handle<StandardMaterial>; 5]>, f32, f32) {
    let root = root.trim_end_matches(|c: char| c.is_ascii_digit());

    // Include hue shift in cache key so tinted variants get separate entries.
    let cache_key = if hue_shift_deg.abs() > 0.1 {
        format!("{}@h{}", root, hue_shift_deg as i32)
    } else {
        root.to_string()
    };

    // Check cache for dimensions (tells us we've loaded this root before)
    if let Some(cache) = cache.as_ref() {
        if let Some(&(w, h)) = cache.dimensions.get(&cache_key) {
            let frames = rebuild_from_cache(&cache_key, cache, w, h);
            if !frames.is_empty() {
                return (frames, w, h);
            }
        }
    }

    // Try progressively shorter roots to handle monlist names like "bar1walk"
    // where the actual sprite files are "bar1waa0" (root "bar1wa").
    let mut try_root = root;
    while try_root.len() >= 3 {
        let (frames, w, h) = load_frames_with_root_padded(try_root, lod_manager, images, materials, hue_shift_deg, 0, 0);
        if !frames.is_empty() {
            store_in_cache(&cache_key, &frames, w, h, cache);
            return (frames, w, h);
        }
        try_root = &try_root[..try_root.len() - 1];
    }
    (Vec::new(), 0.0, 0.0)
}

fn store_in_cache(
    root: &str,
    frames: &[[Handle<StandardMaterial>; 5]],
    w: f32, h: f32,
    cache: &mut Option<&mut SpriteCache>,
) {
    if let Some(cache) = cache.as_mut() {
        cache.dimensions.insert(root.to_string(), (w, h));
        for (fi, dirs) in frames.iter().enumerate() {
            let frame_letter = (b'a' + fi as u8) as char;
            for (di, mat) in dirs.iter().enumerate() {
                let key = format!("{}{}{}", root, frame_letter, di);
                cache.materials.insert(key, mat.clone());
            }
        }
    }
}

fn rebuild_from_cache(
    root: &str,
    cache: &SpriteCache,
    _w: f32, _h: f32,
) -> Vec<[Handle<StandardMaterial>; 5]> {
    let mut frames = Vec::new();
    for fi in 0..6 {
        let frame_letter = (b'a' + fi) as char;
        let key0 = format!("{}{}0", root, frame_letter);
        if let Some(mat0) = cache.materials.get(&key0) {
            let mut dirs: [Handle<StandardMaterial>; 5] = Default::default();
            for di in 0..5 {
                let key = format!("{}{}{}", root, frame_letter, di);
                dirs[di] = cache.materials.get(&key).cloned().unwrap_or_else(|| mat0.clone());
            }
            frames.push(dirs);
        } else {
            break;
        }
    }
    frames
}

fn load_frames_with_root_padded(
    root: &str,
    lod_manager: &LodManager,
    images: &mut Assets<Image>,
    materials: &mut Assets<StandardMaterial>,
    hue_shift_deg: f32,
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
            let img = lod_manager.sprite(&name)
                .or_else(|| lod_manager.sprite(&test_nodir));
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

    // Second pass: pad all sprites to max dimensions and create materials.
    let mut frames = Vec::new();
    for dir_imgs in raw_sprites {
        let mut dir_materials: [Handle<StandardMaterial>; 5] = Default::default();
        for (dir, img_opt) in dir_imgs.into_iter().enumerate() {
            if let Some(mut img) = img_opt {
                if hue_shift_deg.abs() > 0.1 {
                    lod::image::hue_shift(&mut img, hue_shift_deg);
                }
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
                let bevy_img = Image::from_dynamic(img, true, RenderAssetUsages::RENDER_WORLD);
                let tex = images.add(bevy_img);
                dir_materials[dir] = materials.add(StandardMaterial {
                    base_color_texture: Some(tex),
                    alpha_mode: AlphaMode::Mask(0.5),
                    unlit: true,
                    double_sided: true,
                    cull_mode: None,
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

        // Billboard: always face camera. Only rotate around Y axis, never scale.
        // The directional texture already shows the correct view angle.
        let face_angle = dir_to_camera.x.atan2(dir_to_camera.z);
        transform.rotation = Quat::from_rotation_y(face_angle);
        // Mirror for octants 5-7 via negative X scale, otherwise keep scale at 1.
        transform.scale = Vec3::new(if mirrored { -1.0 } else { 1.0 }, 1.0, 1.0);
    }
}
