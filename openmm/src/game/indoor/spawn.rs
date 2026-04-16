//! Indoor world spawning — meshes, doors, decorations, lights, monsters.

use bevy::prelude::*;

use openmm_data::provider::decorations::DecorationEntry;
use openmm_data::provider::lod_decoder::LodDecoder;

use crate::game::InGame;
use crate::game::actors::{Actor, ActorParams, MonsterAiType};
use crate::game::coords::mm6_position_to_bevy;
use crate::game::sprites::SelfLit;
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

/// Spawn decorations from BLV decoration list.
/// Dispatches to directional, animated, or static helpers per decoration type.
fn spawn_decorations(
    prepared: &PreparedIndoorWorld,
    commands: &mut Commands,
    images: &mut Assets<Image>,
    meshes: &mut Assets<Mesh>,
    sprite_materials: &mut Assets<SpriteMaterial>,
    game_assets: &crate::assets::GameAssets,
    cfg: &crate::config::GameConfig,
) {
    let lod = game_assets.lod();
    let mut sprite_cache = sprites::SpriteCache::default();
    for dec in prepared.decorations.entries() {
        let dec_pos = Vec3::from(mm6_position_to_bevy(dec.position[0], dec.position[1], dec.position[2]));
        let key = &dec.sprite_name;
        // sprite_center is set in each branch to the entity's world position (dec_pos + half-height).
        // Used afterwards to place the point light at the sprite centre, not the floor.
        let sprite_center;
        let dec_entity;

        if dec.is_directional {
            let result = spawn_directional_decoration(
                dec,
                dec_pos,
                key,
                commands,
                images,
                meshes,
                sprite_materials,
                game_assets.assets(),
                &lod,
                cfg,
                &mut sprite_cache,
            );
            let Some((center, entity)) = result else { continue };
            sprite_center = center;
            dec_entity = Some(entity);
        } else if dec.num_frames > 1 {
            let result =
                spawn_animated_decoration(dec, dec_pos, key, commands, images, meshes, sprite_materials, cfg, &lod);
            let Some((center, entity)) = result else { continue };
            sprite_center = center;
            dec_entity = Some(entity);
        } else {
            let result =
                spawn_static_decoration(dec, dec_pos, key, commands, images, meshes, sprite_materials, cfg, &lod);
            let Some((center, entity)) = result else { continue };
            sprite_center = center;
            dec_entity = Some(entity);
        }

        // Common: ddeclist point light + selflit marker.
        if dec.light_radius > 0 {
            commands.spawn((
                crate::game::rendering::lighting::decoration_point_light(
                    crate::game::rendering::lighting::DecorationLight::Ddeclist(dec.light_radius),
                    false,
                ),
                Transform::from_translation(sprite_center),
                InGame,
            ));
            // Mark this decoration sprite as self-lit so it isn't dimmed by the tint system.
            if let Some(id) = dec_entity {
                commands.entity(id).insert(SelfLit);
            }
        }
    }
}

/// Spawn a directional decoration. Returns (sprite_center, entity_id) or None if skipped.
fn spawn_directional_decoration(
    dec: &DecorationEntry,
    dec_pos: Vec3,
    key: &str,
    commands: &mut Commands,
    images: &mut Assets<Image>,
    meshes: &mut Assets<Mesh>,
    sprite_materials: &mut Assets<SpriteMaterial>,
    assets: &openmm_data::Assets,
    lod: &LodDecoder<'_>,
    cfg: &crate::config::GameConfig,
    sprite_cache: &mut sprites::SpriteCache,
) -> Option<(Vec3, Entity)> {
    // Indoor directionals with a ddeclist light are selflit.
    let is_selflit = dec.light_radius > 0;
    let (dirs, dir_masks, px_w, px_h) = sprites::load_decoration_directions(
        key,
        assets,
        images,
        sprite_materials,
        &mut Some(sprite_cache),
        is_selflit,
    );
    if px_w == 0.0 {
        return None;
    }
    let dsft_scale = lod.dsft_scale_for_group(key);
    let sw = px_w * dsft_scale;
    let sh = px_h * dsft_scale;
    let initial_mat = dirs[0].clone();
    let quad = meshes.add(Rectangle::new(sw, sh));
    let pos = dec_pos + Vec3::new(0.0, sh / 2.0, 0.0);
    let states = vec![vec![dirs]];
    let state_masks = vec![vec![dir_masks]];
    let ent_id = commands
        .spawn((
            Name::new(format!("decoration:{}", key)),
            Mesh3d(quad),
            MeshMaterial3d(initial_mat),
            Transform::from_translation(pos),
            crate::game::sprites::WorldEntity,
            crate::game::sprites::EntityKind::Decoration,
            crate::game::sprites::Billboard,
            crate::game::sprites::AnimationState::Idle,
            sprites::SpriteSheet::new(states, vec![(sw, sh)], state_masks),
            crate::game::sprites::FacingYaw(dec.facing_yaw),
            InGame,
        ))
        .id();
    crate::game::sprites::apply_shadow_config(commands, ent_id, cfg.billboard_shadows);
    commands
        .entity(ent_id)
        .insert(crate::game::interaction::DecorationInfo::from_entry(
            dec, pos, dec_pos.y, 0.0, 0.0, None,
        ));
    Some((pos, ent_id))
}

