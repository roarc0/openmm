//! Indoor world spawning — meshes, doors, decorations, lights, monsters.

use bevy::prelude::*;

use crate::game::InGame;
use crate::game::coords::mm6_position_to_bevy;
use crate::game::spawn::SpawnCtx;
use crate::game::spawn::actor::{ActorKind, ActorSpawnParams, spawn_actor};
use crate::game::sprites::loading as sprites;
use crate::game::sprites::material::SpriteMaterial;
use crate::states::loading::PreparedIndoorWorld;

use super::types::{
    BlvDoors, DoorColliders, DoorCollisionFace, DoorFace, DoorRuntime, OccluderFaceInfo, OccluderFaces,
    TouchTriggerFaces, TouchTriggerInfo,
};

/// Main entry point: spawn the entire indoor world from prepared data.
pub(crate) fn spawn_indoor_world(
    prepared: Res<PreparedIndoorWorld>,
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut sprite_materials: ResMut<Assets<SpriteMaterial>>,
    game_assets: Res<crate::assets::GameAssets>,
    cfg: Res<crate::config::GameConfig>,
) {
    spawn_static_meshes(&prepared, &mut commands, &mut images, &mut meshes, &mut materials, &cfg);
    spawn_door_faces(&prepared, &mut commands, &mut images, &mut meshes, &mut materials);
    commands.insert_resource(build_blv_doors(&prepared));
    commands.insert_resource(build_door_colliders(&prepared));
    commands.insert_resource(build_clickable_faces(&prepared));
    commands.insert_resource(build_occluder_faces(&prepared));
    commands.insert_resource(build_touch_triggers(&prepared));
    spawn_ambient_light(&mut commands);
    spawn_decorations(
        &prepared,
        &mut commands,
        &mut images,
        &mut meshes,
        &mut sprite_materials,
        &game_assets,
        &cfg,
    );
    spawn_blv_lights(&prepared, &mut commands);
    spawn_indoor_monsters(
        &prepared,
        &mut commands,
        &mut images,
        &mut meshes,
        &mut sprite_materials,
        &game_assets,
        &cfg,
    );
}

/// Spawn all static face meshes (grouped by texture).
fn spawn_static_meshes(
    prepared: &PreparedIndoorWorld,
    commands: &mut Commands,
    images: &mut Assets<Image>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    cfg: &crate::config::GameConfig,
) {
    let model_sampler = crate::assets::sampler_for_filtering(&cfg.models_filtering);
    for model in &prepared.models {
        for sub in &model.sub_meshes {
            let mut mat = sub.material.clone();
            if let Some(ref tex) = sub.texture {
                let mut img = tex.clone();
                img.sampler = model_sampler.clone();
                let tex_handle = images.add(img);
                mat.base_color_texture = Some(tex_handle);
            }
            commands.spawn((
                Mesh3d(meshes.add(sub.mesh.clone())),
                MeshMaterial3d(materials.add(mat)),
                InGame,
            ));
        }
    }
}

/// Spawn door face entities individually (for animation).
fn spawn_door_faces(
    prepared: &PreparedIndoorWorld,
    commands: &mut Commands,
    images: &mut Assets<Image>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    for df in &prepared.door_face_meshes {
        let mut mat = df.material.clone();
        if let Some(ref tex) = df.texture {
            let tex_handle = images.add(tex.clone());
            mat.base_color_texture = Some(tex_handle);
        }
        commands.spawn((
            Mesh3d(meshes.add(df.mesh.clone())),
            MeshMaterial3d(materials.add(mat)),
            DoorFace {
                door_index: df.door_index,
                face_index: df.face_index,
                is_moving_vertex: df.is_moving_vertex.clone(),
                base_positions: df.base_positions.clone(),
                uv_rate: df.uv_rate,
                base_uvs: df.base_uvs.clone(),
                moves_by_door: df.moves_by_door,
            },
            InGame,
        ));
    }
}

/// Build BlvDoors resource from prepared door data.
/// Preserves indices so DoorFace.door_index matches directly.
fn build_blv_doors(prepared: &PreparedIndoorWorld) -> BlvDoors {
    use openmm_data::blv::DoorState;

    let door_runtimes: Vec<DoorRuntime> = prepared
        .doors
        .iter()
        .map(|door| DoorRuntime {
            door_id: door.door_id,
            direction: door.direction,
            move_length: door.move_length,
            open_speed: door.open_speed,
            close_speed: door.close_speed,
            state: door.state,
            time_since_triggered_ms: match door.state {
                DoorState::Open | DoorState::Closed => {
                    if door.move_length > 0 && door.open_speed > 0 {
                        (door.move_length as f32 / door.open_speed as f32) * 1000.0
                    } else {
                        0.0
                    }
                }
                _ => 0.0,
            },
        })
        .collect();

    BlvDoors { doors: door_runtimes }
}

