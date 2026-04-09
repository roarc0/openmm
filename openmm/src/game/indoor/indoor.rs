//! Indoor (BLV) map spawner — the indoor equivalent of odm.rs.
//!
//! When the loading pipeline produces a `PreparedIndoorWorld`, this plugin
//! spawns the face-based geometry, door entities, and ambient lighting.
//! Also handles indoor face interaction (click/Enter) and door animation.

use bevy::light::NotShadowCaster;
use bevy::mesh::VertexAttributeValues;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use openmm_data::blv::DoorState;

use crate::game::coords::mm6_position_to_bevy;

use crate::game::InGame;
use crate::game::actors::actor;
use crate::game::interaction::raycast::{point_in_polygon, ray_plane_intersect};
use crate::game::player::PlayerCamera;
use crate::game::sprites::SelfLit;
use crate::game::sprites::loading as sprites;
use crate::game::sprites::material::{SpriteMaterial, unlit_billboard_material};
use crate::game::world::EventQueue;
use crate::game::world::MapEvents;
use crate::states::loading::PreparedIndoorWorld;

// --- Components ---

/// Marker component on door face entities for animation.
#[derive(Component)]
pub struct DoorFace {
    pub door_index: usize,
    pub face_index: usize,
    /// Per triangle-vertex: whether it moves with the door.
    pub is_moving_vertex: Vec<bool>,
    /// Base vertex positions (Bevy coords) at door distance=0.
    /// Used directly for animation: moved_pos = base_pos + direction_bevy * distance.
    pub base_positions: Vec<[f32; 3]>,
    /// UV change per unit of door displacement for moving vertices.
    pub uv_rate: [f32; 2],
    /// Base UV values per triangle vertex (at distance=0).
    pub base_uvs: Vec<[f32; 2]>,
    /// Whether this face has the MOVES_BY_DOOR (FACE_TexMoveByDoor) attribute.
    pub moves_by_door: bool,
}

// --- Resources ---

/// Runtime state for a single door.
pub struct DoorRuntime {
    pub door_id: u32,
    /// Direction vector in MM6 coordinates.
    pub direction: [f32; 3],
    pub move_length: i32,
    pub open_speed: i32,
    pub close_speed: i32,
    pub state: DoorState,
    /// Milliseconds since last state change.
    pub time_since_triggered_ms: f32,
}

/// Resource holding all door runtime states for the current indoor map.
#[derive(Resource)]
pub struct BlvDoors {
    pub doors: Vec<DoorRuntime>,
}

/// Collision data for a single door face — a polygon that blocks movement.
pub struct DoorCollisionFace {
    pub door_index: usize,
    /// Base vertex positions (Bevy coords) at door distance=0.
    pub base_positions: Vec<Vec3>,
    /// Face normal in Bevy coords.
    pub normal: Vec3,
    /// Per-vertex: whether this vertex moves with the door.
    pub is_moving: Vec<bool>,
}

/// Dynamic collision resource for door faces.
/// Updated each frame by the door animation system.
#[derive(Resource, Default)]
pub struct DoorColliders {
    /// Source data for wall-like door faces (normal mostly horizontal).
    pub face_data: Vec<DoorCollisionFace>,
    /// Source data for horizontal door faces (floor/ceiling panels that block passage).
    pub horizontal_face_data: Vec<DoorCollisionFace>,
    /// Current collision walls (rebuilt from face_data + door positions).
    pub walls: Vec<crate::game::collision::CollisionWall>,
    /// Current dynamic ceiling triangles (rebuilt from horizontal_face_data when closed).
    /// Used to block the player from walking through closed horizontal panels.
    pub dynamic_ceilings: Vec<crate::game::collision::CollisionTriangle>,
}

impl DoorColliders {
    /// Returns true if the player body (from feet_y to eye_y, radius `r`) intersects a
    /// closed horizontal door panel. Used to block entry into areas closed off by rising
    /// or lowering door panels.
    ///
    /// Uses the panel AABB expanded by `r` so narrow panels (< 2*radius wide) still block
    /// the player even if their center never enters the exact triangle footprint.
    pub fn blocks_entry(&self, x: f32, z: f32, feet_y: f32, eye_y: f32, r: f32) -> bool {
        for ceil in &self.dynamic_ceilings {
            if !ceil.near_xz(x, z, r) {
                continue;
            }
            // Sample panel height at center; for near-edge (not exactly inside), approximate
            // with centroid. For horizontal panels all vertices have the same Y so this is exact.
            let panel_y = ceil
                .height_at_xz(x, z)
                .unwrap_or_else(|| (ceil.v0.y + ceil.v1.y + ceil.v2.y) / 3.0);
            // Panel between feet and eyes → body intersects it → blocked
            if panel_y > feet_y && panel_y < eye_y {
                return true;
            }
        }
        false
    }

