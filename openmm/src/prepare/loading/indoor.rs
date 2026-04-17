//! Indoor (BLV) model building — meshes, doors, collision, clickable faces, spawn points.
use bevy::{asset::RenderAssetUsages, mesh::PrimitiveTopology, prelude::*};
use openmm_data::{blv::Blv, utils::OdmName};

use super::{
    ClickableFaceData, LoadRequest, LoadingProgress, OccluderFaceData, PreparedDoorCollision, PreparedDoorFace,
    PreparedIndoorWorld, PreparedModel, PreparedSubMesh, SectorAmbient, StartPoint, TouchTriggerFaceData, helpers,
};
use crate::prepare::build_indoor::{blv_face_normal, blv_face_verts, extract_blv_collision};
use crate::{
    GameState,
    assets::GameAssets,
    game::map::coords::{mm6_binary_angle_to_radians, mm6_position_to_bevy},
};

/// Build indoor (BLV) geometry: meshes, doors, collision, clickable/touch/occluder faces,
/// spawn points, lights, and sector ambient data. Transitions directly to Game state.
pub(super) fn step_build_models_indoor(
    progress: &mut LoadingProgress,
    game_assets: &GameAssets,
    load_request: &LoadRequest,
    commands: &mut Commands,
    game_state: &mut NextState<GameState>,
) {
    let blv = progress.blv.as_ref().unwrap();

    // Indoor: build meshes from BLV faces
    let texture_sizes = helpers::collect_texture_sizes(&blv.texture_names, game_assets);
    // Load DLV to get door data
    let dlv_result = openmm_data::dlv::Dlv::new(
        game_assets.assets(),
        &load_request.map_name.to_string(),
        blv.door_count,
        blv.doors_data_size,
    );
    let mut dlv_doors = dlv_result.as_ref().map(|d| d.doors.clone()).unwrap_or_default();

    // Fill in any doors missing face/vertex data from BLV geometry.
    // Some DLV files have fully populated door data; others need
    // runtime initialization (matching the original engine's InitializeDoors).
    blv.initialize_doors(&mut dlv_doors);

    // Exclude door faces from batched geometry
    let door_faces = openmm_data::blv::Blv::door_face_set(&dlv_doors, &blv.faces);
    let textured = blv.textured_meshes(&texture_sizes, &door_faces);

    // Generate individual door face meshes
    let door_face_meshes_raw = blv.door_face_meshes(&dlv_doors, &texture_sizes);
    let prepared_door_faces: Vec<PreparedDoorFace> = door_face_meshes_raw
        .into_iter()
        .map(|dfm| {
            // Door meshes need MAIN_WORLD to retain vertex data for animation
            let mut mesh = Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
            );
            // Capture base positions before consuming them into the mesh.
            let base_positions = dfm.positions.clone();
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, dfm.positions);
            mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, dfm.normals);
            let base_uvs = dfm.uvs.clone();
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, dfm.uvs);
            // Skip generate_tangents — door vertices are animated and
            // tangents would become stale. Not needed for flat surfaces.
            let texture = game_assets.lod().bitmap(&dfm.texture_name).map(|img| {
                let mut image = crate::assets::dynamic_to_bevy_image(img);
                image.sampler = crate::assets::repeat_sampler();
                image
            });
            PreparedDoorFace {
                face_index: dfm.face_index,
                door_index: dfm.door_index,
                mesh,
                material: helpers::indoor_material(&dfm.texture_name),
                texture,
                is_moving_vertex: dfm.is_moving,
                base_positions,
                uv_rate: dfm.uv_rate,
                base_uvs,
                moves_by_door: dfm.moves_by_door,
            }
        })
        .collect();

    // Build door collision geometry for ALL faces in door_face_set (including
    // invisible/empty-texture ones). These blocking surfaces are excluded from
    // door_face_meshes but still need dynamic collision (e.g. face[1944] in d01
    // is an invisible VerticalWall with empty texture that is the actual blocker).
    let door_collision_geometry: Vec<PreparedDoorCollision> = {
        // Build reverse map: face_index -> door_index
        let mut face_to_door: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        for (di, door) in dlv_doors.iter().enumerate() {
            for &fid in &door.face_ids {
                let fi = fid as usize;
                if door_faces.contains(&fi) {
                    face_to_door.insert(fi, di);
                }
            }
        }
        face_to_door
            .iter()
            .filter_map(|(&face_idx, &door_index)| {
                let face = blv.faces.get(face_idx)?;
                if face.num_vertices < 3 || face.is_portal() {
                    return None;
                }
                let door = dlv_doors.get(door_index)?;
                let moving_vids: std::collections::HashSet<u16> = door.vertex_ids.iter().copied().collect();

                let verts = blv_face_verts(face, blv);
                if verts.len() < 3 {
                    return None;
                }

                // Fan triangulate: tris [0,1,2], [0,2,3], ...
                let nv = verts.len();
                let mut base_positions = Vec::with_capacity((nv - 2) * 3);
                let mut is_moving = Vec::with_capacity((nv - 2) * 3);
                for i in 0..nv - 2 {
                    let idxs = [0usize, i + 1, i + 2];
                    for &k in &idxs {
                        base_positions.push(verts[k]);
                        is_moving.push(moving_vids.contains(&face.vertex_ids[k]));
                    }
                }

                // Stored BLV normal in Bevy coords (no ceiling rendering sign flip
                // — for collision we use the raw geometric normal).
                let normal = blv_face_normal(face);
                Some(PreparedDoorCollision {
                    door_index,
                    base_positions,
                    normal,
                    is_moving,
                })
            })
            .collect()
    };

    // Collect clickable faces
    let clickable_faces: Vec<ClickableFaceData> = blv
        .faces
        .iter()
        .enumerate()
        .filter(|(_, f)| f.is_clickable() && f.event_id != 0 && f.num_vertices >= 3)
        .filter_map(|(i, face)| {
            let verts = blv_face_verts(face, blv);
            if verts.len() < 3 {
                return None;
            }
            let normal = blv_face_normal(face);
            let plane_dist = normal.dot(verts[0]);
            Some(ClickableFaceData {
                face_index: i,
                event_id: face.event_id,
                normal,
                plane_dist,
                vertices: verts,
            })
        })
        .collect();

    // Collect touch-triggered faces (EVENT_BY_TOUCH flag)
    let touch_trigger_faces: Vec<TouchTriggerFaceData> = blv
        .faces
        .iter()
        .enumerate()
        .filter(|(_, f)| f.is_touch_trigger() && f.event_id != 0 && f.num_vertices >= 3)
        .filter_map(|(i, face)| {
            let verts = blv_face_verts(face, blv);
            if verts.len() < 3 {
                return None;
            }
            let center = verts.iter().copied().sum::<Vec3>() / verts.len() as f32;
            // Use half bounding box diagonal as trigger radius
            let min = verts.iter().copied().reduce(|a, b| a.min(b))?;
            let max = verts.iter().copied().reduce(|a, b| a.max(b))?;
            let radius = (max - min).length() * 0.5;
            Some(TouchTriggerFaceData {
                face_index: i,
                event_id: face.event_id,
                center,
                radius: radius.max(128.0), // minimum trigger radius
            })
        })
        .collect();

    // Collect all solid faces for ray occlusion (wall/floor/ceiling, no portals, no door faces).
    let occluder_faces: Vec<OccluderFaceData> = blv
        .faces
        .iter()
        .enumerate()
        .filter(|(i, f)| !f.is_invisible() && !f.is_portal() && f.num_vertices >= 3 && !door_faces.contains(i))
        .filter_map(|(_, face)| {
            let verts = blv_face_verts(face, blv);
            if verts.len() < 3 {
                return None;
            }
            let normal = blv_face_normal(face);
            let plane_dist = normal.dot(verts[0]);
            Some(OccluderFaceData {
                normal,
                plane_dist,
                vertices: verts,
            })
        })
        .collect();

    let models = vec![PreparedModel {
        sub_meshes: textured
            .into_iter()
            .map(|tm| {
                let mesh = helpers::build_textured_mesh(tm.positions, tm.normals, tm.uvs);
                let texture = game_assets.lod().bitmap(&tm.texture_name).map(|img| {
                    let mut image = crate::assets::dynamic_to_bevy_image(img);
                    image.sampler = crate::assets::repeat_sampler();
                    image
                });
                PreparedSubMesh {
                    mesh,
                    material: helpers::indoor_material(&tm.texture_name),
                    texture,
                    texture_name: tm.texture_name.clone(),
                    face_indices: tm.face_indices.clone(),
                }
            })
            .collect(),
        name: "blv_faces".to_string(),
        position: Vec3::ZERO,
        event_ids: vec![],
    }];
    // Spawn position: prefer LoadRequest.spawn_position (set by MoveToMap),
    // then try the map's own EVT for a self-referencing MoveToMap (entry point),
    // finally fall back to sector center.
    let (spawn_pos, spawn_yaw) = if let Some(pos) = load_request.spawn_position {
        info!("Indoor spawn from MoveToMap event: pos={:?}", pos);
        (Vec3::from(pos), load_request.spawn_yaw.unwrap_or(0.0))
    } else {
        resolve_indoor_spawn(blv, load_request, game_assets)
    };
    let start_points = vec![StartPoint {
        name: "indoor_start".to_string(),
        position: spawn_pos,
        yaw: spawn_yaw,
    }];
    // Extract collision geometry from BLV faces, excluding animated door faces.
    // Door face geometry is animated separately; their collision would block
    // the player even after a door opens.
    let (collision_walls, collision_floors, collision_ceilings) = extract_blv_collision(blv, &door_faces);
    let map_base = match &load_request.map_name {
        openmm_data::utils::MapName::Indoor(name) => name.clone(),
        _ => load_request.map_name.to_string().replace(".blv", ""),
    };
    // Resolve BLV decorations (torches, chests, etc.)
    let decorations = openmm_data::assets::Decorations::from_blv(game_assets.assets(), &blv.decorations)
        .unwrap_or_else(|e| {
            warn!("Failed to resolve indoor decorations: {e}");
            openmm_data::assets::Decorations::empty()
        });

    // Per-sector ambient data for the lighting system.
    // Sector 0 is always a sentinel "void" sector — skip it.
    let sector_ambients: Vec<SectorAmbient> = blv
        .sectors
        .iter()
        .skip(1)
        .filter(|s| s.floor_count > 0)
        .map(|s| {
            let [x0, y0, z0] = s.bbox_min.map(|v| v as i32);
            let [x1, y1, z1] = s.bbox_max.map(|v| v as i32);
            let bmin = Vec3::from(mm6_position_to_bevy(x0, y0, z0));
            let bmax = Vec3::from(mm6_position_to_bevy(x1, y1, z1));
            // mm6_position_to_bevy flips Y/Z, so ensure min ≤ max on all axes.
            SectorAmbient {
                bbox_min: bmin.min(bmax),
                bbox_max: bmin.max(bmax),
                min_ambient: s.min_ambient_light.clamp(0, 255) as u8,
            }
        })
        .collect();

    // Collect BLV static lights (designer-placed — campfires, braziers, etc.).
    // radius is always 0 in MM6 data; brightness drives range + intensity.
    let blv_lights: Vec<(Vec3, u16)> = blv
        .lights
        .iter()
        .filter(|l| l.brightness > 0)
        .map(|l| {
            let [x, y, z] = l.position;
            let pos = Vec3::from(mm6_position_to_bevy(x as i32, y as i32, z as i32));
            (pos, l.brightness)
        })
        .collect();

    // Resolve BLV spawn-point actors (same pipeline as ODM monsters).
    let resolved_actors = openmm_data::assets::Monsters::load_for_blv(
        &blv.spawn_points,
        &load_request.map_name.to_string(),
        game_assets.data(),
        game_assets.assets(),
    )
    .ok();

    crate::game::events::load_map_events(commands, game_assets, &map_base, true);
    commands.insert_resource(PreparedIndoorWorld {
        models,
        start_points,
        collision_walls,
        collision_floors,
        collision_ceilings,
        doors: dlv_doors,
        door_face_meshes: prepared_door_faces,
        door_collision_geometry,
        clickable_faces,
        touch_trigger_faces,
        occluder_faces,
        map_base,
        decorations,
        resolved_actors,
        blv_lights,
        sector_ambients,
    });
    commands.insert_resource(crate::game::map::CurrentMap(load_request.map_name.clone()));
    commands.remove_resource::<LoadingProgress>();
    commands.remove_resource::<LoadRequest>();
    game_state.set(GameState::Game);
}

