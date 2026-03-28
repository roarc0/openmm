use bevy::{asset::RenderAssetUsages, prelude::*};

use lod::{monlist::MonsterList, LodManager};

use crate::game::entities::{AnimationState, EntityKind, WorldEntity};
use crate::game::player::PlayerCamera;
use crate::states::loading::PreparedWorld;

/// Unified NPC/monster actor component.
#[derive(Component)]
pub struct Actor {
    pub name: String,
    pub hp: i16,
    pub max_hp: i16,
    pub move_speed: f32,
    pub initial_position: Vec3,
    pub guarding_position: Vec3,
    pub tether_distance: f32,
    pub wander_timer: f32,
    pub wander_target: Vec3,
    pub facing_yaw: f32,
    pub hostile: bool,
}

/// Preloaded sprite frames for an actor.
/// `states[state_idx][frame_idx]` = array of 5 material handles (directions 0-4)
#[derive(Component)]
pub struct ActorSprites {
    /// states[0]=standing, states[1]=walking
    pub states: Vec<Vec<[Handle<StandardMaterial>; 5]>>,
    pub current_frame: usize,
    pub frame_timer: f32,
    pub frame_duration: f32,
}

/// Load sprite frames for a sprite name root (e.g. "pfemst" for standing).
/// Loads frames a-f with directions 0-4. Returns frames + dimensions.
fn load_sprite_frames(
    root: &str,
    lod_manager: &LodManager,
    images: &mut Assets<Image>,
    materials: &mut Assets<StandardMaterial>,
) -> (Vec<[Handle<StandardMaterial>; 5]>, f32, f32) {
    let mut frames = Vec::new();
    let mut sprite_w = 0.0_f32;
    let mut sprite_h = 0.0_f32;

    // The root might already end with 'a' (the first frame letter).
    // Strip trailing digits/letters to normalize.
    let root = root.trim_end_matches(|c: char| c.is_ascii_digit());
    let root = if root.ends_with('a') && root.len() > 3 {
        &root[..root.len() - 1]
    } else {
        root
    };

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

/// Spawn actors from DDM data with proper sprites resolved via DSFT.
pub fn spawn_actors(
    parent: &mut ChildSpawnerCommands,
    prepared: &PreparedWorld,
    lod_manager: &LodManager,
    images: &mut ResMut<Assets<Image>>,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    let monlist = match MonsterList::new(lod_manager) {
        Ok(m) => m,
        Err(_) => return,
    };

    for (_i, actor) in prepared.actors.iter().enumerate() {
        if actor.hp <= 0 {
            continue;
        }
        if actor.position[0].abs() > 20000 || actor.position[1].abs() > 20000 {
            continue;
        }

        // Look up monster description by ID, try sprite names with fallback
        let monster_desc = monlist.get(actor.monster_id as usize);

        let (standing_name, walking_name) = if let Some(desc) = monster_desc {
            (desc.sprite_names[0].clone(), desc.sprite_names[1].clone())
        } else {
            ("pfemst".to_string(), "pfemwa".to_string())
        };

        // Try loading standing frames, with fallback chain
        let fallbacks = [
            (standing_name.as_str(), walking_name.as_str()),
            ("pfemst", "pfemwa"),
            ("pmanst", "pmanwk"),
            ("pmn2st", "pmn2wa"),
        ];

        let mut standing_frames = Vec::new();
        let mut walking_frames = Vec::new();
        let mut sprite_w = 0.0_f32;
        let mut sprite_h = 0.0_f32;

        for (st, wa) in &fallbacks {
            let (sf, w, h) = load_sprite_frames(st, lod_manager, images, materials);
            if !sf.is_empty() && w > 0.0 {
                standing_frames = sf;
                sprite_w = w;
                sprite_h = h;
                let (wf, _, _) = load_sprite_frames(wa, lod_manager, images, materials);
                walking_frames = wf;
                break;
            }
        }

        if standing_frames.is_empty() || sprite_w == 0.0 {
            continue;
        }

        let mut states = vec![standing_frames];
        if !walking_frames.is_empty() {
            states.push(walking_frames);
        }

        let initial_mat = states[0][0][0].clone();
        let quad = meshes.add(Rectangle::new(sprite_w, sprite_h));

        // MM6 coords (x, y, z) → Bevy (x, z, -y)
        let pos = Vec3::new(
            actor.position[0] as f32,
            actor.position[2] as f32 + sprite_h / 2.0,
            -actor.position[1] as f32,
        );
        let initial = Vec3::new(
            actor.initial_position[0] as f32,
            actor.initial_position[2] as f32,
            -actor.initial_position[1] as f32,
        );
        let guarding = Vec3::new(
            actor.guarding_position[0] as f32,
            actor.guarding_position[2] as f32,
            -actor.guarding_position[1] as f32,
        );

        parent.spawn((
            Name::new(format!("actor:{}", actor.name)),
            Mesh3d(quad),
            MeshMaterial3d(initial_mat),
            Transform::from_translation(pos),
            WorldEntity,
            EntityKind::Npc,
            AnimationState::Idle,
            ActorSprites {
                states,
                current_frame: 0,
                frame_timer: 0.0,
                frame_duration: 0.15,
            },
            Actor {
                name: actor.name.clone(),
                hp: actor.hp,
                max_hp: actor.hp,
                move_speed: actor.move_speed as f32,
                initial_position: initial,
                guarding_position: guarding,
                tether_distance: actor.tether_distance as f32,
                wander_timer: 0.0,
                wander_target: pos,
                facing_yaw: 0.0,
                hostile: false,
            },
        ));
    }
}

/// Update actor sprites based on camera angle and animation state.
pub fn update_actor_sprites(
    time: Res<Time>,
    camera_query: Query<&GlobalTransform, With<PlayerCamera>>,
    mut query: Query<(
        &mut ActorSprites,
        &mut MeshMaterial3d<StandardMaterial>,
        &mut Transform,
        &GlobalTransform,
        &AnimationState,
        &Actor,
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
            AnimationState::Walking if sprites.states.len() > 1 && !sprites.states[1].is_empty() => 1,
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

        // Face camera
        if dir_to_camera.x.abs() > 0.01 || dir_to_camera.z.abs() > 0.01 {
            let face_angle = dir_to_camera.x.atan2(dir_to_camera.z);
            transform.rotation = Quat::from_rotation_y(face_angle);
        }

        transform.scale.x = if mirrored { -1.0 } else { 1.0 };
    }
}