/// Spawn an animated decoration. Returns (sprite_center, entity_id) or None if skipped.
fn spawn_animated_decoration(
    dec: &DecorationEntry,
    dec_pos: Vec3,
    key: &str,
    commands: &mut Commands,
    images: &mut Assets<Image>,
    meshes: &mut Assets<Mesh>,
    sprite_materials: &mut Assets<SpriteMaterial>,
    cfg: &crate::config::GameConfig,
    lod: &LodDecoder<'_>,
) -> Option<(Vec3, Entity)> {
    let frame_sprites = lod.billboard_animation_frames(key, dec.declist_id);
    if frame_sprites.is_empty() {
        return None;
    }
    let (w, h) = frame_sprites[0].dimensions();
    if w == 0.0 || h == 0.0 {
        return None;
    }
    let quad = meshes.add(Rectangle::new(w, h));
    let pos = dec_pos + Vec3::new(0.0, h / 2.0, 0.0);
    // All indoor animated decorations (torches, campfires, cauldrons)
    // are flame sources -> always selflit.
    let mut frame_mats = vec![];
    let mut frame_masks = vec![];
    for sprite in &frame_sprites {
        let rgba = sprite.image.to_rgba8();
        let (mat, msk) = sprites::sprite_to_material_with_mask(rgba, images, sprite_materials, true);
        frame_mats.push(std::array::from_fn(|_| mat.clone()));
        frame_masks.push(std::array::from_fn(|_| msk.clone()));
    }
    let initial_mat = frame_mats[0][0].clone();
    let mut sheet = sprites::SpriteSheet::new(vec![frame_mats], vec![(w, h)], vec![frame_masks]);
    sheet.frame_duration = dec.frame_duration;
    let ent_id = commands
        .spawn((
            Name::new(format!("decoration:{}", key)),
            Mesh3d(quad),
            MeshMaterial3d(initial_mat),
            Transform::from_translation(pos),
            crate::game::sprites::WorldEntity,
            crate::game::sprites::EntityKind::Decoration,
            crate::game::sprites::Billboard,
            crate::game::sprites::AnimationState::Idle,
            sheet,
            InGame,
        ))
        .id();
    crate::game::sprites::apply_shadow_config(commands, ent_id, cfg.billboard_shadows);
    commands
        .entity(ent_id)
        .insert(crate::game::interaction::DecorationInfo::from_entry(
            dec, pos, dec_pos.y, 0.0, 0.0, None,
        ));
    // Animated flame decorations do NOT get DecorFlicker — the frame cycling
    // is already the visual effect. Adding visibility toggle on top makes them
    // blink entirely off, which looks like a bug.
    // All animated indoor decorations (torches, campfires, cauldrons) are fire
    // sources — mark them so the tint system skips them.
    commands.entity(ent_id).insert(SelfLit);

    // Luminous animated decorations (campfires, braziers) carry their point-light
    // radius in the DSFT frame, not in the ddeclist.light_radius field.
    // Campfireon: DSFT light_radius=256, is_luminous=true.
    let dsft_lr = lod.billboard_luminous_light_radius(dec.declist_id);
    if dsft_lr > 0 {
        commands.spawn((
            crate::game::rendering::lighting::decoration_point_light(
                crate::game::rendering::lighting::DecorationLight::AnimatedDsft(dsft_lr),
                false,
            ),
            Transform::from_translation(pos),
            InGame,
        ));
    }

    Some((pos, ent_id))
}