/// Build DoorColliders from precomputed door collision geometry.
///
/// Includes invisible blocking surfaces excluded from door_face_meshes. Uses stored
/// BLV normals for correct classification — cross-product from base positions gives
/// wrong normals when all base vertices share the same Y (floor-level retracted position).
/// Wall-like faces (normal.y < 0.7) -> collision walls blocking XZ movement.
/// Horizontal faces (normal.y >= 0.7) -> dynamic ceiling panels blocking vertical passage.
fn build_door_colliders(prepared: &PreparedIndoorWorld) -> DoorColliders {
    let mut door_collision_faces = Vec::new();
    let mut door_horizontal_faces = Vec::new();
    for dc in &prepared.door_collision_geometry {
        if dc.base_positions.len() < 3 {
            continue;
        }
        if dc.normal.y.abs() > 0.7 {
            door_horizontal_faces.push(DoorCollisionFace {
                door_index: dc.door_index,
                base_positions: dc.base_positions.clone(),
                normal: dc.normal,
                is_moving: dc.is_moving.clone(),
            });
        } else {
            door_collision_faces.push(DoorCollisionFace {
                door_index: dc.door_index,
                base_positions: dc.base_positions.clone(),
                normal: dc.normal,
                is_moving: dc.is_moving.clone(),
            });
        }
    }
    DoorColliders {
        face_data: door_collision_faces,
        horizontal_face_data: door_horizontal_faces,
        walls: Vec::new(),
        dynamic_ceilings: Vec::new(),
    }
}

/// Build clickable face resource for indoor interaction raycasts.
fn build_clickable_faces(prepared: &PreparedIndoorWorld) -> crate::game::interaction::clickable::Faces {
    let faces: Vec<crate::game::interaction::clickable::FaceInfo> = prepared
        .clickable_faces
        .iter()
        .map(|cf| crate::game::interaction::clickable::FaceInfo {
            face_index: cf.face_index,
            event_id: cf.event_id,
            normal: cf.normal,
            plane_dist: cf.plane_dist,
            vertices: cf.vertices.clone(),
        })
        .collect();
    crate::game::interaction::clickable::Faces { faces, is_indoor: true }
}

/// Build OccluderFaces resource — all solid indoor geometry for ray occlusion.
fn build_occluder_faces(prepared: &PreparedIndoorWorld) -> OccluderFaces {
    let occ_faces: Vec<OccluderFaceInfo> = prepared
        .occluder_faces
        .iter()
        .map(|f| OccluderFaceInfo {
            normal: f.normal,
            plane_dist: f.plane_dist,
            vertices: f.vertices.clone(),
        })
        .collect();
    OccluderFaces::new(occ_faces)
}

/// Build TouchTriggerFaces resource for proximity-based event dispatch.
fn build_touch_triggers(prepared: &PreparedIndoorWorld) -> TouchTriggerFaces {
    let touch_faces: Vec<TouchTriggerInfo> = prepared
        .touch_trigger_faces
        .iter()
        .map(|tf| TouchTriggerInfo {
            event_id: tf.event_id,
            center: tf.center,
            radius: tf.radius,
        })
        .collect();
    if !touch_faces.is_empty() {
        info!("Indoor map has {} touch-trigger faces", touch_faces.len());
    }
    TouchTriggerFaces {
        faces: touch_faces,
        fired: std::collections::HashSet::new(),
    }
}

/// Spawn near-zero ambient light so dungeons are dark by default.
/// The party torch (point light on the player) provides local illumination.
fn spawn_ambient_light(commands: &mut Commands) {
    commands.spawn((
        AmbientLight {
            color: Color::srgb(0.05, 0.04, 0.03),
            brightness: 0.5,
            ..default()
        },
        InGame,
    ));
}