/// Search outdoor EVT files for a MoveToMap targeting this BLV, or fall back
/// to sector center. Returns (position, yaw) in Bevy coords.
fn resolve_indoor_spawn(blv: &Blv, load_request: &LoadRequest, game_assets: &GameAssets) -> (Vec3, f32) {
    let blv_name = match &load_request.map_name {
        openmm_data::utils::MapName::Indoor(name) => format!("{}.blv", name),
        _ => String::new(),
    };
    // Search all outdoor map EVT files for a spawn point matching this indoor map.
    let outdoor_bases: Vec<String> = OdmName::all().map(|o| o.base_name()).collect();
    let evt_entry = outdoor_bases.iter().find_map(|base| {
        openmm_data::evt::EvtFile::parse(game_assets.assets(), base)
            .ok()
            .and_then(|evt| {
                evt.events.values().flatten().find_map(|s| {
                    if let openmm_data::evt::GameEvent::MoveToMap {
                        x,
                        y,
                        z,
                        direction,
                        map_name,
                    } = &s.event
                    {
                        if map_name.eq_ignore_ascii_case(&blv_name) {
                            Some((*x, *y, *z, *direction))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
            })
    });
    if let Some((x, y, z, dir)) = evt_entry {
        let pos = Vec3::from(mm6_position_to_bevy(x, y, z));
        let yaw = mm6_binary_angle_to_radians(dir);
        info!(
            "Indoor spawn from EVT self-MoveToMap: mm6=({},{},{}) dir={}",
            x, y, z, dir
        );
        (pos, yaw)
    } else {
        // Final fallback: center of sector with most floors
        let spawn_sector = blv.sectors.iter().skip(1).max_by_key(|s| s.floor_count);
        let pos = if let Some(sector) = spawn_sector {
            let cx = (sector.bbox_min[0] as i32 + sector.bbox_max[0] as i32) / 2;
            let cy = (sector.bbox_min[1] as i32 + sector.bbox_max[1] as i32) / 2;
            let floor_z = sector.bbox_min[2].min(sector.bbox_max[2]) as i32;
            info!("Indoor spawn from sector center: floors={}", sector.floor_count);
            Vec3::from(mm6_position_to_bevy(cx, cy, floor_z))
        } else {
            Vec3::ZERO
        };
        (pos, 0.0)
    }
}