/// Spawn a static single-frame decoration. Returns (sprite_center, entity_id) or None if skipped.
fn spawn_static_decoration(
    dec: &DecorationEntry,
    dec_pos: Vec3,
    key: &str,
    commands: &mut Commands,
    images: &mut Assets<Image>,
    meshes: &mut Assets<Mesh>,
    sprite_materials: &mut Assets<SpriteMaterial>,
    cfg: &crate::config::GameConfig,
    lod: &LodDecoder<'_>,
) -> Option<(Vec3, Entity)> {
    let sprite = lod.billboard(key, dec.declist_id)?;
    let dsft_lr = lod.billboard_luminous_light_radius(dec.declist_id);
    let (w, h) = sprite.dimensions();
    if w == 0.0 || h == 0.0 {
        return None;
    }
    // Luminous DSFT statics (chandeliers, crystals, sconces) and
    // ddeclist-lit decorations are selflit.
    let is_selflit = dec.light_radius > 0 || dsft_lr > 0;
    let rgba = sprite.image.to_rgba8();
    let (mat, mask) = sprites::sprite_to_material_with_mask(rgba, images, sprite_materials, is_selflit);
    let quad = meshes.add(Rectangle::new(w, h));
    let pos = dec_pos + Vec3::new(0.0, h / 2.0, 0.0);
    let ent_id = commands
        .spawn((
            Name::new(format!("decoration:{}", key)),
            Mesh3d(quad),
            MeshMaterial3d(mat),
            Transform::from_translation(pos),
            crate::game::sprites::WorldEntity,
            crate::game::sprites::EntityKind::Decoration,
            crate::game::sprites::Billboard,
            crate::game::sprites::AnimationState::Idle,
            InGame,
        ))
        .id();
    crate::game::sprites::apply_shadow_config(commands, ent_id, cfg.billboard_shadows);
    commands
        .entity(ent_id)
        .insert(crate::game::interaction::DecorationInfo::from_entry(
            dec,
            pos,
            dec_pos.y,
            w / 2.0,
            h / 2.0,
            Some(mask),
        ));
    // DSFT-luminous static decs (chandeliers, crystals, sconces) get SelfLit + light.
    if dsft_lr > 0 {
        commands.entity(ent_id).insert(SelfLit);
    }
    if dsft_lr > 0 {
        commands.spawn((
            crate::game::rendering::lighting::decoration_point_light(
                crate::game::rendering::lighting::DecorationLight::StaticDsft(dsft_lr),
                false,
            ),
            Transform::from_translation(pos),
            InGame,
        ));
    }

    Some((pos, ent_id))
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
    let lod = game_assets.lod();
    let mut sprite_cache = sprites::SpriteCache::default();
    let Some(ref monsters) = prepared.resolved_actors else {
        return;
    };

    for mon in monsters.iter() {
        let (states, state_masks, raw_w, raw_h) = sprites::load_entity_sprites(
            &mon.standing_sprite,
            &mon.walking_sprite,
            &mon.attacking_sprite,
            &mon.dying_sprite,
            game_assets.assets(),
            images,
            sprite_materials,
            &mut Some(&mut sprite_cache),
            mon.variant,
            mon.palette_id,
            false,
        );
        if states.is_empty() || states[0].is_empty() {
            error!(
                "Indoor monster '{}' sprite '{}' failed to load — skipping",
                mon.name, mon.standing_sprite
            );
            continue;
        }
        let dsft_scale = lod.dsft_scale_for_group(&mon.standing_sprite);
        let sw = raw_w * dsft_scale;
        let sh = raw_h * dsft_scale;
        let state_count = states.len();
        let initial_mat = states[0][0][0].clone();
        let quad = meshes.add(Rectangle::new(sw, sh));

        // Spread group members using golden angle (same as ODM, no terrain probe for indoor).
        let angle = mon.group_index as f32 * 2.399_f32;
        let r = mon.spawn_radius as f32;
        let [bx, by, bz] = mm6_position_to_bevy(
            mon.spawn_position[0] + (r * angle.cos()) as i32,
            mon.spawn_position[1] + (r * angle.sin()) as i32,
            mon.spawn_position[2],
        );
        let pos = Vec3::new(bx, by + sh / 2.0, bz);

        let mon_id = commands
            .spawn((
                Name::new(format!("monster:{}", mon.name)),
                Mesh3d(quad),
                MeshMaterial3d(initial_mat),
                Transform::from_translation(pos),
                crate::game::sprites::WorldEntity,
                crate::game::sprites::EntityKind::Monster,
                crate::game::sprites::AnimationState::Idle,
                sprites::SpriteSheet::new(states, vec![(sw, sh); state_count], state_masks),
                crate::game::interaction::MonsterInteractable { name: mon.name.clone() },
                crate::game::actors::MonsterAiMode::Wander,
                Actor::new(ActorParams {
                    name: mon.name.clone(),
                    hp: mon.hp,
                    move_speed: mon.move_speed as f32,
                    position: pos,
                    hostile: true,
                    variant: mon.variant,
                    sound_ids: mon.sound_ids,
                    tether_distance: mon.radius as f32 * 2.0,
                    attack_range: mon.body_radius as f32 * 2.0,
                    ddm_id: -1,
                    group_id: 0,
                    aggro_range: mon.aggro_range,
                    recovery_secs: mon.recovery_secs,
                    sprite_half_height: sh / 2.0,
                    can_fly: mon.can_fly,
                    ai_type: MonsterAiType::from_str(&mon.ai_type),
                }),
                crate::game::sprites::Billboard,
                InGame,
            ))
            .id();
        crate::game::sprites::apply_shadow_config(commands, mon_id, cfg.actor_shadows);
        info!("Spawned indoor monster '{}' at {:?}", mon.name, pos);
    }
}