/// Spawn decorations from BLV decoration list using the shared spawn_decoration helper.
fn spawn_decorations(
    prepared: &PreparedIndoorWorld,
    commands: &mut Commands,
    images: &mut Assets<Image>,
    meshes: &mut Assets<Mesh>,
    sprite_materials: &mut Assets<SpriteMaterial>,
    game_assets: &crate::assets::GameAssets,
    cfg: &crate::config::GameConfig,
) {
    let mut sprite_cache = sprites::SpriteCache::default();
    let mut dec_sprite_cache = crate::game::spawn::decoration::DecSpriteCache::default();

    let mut ctx = SpawnCtx {
        game_assets,
        images,
        meshes,
        sprite_materials,
        sprite_cache: &mut sprite_cache,
        shadows: cfg.shadows,
        billboard_shadows: cfg.billboard_shadows,
        actor_shadows: cfg.actor_shadows,
    };

    for dec in prepared.decorations.entries() {
        let dec_pos = Vec3::from(mm6_position_to_bevy(dec.position[0], dec.position[1], dec.position[2]));
        crate::game::spawn::decoration::spawn_decoration(commands, &mut ctx, dec, dec_pos, None, &mut dec_sprite_cache);
    }
}

/// Spawn BLV static point lights (designer-placed lights for campfires, cauldrons, etc.).
/// radius field is always 0 in MM6 data — brightness alone drives the falloff.
fn spawn_blv_lights(prepared: &PreparedIndoorWorld, commands: &mut Commands) {
    // Range and intensity are decoupled: range scales linearly so small lights don't get
    // a range boost from a high-intensity formula.
    // brightness=64 -> range~960 (small torch); brightness=640 -> range~9600 (campfire room-fill).
    const BLV_LIGHT_RANGE_SCALE: f32 = 5.0;
    const BLV_LIGHT_INTENSITY_SCALE: f32 = 300.0;
    for &(pos, brightness) in &prepared.blv_lights {
        let b = brightness as f32;
        let range = b * BLV_LIGHT_RANGE_SCALE;
        let intensity = b * b * BLV_LIGHT_INTENSITY_SCALE;
        commands.spawn((
            PointLight {
                color: Color::srgb(1.0, 0.76, 0.38),
                intensity,
                range,
                shadows_enabled: false, //cfg.shadows,
                ..default()
            },
            Transform::from_translation(pos),
            InGame,
        ));
    }
}

/// Spawn BLV monsters from spawn_points (same pipeline as ODM).
fn spawn_indoor_monsters(
    prepared: &PreparedIndoorWorld,
    commands: &mut Commands,
    images: &mut Assets<Image>,
    meshes: &mut Assets<Mesh>,
    sprite_materials: &mut Assets<SpriteMaterial>,
    game_assets: &crate::assets::GameAssets,
    cfg: &crate::config::GameConfig,
) {
    let mut sprite_cache = sprites::SpriteCache::default();
    let Some(ref monsters) = prepared.resolved_actors else {
        return;
    };

    let mut ctx = SpawnCtx {
        game_assets,
        images,
        meshes,
        sprite_materials,
        sprite_cache: &mut sprite_cache,
        shadows: cfg.shadows,
        billboard_shadows: cfg.billboard_shadows,
        actor_shadows: cfg.actor_shadows,
    };

    for mon in monsters.iter() {
        // Spread group members using golden angle (same as ODM, no terrain probe for indoor).
        let angle = mon.group_index as f32 * 2.399_f32;
        let r = mon.spawn_radius as f32;
        let [bx, by, bz] = mm6_position_to_bevy(
            mon.spawn_position[0] + (r * angle.cos()) as i32,
            mon.spawn_position[1] + (r * angle.sin()) as i32,
            mon.spawn_position[2],
        );
        let ground_pos = Vec3::new(bx, by, bz);

        let params = ActorSpawnParams {
            kind: ActorKind::Monster,
            name: &mon.name,
            standing_sprite: &mon.standing_sprite,
            walking_sprite: &mon.walking_sprite,
            attacking_sprite: &mon.attacking_sprite,
            dying_sprite: &mon.dying_sprite,
            variant: mon.variant,
            palette_id: mon.palette_id,
            ground_pos,
            hp: mon.hp,
            move_speed: mon.move_speed as f32,
            sound_ids: mon.sound_ids,
            tether_distance: mon.radius as f32 * 2.0,
            attack_range: mon.body_radius as f32,
            aggro_range: mon.aggro_range,
            recovery_secs: mon.recovery_secs,
            can_fly: mon.can_fly,
            ai_type: &mon.ai_type,
            ddm_id: -1,
            group_id: 0,
            hostile: true,
        };
        if spawn_actor(commands, &mut ctx, &params, None).is_some() {
            info!("Spawned indoor monster '{}' at {:?}", mon.name, ground_pos);
        }
    }
}