    /// Push the player out of any door wall they would penetrate.
    /// Same algorithm as BuildingColliders::resolve_movement.
    pub fn resolve_movement(&self, from: Vec3, to: Vec3, radius: f32, eye_height: f32) -> Vec3 {
        let mut result = to;
        let feet_y = from.y - eye_height;

        for _ in 0..3 {
            let prev = result;

            for wall in &self.walls {
                if feet_y > wall.max_y || from.y < wall.min_y {
                    continue;
                }
                // Check if player is within the face's XZ footprint
                if result.x + radius < wall.min_x
                    || result.x - radius > wall.max_x
                    || result.z + radius < wall.min_z
                    || result.z - radius > wall.max_z
                {
                    continue;
                }

                let dist_to = wall.normal.dot(result) - wall.plane_dist;
                if dist_to < radius && dist_to > -radius {
                    // We are penetrating or near the plane. Push out towards the side we came from.
                    let dist_from = wall.normal.dot(from) - wall.plane_dist;
                    let push = if dist_from >= 0.0 {
                        radius - dist_to
                    } else {
                        -radius - dist_to
                    };
                    result.x += wall.normal.x * push;
                    result.z += wall.normal.z * push;
                }
            }

            if (result.x - prev.x).abs() < 0.1 && (result.z - prev.z).abs() < 0.1 {
                break;
            }
        }

        result
    }
}

/// Info about a single solid face used for ray occlusion.
pub struct OccluderFaceInfo {
    pub normal: Vec3,
    pub plane_dist: f32,
    pub vertices: Vec<Vec3>,
}

/// Resource holding all solid face geometry for ray-occlusion tests.
///
/// Present for both outdoor (BSP model faces) and indoor (BLV wall/floor/ceiling faces)
/// maps. Used by hover and interact systems to gate hits — an NPC or decoration
/// behind a building wall should not be targetable.
#[derive(Resource, Default)]
pub struct OccluderFaces {
    pub faces: Vec<OccluderFaceInfo>,
}

impl OccluderFaces {
    /// Returns the smallest `t` along `(origin, dir)` that hits any solid face,
    /// or `f32::MAX` if the ray misses all faces.
    pub fn min_hit_t(&self, origin: Vec3, dir: Vec3) -> f32 {
        use crate::game::interaction::raycast::{point_in_polygon, ray_plane_intersect};
        let mut min_t = f32::MAX;
        for face in &self.faces {
            if let Some(t) = ray_plane_intersect(origin, dir, face.normal, face.plane_dist)
                && t < min_t
            {
                let hit = origin + dir * t;
                if point_in_polygon(hit, &face.vertices, face.normal) {
                    min_t = t;
                }
            }
        }
        min_t
    }
}

/// Info about a single touch-triggered indoor face.
pub struct TouchTriggerInfo {
    pub event_id: u16,
    pub center: Vec3,
    pub radius: f32,
}

/// Resource holding touch-triggered face data for the current indoor map.
#[derive(Resource)]
pub struct TouchTriggerFaces {
    pub faces: Vec<TouchTriggerInfo>,
    /// Track which events were already fired to avoid repeating every frame.
    pub fired: std::collections::HashSet<u16>,
}

// --- Spawn ---

