//! Sprite loading and resolution for world entities.
//!
//! Resolves DSFT group IDs to sprite roots, loads directional sprite frames,
//! and provides the animation update system. Reusable by any entity type
//! (NPCs, monsters, decorations with animations).

use std::collections::HashMap;

use bevy::prelude::*;
use image::{DynamicImage, GenericImageView, RgbaImage};

use lod::LodManager;

use crate::game::entities::{AnimationState, FacingYaw};
use crate::game::entities::actor::Actor;
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
        roots: &[(&str, u8, u16)],
        lod_manager: &LodManager,
        images: &mut Assets<Image>,
        materials: &mut Assets<StandardMaterial>,
    ) {
        for &(root, variant, palette_id) in roots {
            load_sprite_frames(root, lod_manager, images, materials, &mut Some(self), variant, 0, 0, palette_id);
        }
    }
}

/// Build a cache key for a sprite root with optional variant, minimum size, and palette.
/// Format: "root", "root@v2", "root@v2p223", "root@64x128", "root@64x128@v2", "root@64x128@v2p223"
/// palette_id is included only when variant > 1 && palette_id > 0 (DSFT direct path).
fn cache_key(root: &str, variant: u8, min_w: u32, min_h: u32, palette_id: u16) -> String {
    let has_size = min_w > 0 || min_h > 0;
    let has_variant = variant > 1;
    let has_palette = has_variant && palette_id > 0;
    match (has_size, has_variant, has_palette) {
        (false, false, _)      => root.to_string(),
        (false, true,  false)  => format!("{}@v{}", root, variant),
        (false, true,  true)   => format!("{}@v{}p{}", root, variant, palette_id),
        (true,  false, _)      => format!("{}@{}x{}", root, min_w, min_h),
        (true,  true,  false)  => format!("{}@{}x{}@v{}", root, min_w, min_h, variant),
        (true,  true,  true)   => format!("{}@{}x{}@v{}p{}", root, min_w, min_h, variant, palette_id),
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
/// Load entity sprites. `palette_id` is the DSFT palette for this variant —
/// when non-zero and variant > 1, used directly for palette swap instead of
/// the offset-from-sprite-header approach (which uses a different numbering).
pub fn load_entity_sprites(
    standing_root: &str,
    walking_root: &str,
    lod_manager: &LodManager,
    images: &mut Assets<Image>,
    materials: &mut Assets<StandardMaterial>,
    cache: &mut Option<&mut SpriteCache>,
    variant: u8,
    palette_id: u16,
) -> (Vec<Vec<[Handle<StandardMaterial>; 5]>>, f32, f32) {
    // Load walking first (usually wider) to get target dimensions
    let (walking, ww, wh) = load_sprite_frames(
        walking_root, lod_manager, images, materials, cache, variant, 0, 0, palette_id);

    // Load standing, padded to at least walking dimensions
    let (standing, sw, sh) = load_sprite_frames(
        standing_root, lod_manager, images, materials, cache, variant,
        ww as u32, wh as u32, palette_id);
    if standing.is_empty() {
        return (Vec::new(), 0.0, 0.0);
    }

    let qw = sw.max(ww);
    let qh = sh.max(wh);

    // If standing is larger than walking, reload walking padded to match
    let walking = if !walking.is_empty() && (sw > ww || sh > wh) {
        let (padded, _, _) = load_sprite_frames(
            walking_root, lod_manager, images, materials, cache, variant,
            qw as u32, qh as u32, palette_id);
        padded
    } else {
        walking
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
/// `palette_id` is the DSFT palette — when non-zero and variant > 1, used directly.
pub fn load_sprite_frames(
    root: &str,
    lod_manager: &LodManager,
    images: &mut Assets<Image>,
    materials: &mut Assets<StandardMaterial>,
    cache: &mut Option<&mut SpriteCache>,
    variant: u8,
    min_w: u32,
    min_h: u32,
    palette_id: u16,
) -> (Vec<[Handle<StandardMaterial>; 5]>, f32, f32) {
    let root = root.trim_end_matches(|c: char| c.is_ascii_digit());
    let key = cache_key(root, variant, min_w, min_h, palette_id);

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
            try_root, lod_manager, images, materials, variant, min_w, min_h, palette_id);
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
/// When `palette_id > 0` and `variant > 1`, uses the DSFT palette directly
/// (sprite file header palettes use a different numbering system).
fn decode_sprite_frames(
    root: &str,
    lod_manager: &LodManager,
    images: &mut Assets<Image>,
    materials: &mut Assets<StandardMaterial>,
    variant: u8,
    min_w: u32,
    min_h: u32,
    palette_id: u16,
) -> (Vec<[Handle<StandardMaterial>; 5]>, f32, f32) {
    // First pass: collect all raw sprites and find max dimensions.
    let mut raw_sprites: Vec<Vec<Option<DynamicImage>>> = Vec::new();
    let mut max_w = min_w;
    let mut max_h = min_h;

    for frame_char in b'a'..=b'f' {
        let frame_letter = frame_char as char;
        let test0 = format!("{}{}0", root, frame_letter);
        let test_nodir = format!("{}{}", root, frame_letter);

        let has_frame = lod_manager.game().sprite(&test0).is_some()
            || lod_manager.game().sprite(&test_nodir).is_some();
        if !has_frame {
            break;
        }

        let mut dir_imgs: Vec<Option<DynamicImage>> = Vec::with_capacity(5);
        for dir in 0..5u8 {
            let name = format!("{}{}{}", root, frame_letter, dir);
            let img = if variant > 1 && palette_id > 0 {
                // Use DSFT palette directly (sprite header palettes differ in numbering)
                let sprite_name = if lod_manager.try_get_bytes(format!("sprites/{}", name.to_lowercase())).is_ok() {
                    &name
                } else { &test_nodir };
                lod_manager.game().sprite_with_palette(sprite_name, palette_id)
                    .or_else(|| lod_manager.game().sprite(sprite_name))
            } else if variant > 1 {
                let pal_offset = (variant - 1) as u16;
                load_sprite_with_palette_offset(lod_manager, &name, &test_nodir, pal_offset)
            } else {
                lod_manager.game().sprite(&name)
                    .or_else(|| lod_manager.game().sprite(&test_nodir))
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
    lod_manager.game().sprite_with_palette(sprite_name, variant_palette_id)
        .or_else(|| lod_manager.game().sprite(sprite_name))
}

/// Update sprite sheets based on camera angle, entity facing, and animation state.
/// Works with Actor entities (NPCs/monsters) and directional decorations (FacingYaw).
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
        Option<&Actor>,
        Option<&FacingYaw>,
        &Visibility,
    )>,
) {
    let Ok(camera_gt) = camera_query.single() else {
        return;
    };
    let cam_pos = camera_gt.translation();
    let dt = time.delta_secs();

    for (mut sprites, mut mat_handle, mut transform, global_transform, anim_state, actor, facing_yaw, vis) in
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

        // Pick directional frame based on camera angle relative to entity facing.
        // Actors use Actor.facing_yaw (updated by wander), decorations use FacingYaw (fixed from map).
        let entity_yaw = actor.map(|a| a.facing_yaw)
            .or_else(|| facing_yaw.map(|f| f.0))
            .unwrap_or(0.0);
        let dir_to_camera = cam_pos - actor_pos;
        let camera_angle = dir_to_camera.x.atan2(dir_to_camera.z);
        let (direction, mirrored) = direction_for_angle(entity_yaw, camera_angle);

        // Only swap material when the displayed frame actually changed
        let current_key = (state_idx, sprites.current_frame, direction);
        if current_key != sprites.last_applied {
            sprites.last_applied = current_key;
            let new_mat = sprites.states[state_idx][sprites.current_frame][direction].clone();
            *mat_handle = MeshMaterial3d(new_mat);
        }

        transform.rotation = Quat::from_rotation_y(camera_angle);
        // Mirror for octants 5-7 via negative X scale, otherwise keep scale at 1.
        transform.scale = Vec3::new(if mirrored { -1.0 } else { 1.0 }, 1.0, 1.0);
    }
}

/// Pick the sprite direction index (0-4) and mirror flag from an entity's facing
/// yaw and the camera angle. MM6 sprites have 5 pre-rendered views (0=front,
/// 1-4=rotations). Octants 5-7 mirror views 3-1 via negative X scale.
pub fn direction_for_angle(facing_yaw: f32, camera_angle: f32) -> (usize, bool) {
    let relative = (facing_yaw - camera_angle).rem_euclid(std::f32::consts::TAU);
    let octant = ((relative + std::f32::consts::FRAC_PI_8)
        / std::f32::consts::FRAC_PI_4) as usize % 8;
    match octant {
        0 => (0, false),
        1 => (1, false),
        2 => (2, false),
        3 => (3, false),
        4 => (4, false),
        5 => (3, true),
        6 => (2, true),
        7 => (1, true),
        _ => (0, false),
    }
}

/// Load decoration directional sprites (e.g. "shp0"-"shp4").
/// Unlike NPC sprites, decoration directions use `{root}{direction}` naming
/// with no frame letters or animation — just 5 pre-rendered views.
/// Returns (materials_for_5_directions, width, height).
pub fn load_decoration_directions(
    root: &str,
    lod_manager: &LodManager,
    images: &mut Assets<Image>,
    materials: &mut Assets<StandardMaterial>,
    cache: &mut Option<&mut SpriteCache>,
) -> ([Handle<StandardMaterial>; 5], f32, f32) {
    let key = format!("dec:{}", root);
    if let Some(c) = cache.as_ref() {
        if let Some(&(w, h)) = c.dimensions.get(&key) {
            let mut dirs: [Handle<StandardMaterial>; 5] = Default::default();
            let mut found = true;
            for di in 0..5 {
                let mat_key = format!("{}a{}", key, di);
                if let Some(mat) = c.materials.get(&mat_key) {
                    dirs[di] = mat.clone();
                } else {
                    found = false;
                    break;
                }
            }
            if found {
                return (dirs, w, h);
            }
        }
    }

    // Load all direction sprites and find max dimensions
    let mut raw: Vec<Option<DynamicImage>> = Vec::with_capacity(5);
    let mut max_w = 0u32;
    let mut max_h = 0u32;
    for dir in 0..5u8 {
        let name = format!("{}{}", root, dir);
        let img = lod_manager.game().sprite(&name);
        if let Some(ref i) = img {
            max_w = max_w.max(i.width());
            max_h = max_h.max(i.height());
        }
        raw.push(img);
    }

    if max_w == 0 || raw[0].is_none() {
        return (Default::default(), 0.0, 0.0);
    }

    // Pad to uniform size and create materials
    let mut dirs: [Handle<StandardMaterial>; 5] = Default::default();
    for (dir, img_opt) in raw.into_iter().enumerate() {
        if let Some(img) = img_opt {
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
            dirs[dir] = materials.add(StandardMaterial {
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
            dirs[dir] = dirs[0].clone();
        }
    }

    // Store in cache
    if let Some(cache) = cache.as_mut() {
        cache.dimensions.insert(key.clone(), (max_w as f32, max_h as f32));
        for (di, mat) in dirs.iter().enumerate() {
            let mat_key = format!("{}a{}", key, di);
            cache.materials.insert(mat_key, mat.clone());
        }
    }

    (dirs, max_w as f32, max_h as f32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};

    #[test]
    fn direction_front_when_camera_faces_entity() {
        // Camera is directly in front of entity (same angle as facing)
        let (dir, mirror) = direction_for_angle(0.0, 0.0);
        assert_eq!(dir, 0);
        assert!(!mirror);
    }

    #[test]
    fn direction_back_when_camera_behind_entity() {
        // Camera is directly behind (facing 0, camera at PI)
        let (dir, mirror) = direction_for_angle(0.0, PI);
        assert_eq!(dir, 4);
        assert!(!mirror);
    }

    #[test]
    fn direction_right_side() {
        // Camera is 90° to the right of entity facing
        let (dir, mirror) = direction_for_angle(0.0, -FRAC_PI_2);
        assert_eq!(dir, 2);
        assert!(!mirror);
    }

    #[test]
    fn direction_left_side_mirrors() {
        // Camera is 90° to the left — should mirror direction 2
        let (dir, mirror) = direction_for_angle(0.0, FRAC_PI_2);
        assert_eq!(dir, 2);
        assert!(mirror);
    }

    #[test]
    fn direction_symmetry_octants_5_6_7_mirror() {
        // Octants 5, 6, 7 should mirror 3, 2, 1 respectively
        // Octant 5: relative ≈ 5*PI/4
        let (dir, mirror) = direction_for_angle(0.0, -5.0 * FRAC_PI_4);
        assert_eq!(dir, 3, "octant 5 should use direction 3");
        assert!(mirror, "octant 5 should be mirrored");

        // Octant 7: relative ≈ 7*PI/4
        let (dir, mirror) = direction_for_angle(0.0, -7.0 * FRAC_PI_4);
        assert_eq!(dir, 1, "octant 7 should use direction 1");
        assert!(mirror, "octant 7 should be mirrored");
    }

    #[test]
    fn direction_wraps_around_tau() {
        // Angles that differ by TAU should give the same result
        let (d1, m1) = direction_for_angle(0.5, 1.0);
        let (d2, m2) = direction_for_angle(0.5 + TAU, 1.0);
        assert_eq!(d1, d2);
        assert_eq!(m1, m2);

        let (d3, m3) = direction_for_angle(0.5, 1.0 + TAU);
        assert_eq!(d1, d3);
        assert_eq!(m1, m3);
    }

    #[test]
    fn direction_negative_angles() {
        // Negative facing and camera angles should work correctly
        let (dir, mirror) = direction_for_angle(-PI, -PI);
        assert_eq!(dir, 0, "same direction should always be front");
        assert!(!mirror);
    }

    #[test]
    fn all_eight_octants_covered() {
        // Walk around the full circle in 8 steps and verify we get all expected directions
        let expected = [
            (0, false), // 0: front
            (1, false), // 1: front-right
            (2, false), // 2: right
            (3, false), // 3: back-right
            (4, false), // 4: back
            (3, true),  // 5: back-left (mirror of 3)
            (2, true),  // 6: left (mirror of 2)
            (1, true),  // 7: front-left (mirror of 1)
        ];
        for (i, &(exp_dir, exp_mirror)) in expected.iter().enumerate() {
            let angle = i as f32 * FRAC_PI_4;
            let (dir, mirror) = direction_for_angle(angle, 0.0);
            assert_eq!(
                (dir, mirror),
                (exp_dir, exp_mirror),
                "octant {}: facing={:.2} camera=0.0",
                i,
                angle
            );
        }
    }

    /// Regression: preloaded cache entries (palette_id=0) must not collide with
    /// spawn-time entries that use a specific DSFT palette_id. Before the fix,
    /// "gstfly@v2" was cached by preload with palette_id=0 (sprite-header offset path),
    /// then reused at spawn time when the correct DSFT palette was 223 — causing the
    /// walking animation to display with the wrong palette while standing was correct
    /// (it used a different cache key due to min_w/min_h padding).
    #[test]
    fn cache_key_includes_palette_id_when_variant_and_palette_nonzero() {
        // No palette: key should not include palette suffix
        assert_eq!(cache_key("gstfly", 2, 0, 0, 0), "gstfly@v2");
        // With DSFT palette: key must differ so preloaded (palette=0) and spawn-time entries don't collide
        assert_eq!(cache_key("gstfly", 2, 0, 0, 223), "gstfly@v2p223");
        assert_ne!(
            cache_key("gstfly", 2, 0, 0, 0),
            cache_key("gstfly", 2, 0, 0, 223),
            "palette_id=0 and palette_id=223 must produce distinct cache keys"
        );
        // variant=1 never uses the DSFT palette path, so palette_id is irrelevant
        assert_eq!(cache_key("gstfly", 1, 0, 0, 223), "gstfly");
        assert_eq!(cache_key("gstfly", 1, 0, 0, 0),   "gstfly");
        // Palette must also be distinct when min dimensions are present
        assert_ne!(
            cache_key("gstfly", 2, 64, 128, 0),
            cache_key("gstfly", 2, 64, 128, 223),
        );
    }

}
