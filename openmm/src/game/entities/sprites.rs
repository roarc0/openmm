//! Sprite loading and resolution for world entities.
//!
//! Resolves DSFT group IDs to sprite roots, loads directional sprite frames,
//! and provides the animation update system. Reusable by any entity type
//! (NPCs, monsters, decorations with animations).

use std::collections::HashMap;

use bevy::{asset::RenderAssetUsages, prelude::*};

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

/// Load all directional sprite frames for a given root name.
pub fn load_sprite_frames(
    root: &str,
    lod_manager: &LodManager,
    images: &mut Assets<Image>,
    materials: &mut Assets<StandardMaterial>,
) -> (Vec<[Handle<StandardMaterial>; 5]>, f32, f32) {
    load_sprite_frames_cached(root, lod_manager, images, materials, &mut None, 0.0)
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
        let (frames, w, h) = load_frames_with_root(try_root, lod_manager, images, materials, hue_shift_deg);
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

fn load_frames_with_root(
    root: &str,
    lod_manager: &LodManager,
    images: &mut Assets<Image>,
    materials: &mut Assets<StandardMaterial>,
    hue_shift_deg: f32,
) -> (Vec<[Handle<StandardMaterial>; 5]>, f32, f32) {
    let mut frames = Vec::new();
    let mut sprite_w = 0.0_f32;
    let mut sprite_h = 0.0_f32;

    for frame_char in b'a'..=b'f' {
        let frame_letter = frame_char as char;
        let test0 = format!("{}{}0", root, frame_letter);
        let test_nodir = format!("{}{}", root, frame_letter);

        let has_frame = lod_manager.sprite(&test0).is_some()
            || lod_manager.sprite(&test_nodir).is_some();
        if !has_frame {
            break;
        }

        let mut dir_materials: [Handle<StandardMaterial>; 5] = Default::default();
        for dir in 0..5u8 {
            let name = format!("{}{}{}",root, frame_letter, dir);
            let img = lod_manager.sprite(&name)
                .or_else(|| lod_manager.sprite(&test_nodir));

            if let Some(mut img) = img {
                if sprite_w == 0.0 {
                    sprite_w = img.width() as f32;
                    sprite_h = img.height() as f32;
                }
                if hue_shift_deg.abs() > 0.1 {
                    lod::image::hue_shift(&mut img, hue_shift_deg);
                }
                let bevy_img = Image::from_dynamic(img, true, RenderAssetUsages::RENDER_WORLD);
                let tex = images.add(bevy_img);
                dir_materials[dir as usize] = materials.add(StandardMaterial {
                    base_color_texture: Some(tex),
                    alpha_mode: AlphaMode::Mask(0.5),
                    unlit: true,
                    double_sided: true,
                    cull_mode: None,
                    ..default()
                });
            } else if dir > 0 {
                dir_materials[dir as usize] = dir_materials[0].clone();
            }
        }
        frames.push(dir_materials);
    }

    (frames, sprite_w, sprite_h)
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
            // Adjust scale for different sprite dimensions between states
            if sprites.state_dimensions.len() > state_idx {
                let (base_w, base_h) = sprites.state_dimensions[0];
                let (_, new_h) = sprites.state_dimensions[state_idx];
                if base_w > 0.0 && base_h > 0.0 {
                    transform.scale.y = new_h / base_h;
                }
            }
        }

        // Advance animation
        sprites.frame_timer += dt;
        if sprites.frame_timer >= sprites.frame_duration {
            sprites.frame_timer -= sprites.frame_duration;
            sprites.current_frame = (sprites.current_frame + 1) % frame_count;
        }
        if sprites.current_frame >= frame_count {
            sprites.current_frame = 0;
        }

        // Direction from camera angle relative to actor facing
        let dir_to_camera = cam_pos - actor_pos;
        let camera_angle = dir_to_camera.x.atan2(dir_to_camera.z);
        let relative = (camera_angle - actor.facing_yaw).rem_euclid(std::f32::consts::TAU);

        let octant = ((relative + std::f32::consts::FRAC_PI_8)
            / std::f32::consts::FRAC_PI_4) as usize
            % 8;

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

        // Only swap material when something actually changed
        let current_key = (state_idx, sprites.current_frame, direction);
        if current_key != sprites.last_applied {
            sprites.last_applied = current_key;
            let new_mat = sprites.states[state_idx][sprites.current_frame][direction].clone();
            *mat_handle = MeshMaterial3d(new_mat);
        }

        // Face camera (billboard)
        if dir_to_camera.x.abs() > 0.01 || dir_to_camera.z.abs() > 0.01 {
            let face_angle = dir_to_camera.x.atan2(dir_to_camera.z);
            transform.rotation = Quat::from_rotation_y(face_angle);
        }

        // Apply mirror + width scale
        let width_scale = if sprites.state_dimensions.len() > state_idx {
            let (base_w, _) = sprites.state_dimensions[0];
            let (new_w, _) = sprites.state_dimensions[state_idx];
            if base_w > 0.0 { new_w / base_w } else { 1.0 }
        } else {
            1.0
        };
        transform.scale.x = if mirrored { -width_scale } else { width_scale };
    }
}
