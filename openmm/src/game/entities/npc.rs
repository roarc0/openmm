use bevy::{asset::RenderAssetUsages, prelude::*};

use lod::LodManager;

use crate::game::entities::{AnimationState, EntityKind, WorldEntity};
use crate::game::player::PlayerCamera;
use crate::states::loading::PreparedWorld;

/// Unified NPC/monster actor component. In MM6, NPCs and monsters are the same
/// entity type — the difference is the `hostile` flag. Peasants are non-hostile
/// by default but can become hostile if attacked.
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
    /// Actor's facing direction in radians (Y-axis rotation). Used for directional sprites.
    pub facing_yaw: f32,
    /// Whether this actor is hostile to the player.
    pub hostile: bool,
}

/// Holds all preloaded sprite frames for an actor, indexed by [state][frame][direction].
/// States: 0=stand, 1=walk. Frames: up to 6 (a-f). Directions: 0-4 (mirrored for 5-7).
#[derive(Component)]
pub struct ActorSprites {
    /// [state_index][frame_index] = array of 5 material handles (directions 0-4)
    /// state 0 = standing, state 1 = walking
    pub states: Vec<Vec<[Handle<StandardMaterial>; 5]>>,
    pub frame_count: [usize; 2], // number of frames per state
    pub current_frame: usize,
    pub frame_timer: f32,
    pub frame_duration: f32,
    /// Which direction index is currently shown (for mesh flipping)
    pub current_direction: usize,
    pub is_mirrored: bool,
}

/// Sprite prefixes for peasant variants.
const PEASANT_SPRITES: &[&str] = &["pfem", "pman", "pmn2"];

/// Animation states we load sprites for
const ANIM_STATES: &[&str] = &["st", "wa"];

/// Load all sprite frames for an NPC actor.
fn load_actor_sprites(
    lod_manager: &LodManager,
    actor_index: usize,
    images: &mut Assets<Image>,
    materials: &mut Assets<StandardMaterial>,
) -> Option<(ActorSprites, f32, f32)> {
    let prefix = PEASANT_SPRITES[actor_index % PEASANT_SPRITES.len()];

    let mut states: Vec<Vec<[Handle<StandardMaterial>; 5]>> = Vec::new();
    let mut frame_counts = [0usize; 2];
    let mut sprite_w = 0.0_f32;
    let mut sprite_h = 0.0_f32;

    for (si, state) in ANIM_STATES.iter().enumerate() {
        let mut frames = Vec::new();
        for frame_char in b'a'..=b'f' {
            let frame_letter = frame_char as char;
            // Try loading direction 0 to check if this frame exists
            let test_name = format!("{}{}{}{}", prefix, state, frame_letter, 0);
            if lod_manager.sprite(&test_name).is_none() {
                break;
            }

            let mut dir_materials: [Handle<StandardMaterial>; 5] = Default::default();
            for dir in 0..5u8 {
                let name = format!("{}{}{}{}", prefix, state, frame_letter, dir);
                if let Some(img) = lod_manager.sprite(&name) {
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
                } else {
                    // Reuse direction 0 as fallback
                    dir_materials[dir as usize] = dir_materials[0].clone();
                }
            }
            frames.push(dir_materials);
        }
        frame_counts[si] = frames.len();
        states.push(frames);
    }

    if states.is_empty() || states[0].is_empty() {
        return None;
    }

    Some((
        ActorSprites {
            states,
            frame_count: frame_counts,
            current_frame: 0,
            frame_timer: 0.0,
            frame_duration: 0.15, // ~6.7 FPS animation
            current_direction: 0,
            is_mirrored: false,
        },
        sprite_w,
        sprite_h,
    ))
}

/// Spawn actors from DDM data with full sprite sets.
pub fn spawn_actors(
    parent: &mut ChildSpawnerCommands,
    prepared: &PreparedWorld,
    lod_manager: &LodManager,
    images: &mut ResMut<Assets<Image>>,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    for (i, actor) in prepared.actors.iter().enumerate() {
        if actor.hp <= 0 {
            continue;
        }
        // Skip actors with positions far outside the playable area
        if actor.position[0].abs() > 20000 || actor.position[1].abs() > 20000 {
            continue;
        }

        let (sprites, sprite_w, sprite_h) =
            match load_actor_sprites(lod_manager, i, images, materials) {
                Some(s) => s,
                None => continue,
            };

        // Start with standing frame 0 direction 0
        let initial_mat = sprites.states[0][0][0].clone();
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
            sprites,
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
                hostile: false, // TODO: read from MonsterInfo attributes
            },
        ));
    }
}

/// Update actor sprites based on camera angle and animation state.
/// Picks the correct direction and advances animation frames.
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

        // Determine which animation state to use
        let state_idx = match anim_state {
            AnimationState::Walking if sprites.states.len() > 1 && !sprites.states[1].is_empty() => 1,
            _ => 0,
        };
        let frame_count = sprites.states[state_idx].len();
        if frame_count == 0 {
            continue;
        }

        // Advance animation frame
        sprites.frame_timer += dt;
        if sprites.frame_timer >= sprites.frame_duration {
            sprites.frame_timer -= sprites.frame_duration;
            sprites.current_frame = (sprites.current_frame + 1) % frame_count;
        }
        // Clamp in case state changed and frame count differs
        if sprites.current_frame >= frame_count {
            sprites.current_frame = 0;
        }

        // Calculate direction based on camera angle relative to actor's facing
        let dir_to_camera = cam_pos - actor_pos;
        let camera_angle = dir_to_camera.x.atan2(dir_to_camera.z);

        // Use actor's movement facing, not transform rotation
        let relative = (camera_angle - actor.facing_yaw).rem_euclid(std::f32::consts::TAU);

        // Map to 8 octants, then to our 5 directions (0-4) with mirroring
        let octant = ((relative + std::f32::consts::FRAC_PI_8)
            / std::f32::consts::FRAC_PI_4) as usize
            % 8;

        // Octant to direction mapping:
        // 0=front, 1=front-right, 2=right, 3=back-right, 4=back
        // 5=back-left(mirror 3), 6=left(mirror 2), 7=front-left(mirror 1)
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

        // Update material if direction or frame changed
        let new_mat = sprites.states[state_idx][sprites.current_frame][direction].clone();
        *mat_handle = MeshMaterial3d(new_mat);

        // Always face camera (billboard behavior)
        if dir_to_camera.x.abs() > 0.01 || dir_to_camera.z.abs() > 0.01 {
            let face_angle = dir_to_camera.x.atan2(dir_to_camera.z);
            transform.rotation = Quat::from_rotation_y(face_angle);
        }


        // Mirror by flipping X scale
        let x_scale = if mirrored { -1.0 } else { 1.0 };
        transform.scale.x = x_scale;

        sprites.current_direction = direction;
        sprites.is_mirrored = mirrored;
    }
}