pub(crate) fn spawn_indoor_world(
    prepared: Option<Res<PreparedIndoorWorld>>,
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut sprite_materials: ResMut<Assets<SpriteMaterial>>,
    game_assets: Res<crate::assets::GameAssets>,
    cfg: Res<crate::config::GameConfig>,
    tint_buffers: Res<crate::game::sprites::tint_buffer::SpriteTintBuffers>,
) {
    let Some(prepared) = prepared else { return };

    // Spawn all static face meshes (grouped by texture)
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

    // Spawn door face entities individually (for animation)
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

    // Build BlvDoors resource from prepared door data.
    // Preserve indices so DoorFace.door_index matches directly.
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

    commands.insert_resource(BlvDoors { doors: door_runtimes });

    // Build DoorColliders from precomputed door collision geometry (includes invisible
    // blocking surfaces excluded from door_face_meshes). Uses stored BLV normals for
    // correct classification — cross-product from base positions gives wrong normals
    // when all base vertices share the same Y (floor-level retracted position).
    // Wall-like faces (normal.y < 0.7) → collision walls blocking XZ movement.
    // Horizontal faces (normal.y >= 0.7) → dynamic ceiling panels blocking vertical passage.
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
    commands.insert_resource(DoorColliders {
        face_data: door_collision_faces,
        horizontal_face_data: door_horizontal_faces,
        walls: Vec::new(),
        dynamic_ceilings: Vec::new(),
    });

    // Build ClickableFaces resource
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
    commands.insert_resource(crate::game::interaction::clickable::Faces { faces, is_indoor: true });

    // Build OccluderFaces resource — all solid indoor geometry for ray occlusion.
    let occ_faces: Vec<OccluderFaceInfo> = prepared
        .occluder_faces
        .iter()
        .map(|f| OccluderFaceInfo {
            normal: f.normal,
            plane_dist: f.plane_dist,
            vertices: f.vertices.clone(),
        })
        .collect();
    commands.insert_resource(OccluderFaces { faces: occ_faces });

    // Build TouchTriggerFaces resource
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
    commands.insert_resource(TouchTriggerFaces {
        faces: touch_faces,
        fired: std::collections::HashSet::new(),
    });

    // Indoor: near-zero ambient so dungeons are dark by default.
    // The party torch (point light on the player) provides local illumination.
    commands.spawn((
        AmbientLight {
            color: Color::srgb(0.05, 0.04, 0.03),
            brightness: 0.5,
            ..default()
        },
        InGame,
    ));

    // Spawn decorations from BLV decoration list.
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
            // Indoor directionals with a ddeclist light are selflit.
            let tint_handle = if dec.light_radius > 0 {
                tint_buffers.selflit.clone()
            } else {
                tint_buffers.regular.clone()
            };
            let (dirs, dir_masks, px_w, px_h) = sprites::load_decoration_directions(
                key,
                game_assets.assets(),
                &mut images,
                &mut sprite_materials,
                &mut Some(&mut sprite_cache),
                &tint_handle,
            );
            if px_w == 0.0 {
                continue;
            }
            let dsft_scale = lod.dsft_scale_for_group(key);
            let sw = px_w * dsft_scale;
            let sh = px_h * dsft_scale;
            let initial_mat = dirs[0].clone();
            let quad = meshes.add(Rectangle::new(sw, sh));
            let pos = dec_pos + Vec3::new(0.0, sh / 2.0, 0.0);
            sprite_center = pos;
            let states = vec![vec![dirs]];
            let state_masks = vec![vec![dir_masks]];
            let mut ent = commands.spawn((
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
            ));
            if !cfg.billboard_shadows {
                ent.insert(NotShadowCaster);
            }
            if dec.event_id > 0 {
                ent.insert(crate::game::interaction::DecorationInfo {
                    event_id: dec.event_id as u16,
                    position: pos,
                    billboard_index: dec.billboard_index,
                    declist_id: dec.declist_id,
                    ground_y: dec_pos.y,
                    half_w: 0.0,
                    half_h: 0.0,
                    mask: None,
                });
            }
            dec_entity = Some(ent.id());
        } else if dec.num_frames > 1 {
            // Animated decoration
            let frame_sprites = lod.billboard_animation_frames(key, dec.declist_id);
            if frame_sprites.is_empty() {
                continue;
            }
            let (w, h) = frame_sprites[0].dimensions();
            if w == 0.0 || h == 0.0 {
                continue;
            }
            let quad = meshes.add(Rectangle::new(w, h));
            let pos = dec_pos + Vec3::new(0.0, h / 2.0, 0.0);
            sprite_center = pos;
            // All indoor animated decorations (torches, campfires, cauldrons)
            // are flame sources → always selflit.
            let tint_handle = tint_buffers.selflit.clone();
            let mut frame_mats = vec![];
            let mut frame_masks = vec![];
            for sprite in &frame_sprites {
                let rgba = sprite.image.to_rgba8();
                let msk = std::sync::Arc::new(sprites::AlphaMask::from_image(&rgba));
                let tex = images.add(crate::assets::rgba8_to_bevy_image(rgba));
                let mat = sprite_materials.add(unlit_billboard_material(tex, tint_handle.clone()));
                frame_mats.push(std::array::from_fn(|_| mat.clone()));
                frame_masks.push(std::array::from_fn(|_| msk.clone()));
            }
            let initial_mat = frame_mats[0][0].clone();
            let mut sheet = sprites::SpriteSheet::new(vec![frame_mats], vec![(w, h)], vec![frame_masks]);
            sheet.frame_duration = dec.frame_duration;
            let mut ent = commands.spawn((
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
            ));
            if !cfg.billboard_shadows {
                ent.insert(NotShadowCaster);
            }
            if dec.event_id > 0 {
                ent.insert(crate::game::interaction::DecorationInfo {
                    event_id: dec.event_id as u16,
                    position: pos,
                    billboard_index: dec.billboard_index,
                    declist_id: dec.declist_id,
                    ground_y: dec_pos.y,
                    half_w: 0.0,
                    half_h: 0.0,
                    mask: None,
                });
            }
            // Animated flame decorations do NOT get DecorFlicker — the frame cycling
            // is already the visual effect. Adding visibility toggle on top makes them
            // blink entirely off, which looks like a bug.
            // All animated indoor decorations (torches, campfires, cauldrons) are fire
            // sources — mark them so the tint system skips them.
            ent.insert(SelfLit);
            let ent_id = ent.id();
            dec_entity = Some(ent_id);

            // Luminous animated decorations (campfires, braziers) carry their point-light
            // radius in the DSFT frame, not in the ddeclist.light_radius field.
            // Campfireon: DSFT light_radius=256, is_luminous=true.
            let dsft_lr = lod.billboard_luminous_light_radius(dec.declist_id);
            if dsft_lr > 0 {
                commands.spawn((
                    crate::game::lighting::decoration_point_light(
                        crate::game::lighting::DecorationLight::AnimatedDsft(dsft_lr),
                        false,
                    ),
                    Transform::from_translation(sprite_center),
                    InGame,
                ));
            }
        } else {
            // Static single-frame decoration
            let Some(sprite) = lod.billboard(key, dec.declist_id) else {
                continue;
            };
            let dsft_lr = lod.billboard_luminous_light_radius(dec.declist_id);
            let (w, h) = sprite.dimensions();
            if w == 0.0 || h == 0.0 {
                continue;
            }
            // Luminous DSFT statics (chandeliers, crystals, sconces) and
            // ddeclist-lit decorations are selflit.
            let is_selflit = dec.light_radius > 0 || dsft_lr > 0;
            let tint_handle = if is_selflit {
                tint_buffers.selflit.clone()
            } else {
                tint_buffers.regular.clone()
            };
            let rgba = sprite.image.to_rgba8();
            let mask = sprites::AlphaMask::from_image(&rgba);
            let tex = images.add(crate::assets::rgba8_to_bevy_image(rgba));
            let mat = sprite_materials.add(unlit_billboard_material(tex, tint_handle));
            let quad = meshes.add(Rectangle::new(w, h));
            let pos = dec_pos + Vec3::new(0.0, h / 2.0, 0.0);
            sprite_center = pos;
            let mut ent = commands.spawn((
                Name::new(format!("decoration:{}", key)),
                Mesh3d(quad),
                MeshMaterial3d(mat),
                Transform::from_translation(pos),
                crate::game::sprites::WorldEntity,
                crate::game::sprites::EntityKind::Decoration,
                crate::game::sprites::Billboard,
                crate::game::sprites::AnimationState::Idle,
                InGame,
            ));
            if !cfg.billboard_shadows {
                ent.insert(NotShadowCaster);
            }
            if dec.event_id > 0 {
                ent.insert(crate::game::interaction::DecorationInfo {
                    event_id: dec.event_id as u16,
                    position: pos,
                    billboard_index: dec.billboard_index,
                    declist_id: dec.declist_id,
                    ground_y: dec_pos.y,
                    half_w: w / 2.0,
                    half_h: h / 2.0,
                    mask: Some(std::sync::Arc::new(mask)),
                });
            }
            // DSFT-luminous static decs (chandeliers, crystals, sconces) get SelfLit + light.
            if dsft_lr > 0 {
                ent.insert(SelfLit);
            }
            let static_id = ent.id();
            dec_entity = Some(static_id);
            drop(ent);
            if dsft_lr > 0 {
                commands.spawn((
                    crate::game::lighting::decoration_point_light(
                        crate::game::lighting::DecorationLight::StaticDsft(dsft_lr),
                        false,
                    ),
                    Transform::from_translation(sprite_center),
                    InGame,
                ));
            }
        }
        if dec.light_radius > 0 {
            commands.spawn((
                crate::game::lighting::decoration_point_light(
                    crate::game::lighting::DecorationLight::Ddeclist(dec.light_radius),
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

    // Spawn BLV static point lights (designer-placed lights for campfires, cauldrons, etc.).
    // radius field is always 0 in MM6 data — brightness alone drives the falloff.
    // Range and intensity are decoupled: range scales linearly so small lights don't get
    // a range boost from a high-intensity formula.
    // brightness=64 → range~960 (small torch); brightness=640 → range~9600 (campfire room-fill).
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

    // Spawn BLV monsters from spawn_points → mapstats (same pipeline as ODM).
    if let Some(ref monsters) = prepared.resolved_actors {
        for mon in monsters.iter() {
            let (states, state_masks, raw_w, raw_h) = sprites::load_entity_sprites(
                &mon.standing_sprite,
                &mon.walking_sprite,
                &mon.attacking_sprite,
                &mon.dying_sprite,
                game_assets.assets(),
                &mut images,
                &mut sprite_materials,
                &mut Some(&mut sprite_cache),
                mon.variant,
                mon.palette_id,
                &tint_buffers.regular,
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
                    actor::Actor {
                        name: mon.name.clone(),
                        hp: mon.hp,
                        max_hp: mon.hp,
                        move_speed: mon.move_speed as f32,
                        initial_position: pos,
                        guarding_position: pos,
                        tether_distance: mon.radius as f32 * 2.0,
                        wander_timer: (pos.x * 0.011 + pos.z * 0.017).abs().fract() * 4.0,
                        wander_target: pos,
                        facing_yaw: 0.0,
                        hostile: true,
                        variant: mon.variant,
                        sound_ids: mon.sound_ids,
                        fidget_timer: (pos.x * 0.013 + pos.z * 0.019).abs().fract() * 15.0 + 5.0,
                        attack_range: mon.body_radius as f32 * 2.0,
                        attack_timer: (pos.x * 0.007 + pos.z * 0.023).abs().fract() * 3.0 + 1.0,
                        attack_anim_remaining: 0.0,
                        ddm_id: -1,
                        group_id: 0,
                        aggro_range: mon.aggro_range,
                        recovery_secs: mon.recovery_secs,
                        sprite_half_height: sh / 2.0,
                        can_fly: mon.can_fly,
                        vertical_velocity: 0.0,
                        ai_type: mon.ai_type.clone(),
                        cached_steer_offset: None,
                    },
                    crate::game::sprites::Billboard,
                    InGame,
                ))
                .id();
            if !cfg.actor_shadows {
                commands.entity(mon_id).insert(NotShadowCaster);
            }
            info!("Spawned indoor monster '{}' at {:?}", mon.name, pos);
        }
    }
}

// --- Indoor face interaction ---

pub(crate) const INDOOR_INTERACT_RANGE: f32 = 5120.0;

/// Detect indoor face interaction (Enter/click) and dispatch EVT events.
pub(crate) fn indoor_interact_system(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    gamepads: Query<&Gamepad>,
    camera_query: Query<(&GlobalTransform, &Camera), With<PlayerCamera>>,
    clickable: Option<Res<crate::game::interaction::clickable::Faces>>,
    map_events: Option<Res<MapEvents>>,
    mut event_queue: ResMut<EventQueue>,
    cursor_query: Query<&CursorOptions, With<PrimaryWindow>>,
) {
    let Some(clickable) = clickable else { return };
    // Only handle indoor maps — outdoor BSP faces are handled by world_interact_system.
    if !clickable.is_indoor || clickable.faces.is_empty() {
        return;
    }

    // Check input
    let key = keys.just_pressed(KeyCode::KeyE) || keys.just_pressed(KeyCode::Enter);
    let click = mouse.just_pressed(MouseButton::Left);
    let gamepad = gamepads
        .iter()
        .any(|gp| gp.just_pressed(bevy::input::gamepad::GamepadButton::East));
    if !key && !click && !gamepad {
        return;
    }

    // Don't process click if cursor isn't grabbed
    if click {
        let cursor_grabbed = cursor_query
            .single()
            .map(|c| !matches!(c.grab_mode, CursorGrabMode::None))
            .unwrap_or(true);
        if !cursor_grabbed {
            return;
        }
    }

    let Ok((cam_global, _)) = camera_query.single() else {
        return;
    };
    let ray_origin = cam_global.translation();
    let ray_dir = cam_global.forward().as_vec3();

    // Find nearest clickable face hit
    let mut nearest_hit: Option<(f32, u16)> = None;
    for face in &clickable.faces {
        if let Some(t) = ray_plane_intersect(ray_origin, ray_dir, face.normal, face.plane_dist) {
            if t > INDOOR_INTERACT_RANGE {
                continue;
            }
            let hit_point = ray_origin + ray_dir * t;
            if point_in_polygon(hit_point, &face.vertices, face.normal)
                && (nearest_hit.is_none() || t < nearest_hit.unwrap().0)
            {
                nearest_hit = Some((t, face.event_id));
            }
        }
    }

    if let Some((dist, event_id)) = nearest_hit {
        info!("Indoor interact: hit face event_id={} at dist={:.0}", event_id, dist);
        let Some(me) = map_events else {
            warn!("Indoor interact: no MapEvents resource");
            return;
        };
        let Some(evt) = me.evt.as_ref() else {
            warn!("Indoor interact: no EVT file loaded");
            return;
        };
        if let Some(steps) = evt.events.get(&event_id) {
            info!(
                "Indoor interact: dispatching {} steps for event_id={}",
                steps.len(),
                event_id
            );
        } else {
            info!("Indoor interact: no actions found for event_id={}", event_id);
        }
        event_queue.push_all(event_id, evt);
    }
}

// --- Touch trigger (EVENT_BY_TOUCH proximity) ---

/// Check player proximity to touch-triggered faces and dispatch events.
pub(crate) fn indoor_touch_trigger_system(
    player_query: Query<&Transform, With<crate::game::player::Player>>,
    mut touch_triggers: Option<ResMut<TouchTriggerFaces>>,
    map_events: Option<Res<MapEvents>>,
    mut event_queue: ResMut<EventQueue>,
) {
    let Some(ref mut triggers) = touch_triggers else { return };
    if triggers.faces.is_empty() {
        return;
    }
    let Ok(player_tf) = player_query.single() else { return };
    let player_pos = player_tf.translation;

    // Collect events to fire (avoids borrow conflict with fired set)
    let to_fire: Vec<u16> = triggers
        .faces
        .iter()
        .filter(|f| !triggers.fired.contains(&f.event_id))
        .filter(|f| player_pos.distance(f.center) <= f.radius)
        .map(|f| f.event_id)
        .collect();

    if let Some(me) = map_events.as_ref()
        && let Some(evt) = me.evt.as_ref()
    {
        for eid in &to_fire {
            info!("Touch trigger: event_id={}", eid);
            event_queue.push_all(*eid, evt);
        }
    }
    for eid in to_fire {
        triggers.fired.insert(eid);
    }

    // Reset fired events when player moves away (allows re-triggering)
    let still_near: std::collections::HashSet<u16> = triggers
        .faces
        .iter()
        .filter(|f| triggers.fired.contains(&f.event_id) && player_pos.distance(f.center) <= f.radius * 1.5)
        .map(|f| f.event_id)
        .collect();
    triggers.fired = still_near;
}

// --- Door trigger ---

/// Trigger a door state change. Called from event_dispatch when ChangeDoorState fires.
///
/// MM6 SetDoorState actions (from MMExtension):
///   0 = go to state (0) = Open position (initial)
///   1 = go to state (1) = Closed position (alternate)
///   2 = toggle if door isn't moving
///   3 = toggle always
pub fn trigger_door(doors: &mut BlvDoors, door_id: u32, action: u8) {
    // For toggle (action=2 or 3), resolve to open/close first
    let resolved_action = if action == 2 || action == 3 {
        let Some(door) = doors.doors.iter().find(|d| d.door_id == door_id) else {
            warn!("trigger_door: no door with id={}", door_id);
            return;
        };
        // action=2: only toggle if fully open/closed (not moving)
        if action == 2 && matches!(door.state, DoorState::Opening | DoorState::Closing) {
            return;
        }
        match door.state {
            DoorState::Closed | DoorState::Closing => 0u8, // -> Open (state 0)
            DoorState::Open | DoorState::Opening => 1u8,   // -> Close (state 1)
        }
    } else {
        action
    };

    let Some(door) = doors.doors.iter_mut().find(|d| d.door_id == door_id) else {
        warn!("trigger_door: no door with id={}", door_id);
        return;
    };

    match resolved_action {
        0 => {
            // Go to state (0) = Open position
            match door.state {
                DoorState::Closed | DoorState::Closing => {
                    if door.state == DoorState::Closing && door.move_length > 0 && door.open_speed > 0 {
                        let total_close_time = (door.move_length as f32 / door.close_speed as f32) * 1000.0;
                        let progress = (door.time_since_triggered_ms / total_close_time).clamp(0.0, 1.0);
                        let total_open_time = (door.move_length as f32 / door.open_speed as f32) * 1000.0;
                        door.time_since_triggered_ms = total_open_time * (1.0 - progress);
                    } else {
                        door.time_since_triggered_ms = 0.0;
                    }
                    door.state = DoorState::Opening;
                }
                _ => {}
            }
        }
        1 => {
            // Go to state (1) = Closed position
            match door.state {
                DoorState::Open | DoorState::Opening => {
                    if door.state == DoorState::Opening && door.move_length > 0 && door.close_speed > 0 {
                        let total_open_time = (door.move_length as f32 / door.open_speed as f32) * 1000.0;
                        let progress = (door.time_since_triggered_ms / total_open_time).clamp(0.0, 1.0);
                        let total_close_time = (door.move_length as f32 / door.close_speed as f32) * 1000.0;
                        door.time_since_triggered_ms = total_close_time * (1.0 - progress);
                    } else {
                        door.time_since_triggered_ms = 0.0;
                    }
                    door.state = DoorState::Closing;
                }
                _ => {}
            }
        }
        _ => {
            warn!("trigger_door: unknown action={}", resolved_action);
        }
    }
}

// --- Door animation ---

/// Calculate the slide distance for a door based on its current state and time.
///
/// Returns the displacement from the "open" (base) position:
///   0.0 = fully open (vertices at BLV positions)
///   move_length = fully closed (vertices displaced by direction * move_length)
fn door_slide_distance(door: &DoorRuntime) -> f32 {
    match door.state {
        DoorState::Open => 0.0,
        DoorState::Closed => door.move_length as f32,
        DoorState::Opening => {
            // Opening = moving toward Open (displacement decreasing: move_length → 0)
            let total_time = if door.open_speed > 0 {
                (door.move_length as f32 / door.open_speed as f32) * 1000.0
            } else {
                0.0
            };
            let progress = if total_time > 0.0 {
                (door.time_since_triggered_ms / total_time).clamp(0.0, 1.0)
            } else {
                1.0
            };
            (1.0 - progress) * door.move_length as f32
        }
        DoorState::Closing => {
            // Closing = moving toward Closed (displacement increasing: 0 → move_length)
            let total_time = if door.close_speed > 0 {
                (door.move_length as f32 / door.close_speed as f32) * 1000.0
            } else {
                0.0
            };
            let progress = if total_time > 0.0 {
                (door.time_since_triggered_ms / total_time).clamp(0.0, 1.0)
            } else {
                1.0
            };
            progress * door.move_length as f32
        }
    }
}

/// Advance door animations, update mesh vertex positions, and rebuild door collision.
pub(crate) fn door_animation_system(
    time: Res<Time>,
    mut blv_doors: Option<ResMut<BlvDoors>>,
    door_faces: Query<(&DoorFace, &Mesh3d)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut door_colliders: Option<ResMut<DoorColliders>>,
) {
    let Some(ref mut doors) = blv_doors else { return };
    let dt_ms = time.delta_secs() * 1000.0;

    // First pass: advance door timers
    for door in &mut doors.doors {
        match door.state {
            DoorState::Opening => {
                door.time_since_triggered_ms += dt_ms;
                let total_time = if door.open_speed > 0 {
                    (door.move_length as f32 / door.open_speed as f32) * 1000.0
                } else {
                    0.0
                };
                if door.time_since_triggered_ms >= total_time {
                    door.time_since_triggered_ms = total_time;
                    door.state = DoorState::Open;
                }
            }
            DoorState::Closing => {
                door.time_since_triggered_ms += dt_ms;
                let total_time = if door.close_speed > 0 {
                    (door.move_length as f32 / door.close_speed as f32) * 1000.0
                } else {
                    0.0
                };
                if door.time_since_triggered_ms >= total_time {
                    door.time_since_triggered_ms = total_time;
                    door.state = DoorState::Closed;
                }
            }
            _ => {} // Open/Closed: no timer change needed
        }
    }

    // Second pass: update mesh positions for all door faces
    for (face, mesh_handle) in door_faces.iter() {
        let Some(door) = doors.doors.get(face.door_index) else {
            continue;
        };

        let distance = door_slide_distance(door);

        // Convert MM6 direction to Bevy coords: mm6(x,y,z) → bevy(x,z,-y)
        let dir = [door.direction[0], door.direction[2], -door.direction[1]];

        let Some(mesh) = meshes.get_mut(&mesh_handle.0) else {
            continue;
        };

        // Update vertex positions using stored base positions (Bevy coords).
        // base_positions are the BLV (deployed/blocking) positions; retracted = base + dir * distance.
        if let Some(VertexAttributeValues::Float32x3(positions)) = mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION) {
            for (vi, pos) in positions.iter_mut().enumerate() {
                if !face.is_moving_vertex.get(vi).copied().unwrap_or(false) {
                    continue;
                }
                let Some(base) = face.base_positions.get(vi) else {
                    continue;
                };
                pos[0] = base[0] + dir[0] * distance;
                pos[1] = base[1] + dir[1] * distance;
                pos[2] = base[2] + dir[2] * distance;
            }
        }

        // Update UVs for faces where texture must track geometry deformation.
        //
        // Three cases based on which vertices move:
        // 1. Pure panel (all vertices moving): no correction needed — the texture is carried
        //    rigidly with the mesh. UVs stay correct without any update.
        // 2. MOVES_BY_DOOR flag (reveal/frame face): fixed vertices anchor the texture while
        //    moving vertices pull it. Scroll OPPOSITE to motion: base - rate*distance.
        // 3. Hybrid face (some vertices move, no flag): deforming geometry. Moving vertices
        //    need UV correction to stay aligned: base + rate*distance.
        let all_moving = face.is_moving_vertex.iter().all(|&m| m);
        if !all_moving
            && face.uv_rate != [0.0, 0.0]
            && let Some(VertexAttributeValues::Float32x2(uvs)) = mesh.attribute_mut(Mesh::ATTRIBUTE_UV_0)
        {
            for (vi, uv) in uvs.iter_mut().enumerate() {
                if face.is_moving_vertex.get(vi).copied().unwrap_or(false)
                    && let Some(base) = face.base_uvs.get(vi)
                {
                    if face.moves_by_door {
                        // Frame face: scroll texture counter to motion so it doesn't stretch.
                        uv[0] = base[0] - face.uv_rate[0] * distance;
                        uv[1] = base[1] - face.uv_rate[1] * distance;
                    } else {
                        // Hybrid face: advance UV with the moving vertex to cancel deformation.
                        uv[0] = base[0] + face.uv_rate[0] * distance;
                        uv[1] = base[1] + face.uv_rate[1] * distance;
                    }
                }
            }
        }
    }

    // Third pass: rebuild door collision walls from face data
    if let Some(ref mut colliders) = door_colliders {
        // Build new walls into a temporary vec to avoid borrow conflict
        let mut new_walls = Vec::new();
        for cf in &colliders.face_data {
            let Some(door) = doors.doors.get(cf.door_index) else {
                continue;
            };

            // Only skip collision when the door is fully retracted (passable).
            let distance = door_slide_distance(door);
            if door.move_length == 0 || distance >= door.move_length as f32 - 1.0 {
                continue; // Door fully retracted — no collision
            }

            let dir_bevy = Vec3::new(door.direction[0], door.direction[2], -door.direction[1]);

            // Compute current vertex positions (applying door displacement to moving verts)
            let current_verts: Vec<Vec3> = cf
                .base_positions
                .iter()
                .enumerate()
                .map(|(vi, base)| {
                    if cf.is_moving.get(vi).copied().unwrap_or(false) {
                        *base + dir_bevy * distance
                    } else {
                        *base
                    }
                })
                .collect();

            if current_verts.len() < 3 {
                continue;
            }

            // Deduplicate vertices (triangulated meshes have duplicates)
            let mut unique_verts: Vec<Vec3> = Vec::new();
            for v in &current_verts {
                let is_dup = unique_verts.iter().any(|u| (*u - *v).length_squared() < 1.0);
                if !is_dup {
                    unique_verts.push(*v);
                }
            }
            if unique_verts.len() < 3 {
                continue;
            }

            let plane_dist = cf.normal.dot(unique_verts[0]);
            new_walls.push(crate::game::collision::CollisionWall::new(
                cf.normal,
                plane_dist,
                &unique_verts,
            ));
        }
        colliders.walls = new_walls;

        // Rebuild dynamic ceilings from horizontal door faces.
        // A horizontal panel at height Y blocks the player when Y is between their feet and eyes.
        let mut new_ceilings = Vec::new();
        for cf in &colliders.horizontal_face_data {
            let Some(door) = doors.doors.get(cf.door_index) else {
                continue;
            };
            let distance = door_slide_distance(door);
            if door.move_length == 0 || distance >= door.move_length as f32 - 1.0 {
                continue; // Door fully retracted — no collision
            }

            let dir_bevy = Vec3::new(door.direction[0], door.direction[2], -door.direction[1]);

            // Mesh vertices come in triangle triples; compute current positions for each
            for (chunk_idx, chunk) in cf.base_positions.chunks(3).enumerate() {
                if chunk.len() < 3 {
                    continue;
                }
                let base_vi = chunk_idx * 3;
                let v: Vec<Vec3> = chunk
                    .iter()
                    .enumerate()
                    .map(|(i, base)| {
                        let vi = base_vi + i;
                        if cf.is_moving.get(vi).copied().unwrap_or(false) {
                            *base + dir_bevy * distance
                        } else {
                            *base
                        }
                    })
                    .collect();
                new_ceilings.push(crate::game::collision::CollisionTriangle::new(
                    v[0], v[1], v[2], cf.normal,
                ));
            }
        }
        colliders.dynamic_ceilings = new_ceilings;
    }
}
