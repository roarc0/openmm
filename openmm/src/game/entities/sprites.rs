//! Sprite loading and resolution for world entities.
//!
//! Resolves DSFT group IDs to sprite roots, loads directional sprite frames,
//! and provides the animation update system. Reusable by any entity type
//! (NPCs, monsters, decorations with animations).

use bevy::{asset::RenderAssetUsages, prelude::*};

use lod::{dsft::DSFT, LodManager};

use crate::game::entities::AnimationState;
use crate::game::player::PlayerCamera;

/// Preloaded sprite frames for an entity.
/// `states[state_idx][frame_idx]` = array of 5 material handles (directions 0-4).
#[derive(Component)]
pub struct SpriteSheet {
    /// states[0]=standing, states[1]=walking (if available)
    pub states: Vec<Vec<[Handle<StandardMaterial>; 5]>>,
    pub current_frame: usize,
    pub frame_timer: f32,
    pub frame_duration: f32,
}

/// Resolve a DSFT group ID to a sprite root name.
/// e.g. group 1402 → frame "skesta" → root "skest"
pub fn resolve_sprite_root(dsft: &DSFT, group_id: u16) -> Option<String> {
    if group_id == 0 || (group_id as usize) >= dsft.groups.len() {
        return None;
    }
    let frame_idx = dsft.groups[group_id as usize] as usize;
    if frame_idx >= dsft.frames.len() {
        return None;
    }
    let name = dsft.frames[frame_idx].sprite_name()?;
    let bytes = name.as_bytes();
    if bytes.len() < 2 {
        return Some(name);
    }
    let last = bytes[bytes.len() - 1];
    let second_last = bytes[bytes.len() - 2];
    if last.is_ascii_digit() && second_last.is_ascii_lowercase() {
        Some(name[..name.len() - 2].to_string())
    } else if last.is_ascii_lowercase() {
        Some(name[..name.len() - 1].to_string())
    } else {
        Some(name)
    }
}

/// Load all directional sprite frames for a given root name.
/// Tries frames a-f with directions 0-4.
/// Returns (frames, width, height).
pub fn load_sprite_frames(
    root: &str,
    lod_manager: &LodManager,
    images: &mut Assets<Image>,
    materials: &mut Assets<StandardMaterial>,
) -> (Vec<[Handle<StandardMaterial>; 5]>, f32, f32) {
    let mut frames = Vec::new();
    let mut sprite_w = 0.0_f32;
    let mut sprite_h = 0.0_f32;

    let root = root.trim_end_matches(|c: char| c.is_ascii_digit());

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

            if let Some(img) = img {
                if sprite_w == 0.0 {
                    sprite_w = img.width() as f32;
                    sprite_h = img.height() as f32;
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

/// Load a full SpriteSheet from DSFT group IDs.
/// `sprite_ids` layout: [0]=dying, [1]=fidget, [2]=standing, [3]=walking, [4]=hit
/// Returns (SpriteSheet, width, height) or None if no sprites load.
pub fn load_actor_sprite_sheet(
    dsft: &DSFT,
    sprite_ids: &[u16; 8],
    lod_manager: &LodManager,
    images: &mut Assets<Image>,
    materials: &mut Assets<StandardMaterial>,
) -> Option<(SpriteSheet, f32, f32)> {
    let standing_root = resolve_sprite_root(dsft, sprite_ids[2]);
    let walking_root = resolve_sprite_root(dsft, sprite_ids[3]);

    let st_root = standing_root.as_deref().unwrap_or("pfemst");
    let wa_root = walking_root.as_deref().unwrap_or("pfemwa");

    let (standing_frames, sprite_w, sprite_h) =
        load_sprite_frames(st_root, lod_manager, images, materials);

    if standing_frames.is_empty() || sprite_w == 0.0 {
        return None;
    }

    let (walking_frames, _, _) =
        load_sprite_frames(wa_root, lod_manager, images, materials);

    let mut states = vec![standing_frames];
    if !walking_frames.is_empty() {
        states.push(walking_frames);
    }

    Some((
        SpriteSheet {
            states,
            current_frame: 0,
            frame_timer: 0.0,
            frame_duration: 0.15,
        },
        sprite_w,
        sprite_h,
    ))
}

/// Update sprite sheets based on camera angle, entity facing, and animation state.
/// Works for any entity with SpriteSheet + AnimationState + a facing direction.
pub fn update_sprite_sheets(
    time: Res<Time>,
    camera_query: Query<&GlobalTransform, With<PlayerCamera>>,
    mut query: Query<(
        &mut SpriteSheet,
        &mut MeshMaterial3d<StandardMaterial>,
        &mut Transform,
        &GlobalTransform,
        &AnimationState,
        &super::actor::Actor,
    )>,
) {
    let Ok(camera_gt) = camera_query.single() else {
        return;
    };
    let cam_pos = camera_gt.translation();
    let dt = time.delta_secs();

    for (mut sprites, mut mat_handle, mut transform, global_transform, anim_state, actor) in
        query.iter_mut()
    {
        let actor_pos = global_transform.translation();

        let state_idx = match anim_state {
            AnimationState::Walking
                if sprites.states.len() > 1 && !sprites.states[1].is_empty() => 1,
            _ => 0,
        };
        let frame_count = sprites.states[state_idx].len();
        if frame_count == 0 {
            continue;
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

        let new_mat = sprites.states[state_idx][sprites.current_frame][direction].clone();
        *mat_handle = MeshMaterial3d(new_mat);

        // Face camera (billboard)
        if dir_to_camera.x.abs() > 0.01 || dir_to_camera.z.abs() > 0.01 {
            let face_angle = dir_to_camera.x.atan2(dir_to_camera.z);
            transform.rotation = Quat::from_rotation_y(face_angle);
        }

        transform.scale.x = if mirrored { -1.0 } else { 1.0 };
    }
}
