use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

use std::collections::HashMap;

use crate::{
    GameState, assets::GameAssets, config::GameConfig, despawn_all, game::map_name::MapName, game::odm::OdmName,
};
use openmm_data::{
    blv::Blv,
    dtile::{Dtile, TileTable},
    odm::{Odm, OdmData, mm6_to_bevy},
};

pub struct LoadingPlugin;

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(GameState::Loading),
            (despawn_all::<crate::game::InGame>, loading_setup).chain(),
        )
        .add_systems(Update, loading_step.run_if(in_state(GameState::Loading)))
        .add_systems(OnExit(GameState::Loading), despawn_all::<InLoading>);
    }
}

#[derive(Component)]
struct InLoading;

/// Requested map to load. Insert this resource before transitioning to Loading state.
#[derive(Resource)]
pub struct LoadRequest {
    pub map_name: MapName,
    /// Optional spawn position override (Bevy coords). When set, this takes
    /// priority over save_data for indoor spawn. Set by MoveToMap events.
    pub spawn_position: Option<[f32; 3]>,
    /// Optional spawn yaw override (radians).
    pub spawn_yaw: Option<f32>,
}

/// Tracks which step of the loading pipeline we're on.
#[derive(Resource)]
struct LoadingProgress {
    step: LoadingStep,
    odm: Option<Odm>,
    tile_table: Option<TileTable>,
    odm_data: Option<OdmData>,
    terrain_mesh: Option<Mesh>,
    terrain_texture: Option<Image>,
    water_mask: Option<Image>,
    water_texture: Option<Image>,
    models: Option<Vec<PreparedModel>>,
    decorations: Option<openmm_data::assets::Decorations>,
    resolved_actors: Option<openmm_data::assets::Actors>,
    resolved_monsters: Option<openmm_data::assets::Monsters>,
    start_points: Option<Vec<StartPoint>>,
    sprite_cache: Option<crate::game::entities::sprites::SpriteCache>,
    dec_sprite_cache: Option<
        std::collections::HashMap<
            String,
            (
                Handle<StandardMaterial>,
                Handle<Mesh>,
                f32,
                f32,
                std::sync::Arc<crate::game::entities::sprites::AlphaMask>,
            ),
        >,
    >,
    water_cells: Option<Vec<bool>>,
    terrain_lookup: Option<openmm_data::terrain::TerrainLookup>,
    music_track: u8,
    blv: Option<Blv>,
    /// Queued sprite preload work, processed in batches across frames.
    preload_queue: Option<PreloadQueue>,
}

/// Queued sprite preload work items, processed across multiple frames.
struct PreloadQueue {
    /// (sprite_root, variant, palette_id) triples to preload into SpriteCache.
    sprite_roots: Vec<(String, u8, u16)>,
    /// Index into progress.decorations.entries() for billboard preloading.
    billboard_idx: usize,
    /// Current position in sprite_roots.
    sprite_idx: usize,
    /// Whether music track has been resolved.
    music_resolved: bool,
}

/// Resource for indoor (BLV) maps — the indoor equivalent of PreparedWorld.
#[derive(Resource)]
pub struct PreparedIndoorWorld {
    pub models: Vec<PreparedModel>,
    pub start_points: Vec<StartPoint>,
    /// Wall collision geometry extracted from BLV faces.
    pub collision_walls: Vec<crate::game::collision::CollisionWall>,
    /// Floor collision geometry extracted from BLV faces.
    pub collision_floors: Vec<crate::game::collision::CollisionTriangle>,
    /// Ceiling collision geometry extracted from BLV faces.
    pub collision_ceilings: Vec<crate::game::collision::CollisionTriangle>,
    /// Door definitions from DLV.
    pub doors: Vec<openmm_data::blv::BlvDoor>,
    /// Individual door face meshes for animation.
    pub door_face_meshes: Vec<PreparedDoorFace>,
    /// Collision geometry for ALL door faces (including invisible), for DoorColliders.
    pub door_collision_geometry: Vec<PreparedDoorCollision>,
    /// Clickable face data for indoor interaction.
    pub clickable_faces: Vec<ClickableFaceData>,
    /// Touch-triggered faces (EVENT_BY_TOUCH) for proximity events.
    pub touch_trigger_faces: Vec<TouchTriggerFaceData>,
    /// All solid faces (wall/floor/ceiling, non-portal, non-invisible) for ray occlusion.
    pub occluder_faces: Vec<OccluderFaceData>,
    /// Map base name for EVT loading (e.g. "d01").
    pub map_base: String,
    /// Resolved decorations from BLV decoration list.
    pub decorations: openmm_data::assets::Decorations,
    /// Resolved monsters from BLV spawn_points → mapstats (same pipeline as ODM).
    pub resolved_actors: Option<openmm_data::assets::Monsters>,
    /// Static point lights from the BLV file (position in Bevy coords, brightness 0–65535).
    /// These are the designer-placed lights that illuminate campfires, cauldrons, etc.
    pub blv_lights: Vec<(Vec3, u16)>,
    /// Per-sector ambient data: (bbox_min, bbox_max in Bevy coords, min_ambient_light 0–255).
    /// Used to set global ambient based on which sector the player currently occupies.
    pub sector_ambients: Vec<SectorAmbient>,
}

/// Bounding box + minimum ambient light for one BLV sector.
/// `min_ambient` is the original MM6 value (0–255); scale to Bevy lux in the lighting system.
#[derive(Clone)]
pub struct SectorAmbient {
    pub bbox_min: Vec3,
    pub bbox_max: Vec3,
    /// 0–255 ambient floor. 0 = pitch black, 255 = fully lit.
    pub min_ambient: u8,
}

/// Collision-only geometry for a single door face, including invisible blocking surfaces.
/// This covers faces excluded from `door_face_meshes` (e.g. empty-texture invisible blockers)
/// that still need to block player movement.
pub struct PreparedDoorCollision {
    pub door_index: usize,
    /// Triangulated vertex positions in Bevy coords at door distance=0 (open/retracted state).
    pub base_positions: Vec<Vec3>,
    /// Face normal in Bevy coords (from BLV face.normal_f32(), without rendering sign flip).
    pub normal: Vec3,
    /// Per triangle-vertex: whether it moves with the door.
    pub is_moving: Vec<bool>,
}

/// A prepared door face mesh ready for spawning.
pub struct PreparedDoorFace {
    pub face_index: usize,
    pub door_index: usize,
    pub mesh: Mesh,
    pub material: StandardMaterial,
    pub texture: Option<Image>,
    /// Per triangle-vertex: whether it moves with the door.
    pub is_moving_vertex: Vec<bool>,
    /// Base vertex positions (Bevy coords) at door distance=0 (open/retracted state).
    pub base_positions: Vec<[f32; 3]>,
    /// UV change per unit of door displacement for moving vertices.
    pub uv_rate: [f32; 2],
    /// Base UV values per triangle vertex (at distance=0).
    pub base_uvs: Vec<[f32; 2]>,
    /// Whether this face has the MOVES_BY_DOOR flag (needs UV scrolling).
    pub moves_by_door: bool,
}

/// Data for a clickable indoor face.
pub struct ClickableFaceData {
    pub face_index: usize,
    pub event_id: u16,
    pub normal: Vec3,
    pub plane_dist: f32,
    pub vertices: Vec<Vec3>,
}

/// Data for a solid indoor face used for ray occlusion.
pub struct OccluderFaceData {
    pub normal: Vec3,
    pub plane_dist: f32,
    pub vertices: Vec<Vec3>,
}

/// Data for a touch-triggered indoor face (EVENT_BY_TOUCH flag).
/// These fire events when the player walks near/over them.
pub struct TouchTriggerFaceData {
    pub face_index: usize,
    pub event_id: u16,
    /// Center of the face in Bevy coordinates (for distance check).
    pub center: Vec3,
    /// Trigger radius — half the bounding box diagonal for floor faces.
    pub radius: f32,
}

pub struct PreparedModel {
    /// Sub-meshes, one per unique texture in the BSP model.
    pub sub_meshes: Vec<PreparedSubMesh>,
    /// BSP model name (e.g. "TavFrntW", "ArmoryW", "GenStorE").
    pub name: String,
    /// Model center position in Bevy coordinates.
    pub position: Vec3,
    /// Unique event IDs from this model's faces (cog_trigger_id values > 0).
    pub event_ids: Vec<u16>,
}

pub struct PreparedSubMesh {
    pub mesh: Mesh,
    pub material: StandardMaterial,
    pub texture: Option<Image>,
    /// Texture name used by this sub-mesh.
    pub texture_name: String,
    /// Original face indices (into BSPModel::faces) that contributed to this sub-mesh.
    pub face_indices: Vec<u32>,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum LoadingStep {
    #[default]
    ParseMap,
    BuildTerrain,
    BuildAtlas,
    BuildModels,
    BuildBillboards,
    PreloadSprites,
    Done,
}

impl LoadingStep {
    fn label(&self) -> &'static str {
        match self {
            Self::ParseMap => "Parsing map...",
            Self::BuildTerrain => "Building terrain...",
            Self::BuildAtlas => "Building textures...",
            Self::BuildModels => "Building models...",
            Self::BuildBillboards => "Loading decorations...",
            Self::PreloadSprites => "Loading sprites...",
            Self::Done => "Done!",
        }
    }

    fn next(&self) -> Self {
        match self {
            Self::ParseMap => Self::BuildTerrain,
            Self::BuildTerrain => Self::BuildAtlas,
            Self::BuildAtlas => Self::BuildModels,
            Self::BuildModels => Self::BuildBillboards,
            Self::BuildBillboards => Self::PreloadSprites,
            Self::PreloadSprites => Self::Done,
            Self::Done => Self::Done,
        }
    }
}

/// A named start/teleport point extracted from map decorations.
pub struct StartPoint {
    pub name: String,
    pub position: Vec3,
    pub yaw: f32,
}

/// Resource containing everything needed to spawn the world after loading.
#[derive(Resource)]
pub struct PreparedWorld {
    pub map: Odm,
    pub terrain_mesh: Mesh,
    pub terrain_texture: Image,
    pub water_mask: Option<Image>,
    pub water_texture: Option<Image>,
    pub models: Vec<PreparedModel>,
    pub decorations: openmm_data::assets::Decorations,
    pub resolved_actors: Option<openmm_data::assets::Actors>,
    pub resolved_monsters: Option<openmm_data::assets::Monsters>,
    pub start_points: Vec<StartPoint>,
    pub sprite_cache: crate::game::entities::sprites::SpriteCache,
    pub dec_sprite_cache: std::collections::HashMap<
        String,
        (
            Handle<StandardMaterial>,
            Handle<Mesh>,
            f32,
            f32,
            std::sync::Arc<crate::game::entities::sprites::AlphaMask>,
        ),
    >,
    pub water_cells: Vec<bool>,
    pub terrain_lookup: openmm_data::terrain::TerrainLookup,
    /// Music track ID from mapstats.txt (maps to Music/{track}.mp3). 0 = no music.
    pub music_track: u8,
}

impl PreparedWorld {
    /// Get the terrain tileset at a Bevy world position.
    pub fn terrain_at(&self, x: f32, z: f32) -> Option<openmm_data::dtile::Tileset> {
        self.terrain_lookup.tileset_at(&self.map, x, z)
    }
}

#[derive(Component)]
struct LoadingText;

fn loading_setup(
    mut commands: Commands,
    load_request: Option<Res<LoadRequest>>,
    save_data: Res<crate::save::GameSave>,
    cfg: Res<GameConfig>,
    game_assets: Res<GameAssets>,
    mut ui_assets: ResMut<crate::ui_assets::UiAssets>,
    mut images: ResMut<Assets<Image>>,
    mut world_state: ResMut<crate::game::world_state::WorldState>,
) {
    // Clean up resources from previous map (indoor or outdoor)
    commands.remove_resource::<PreparedWorld>();
    commands.remove_resource::<PreparedIndoorWorld>();
    commands.remove_resource::<crate::game::blv::BlvDoors>();
    commands.remove_resource::<crate::game::blv::DoorColliders>();
    commands.remove_resource::<crate::game::blv::ClickableFaces>();
    commands.remove_resource::<crate::game::blv::TouchTriggerFaces>();
    commands.remove_resource::<crate::game::blv::OccluderFaces>();
    commands.remove_resource::<crate::game::hud::MapOverviewImage>();

    // Consume and remove LoadRequest so it doesn't persist and block boundary crossing.
    let (map_name, spawn_position, spawn_yaw) = if let Some(r) = load_request {
        (r.map_name.clone(), r.spawn_position, r.spawn_yaw)
    } else {
        let name = cfg
            .map
            .as_ref()
            .and_then(|m| {
                MapName::try_from(m.as_str())
                    .inspect_err(|e| eprintln!("warning: invalid map in config: {e}"))
                    .ok()
            })
            .unwrap_or_else(|| {
                MapName::Outdoor(OdmName {
                    x: save_data.map.map_x,
                    y: save_data.map.map_y,
                })
            });
        (name, None, None)
    };

    // Keep world_state in sync so spawn_world sees the correct map name
    world_state.map.name = map_name.clone();
    if let MapName::Outdoor(ref odm) = map_name {
        world_state.map.map_x = odm.x;
        world_state.map.map_y = odm.y;
    }
    commands.insert_resource(LoadingProgress {
        step: LoadingStep::ParseMap,
        odm: None,
        tile_table: None,
        odm_data: None,
        terrain_mesh: None,
        terrain_texture: None,
        water_mask: None,
        water_texture: None,
        models: None,
        decorations: None,
        resolved_actors: None,
        resolved_monsters: None,
        start_points: None,
        sprite_cache: None,
        dec_sprite_cache: None,
        water_cells: None,
        terrain_lookup: None,
        music_track: 0,
        blv: None,
        preload_queue: None,
    });

    // Keep the load request around as context (preserve spawn position from MoveToMap)
    commands.insert_resource(LoadRequest {
        map_name,
        spawn_position,
        spawn_yaw,
    });

    // Spawn loading screen with loading.pcx background from LOD
    commands.spawn((Camera2d, InLoading));

    let loading_bg = ui_assets.get_or_load("loading.pcx", &game_assets, &mut images, &cfg);

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::FlexEnd,
                justify_content: JustifyContent::Center,
                ..default()
            },
            ImageNode::new(loading_bg.unwrap_or_default()),
            InLoading,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Loading..."),
                TextFont {
                    font_size: 24.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                Node {
                    margin: UiRect::all(Val::Px(20.0)),
                    ..default()
                },
                LoadingText,
            ));
        });
}

fn loading_step(
    mut progress: ResMut<LoadingProgress>,
    game_assets: Res<GameAssets>,
    load_request: Res<LoadRequest>,
    mut game_state: ResMut<NextState<GameState>>,
    mut commands: Commands,
    mut text_query: Query<&mut Text, With<LoadingText>>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    world_state: Option<Res<crate::game::world_state::WorldState>>,
) {
    // Update loading text
    for mut text in &mut text_query {
        **text = progress.step.label().to_string();
    }

    match progress.step {
        LoadingStep::ParseMap => {
            let map_name = load_request.map_name.to_string();
            if load_request.map_name.is_indoor() {
                match Blv::load(game_assets.lod_manager(), &map_name) {
                    Ok(blv) => {
                        progress.blv = Some(blv);
                        // Skip terrain — jump straight to BuildModels
                        progress.step = LoadingStep::BuildModels;
                    }
                    Err(e) => {
                        error!("Failed to parse indoor map {}: {}", map_name, e);
                        commands.remove_resource::<LoadRequest>();
                        game_state.set(GameState::Menu);
                        return;
                    }
                }
                return;
            }
            match Odm::load(game_assets.lod_manager(), &map_name) {
                Ok(odm) => {
                    match odm.tile_table(game_assets.lod_manager()) {
                        Ok(tile_table) => {
                            // Build water map from tile data
                            if let Ok(dtile) = Dtile::load(game_assets.lod_manager()) {
                                let water_cells: Vec<bool> =
                                    odm.tile_map.iter().map(|&idx| dtile.is_deep_water_tile(idx)).collect();
                                progress.water_cells = Some(water_cells);
                                progress.terrain_lookup =
                                    Some(openmm_data::terrain::TerrainLookup::new(&dtile, odm.tile_data));
                            }
                            progress.tile_table = Some(tile_table);
                            progress.odm = Some(odm);
                            progress.step = progress.step.next();
                        }
                        Err(e) => {
                            error!("Failed to load tile table: {}", e);
                            commands.remove_resource::<LoadRequest>();
                            game_state.set(GameState::Menu);
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to parse map {}: {}", map_name, e);
                    commands.remove_resource::<LoadRequest>();
                    game_state.set(GameState::Menu);
                }
            }
        }
        LoadingStep::BuildTerrain => {
            if let (Some(odm), Some(tile_table)) = (&progress.odm, &progress.tile_table) {
                let odm_data = OdmData::new(odm, tile_table);
                let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
                mesh.insert_indices(Indices::U32(odm_data.indices.clone()));
                mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, odm_data.positions.clone());
                mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, odm_data.normals.clone());
                mesh.duplicate_vertices();
                mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, odm_data.uvs.clone());
                _ = mesh.generate_tangents();

                progress.terrain_mesh = Some(mesh);
                progress.odm_data = Some(odm_data);
                progress.step = progress.step.next();
            }
        }
        LoadingStep::BuildAtlas => {
            if let Some(tile_table) = &progress.tile_table {
                match tile_table.atlas_image(game_assets.lod_manager()) {
                    Ok(mut atlas) => {
                        let mask = openmm_data::image::extract_water_mask(&mut atlas);
                        progress.terrain_texture = Some(crate::assets::dynamic_to_bevy_image(atlas));
                        progress.water_mask = Some(crate::assets::dynamic_to_bevy_image(mask));

                        // Load water texture
                        progress.water_texture = game_assets.lod().bitmap("wtrtyl").map(|img| {
                            let mut water_img = crate::assets::dynamic_to_bevy_image(img);
                            water_img.sampler = crate::assets::repeat_sampler();
                            water_img
                        });

                        progress.step = progress.step.next();
                    }
                    Err(e) => {
                        error!("Failed to build atlas: {}", e);
                    }
                }
            }
        }
        LoadingStep::BuildModels => {
            if let Some(blv) = &progress.blv {
                // Indoor: build meshes from BLV faces
                let mut texture_sizes: HashMap<String, (u32, u32)> = HashMap::new();
                for name in &blv.texture_names {
                    if name.is_empty() || texture_sizes.contains_key(name) {
                        continue;
                    }
                    if let Some(img) = game_assets.lod().bitmap(name) {
                        texture_sizes.insert(name.clone(), (img.width(), img.height()));
                    }
                }
                // Load DLV to get door data
                let dlv_result = openmm_data::dlv::Dlv::new(
                    game_assets.lod_manager(),
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
                            material: StandardMaterial {
                                base_color: Color::WHITE,
                                alpha_mode: AlphaMode::Opaque,
                                cull_mode: None,
                                double_sided: false,
                                perceptual_roughness: 1.0,
                                reflectance: 0.0,
                                metallic: 0.0,
                                ..default()
                            },
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

                            let n = face.num_vertices as usize;
                            let verts: Vec<Vec3> = face.vertex_ids[..n]
                                .iter()
                                .filter_map(|&vid| {
                                    let v = blv.vertices.get(vid as usize)?;
                                    Some(Vec3::from(openmm_data::odm::mm6_to_bevy(
                                        v.x as i32, v.y as i32, v.z as i32,
                                    )))
                                })
                                .collect();
                            if verts.len() < 3 {
                                return None;
                            }

                            // Fan triangulate: tris [0,1,2], [0,2,3], ...
                            let mut base_positions = Vec::with_capacity((n - 2) * 3);
                            let mut is_moving = Vec::with_capacity((n - 2) * 3);
                            for i in 0..n - 2 {
                                let idxs = [0usize, i + 1, i + 2];
                                for &k in &idxs {
                                    base_positions.push(verts[k]);
                                    is_moving.push(moving_vids.contains(&face.vertex_ids[k]));
                                }
                            }

                            // Stored BLV normal in Bevy coords (no ceiling rendering sign flip
                            // — for collision we use the raw geometric normal).
                            let mm6n = face.normal_f32();
                            let normal = Vec3::new(mm6n[0], mm6n[2], -mm6n[1]);
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
                        let verts: Vec<Vec3> = face
                            .vertex_ids
                            .iter()
                            .filter_map(|&vid| {
                                let v = blv.vertices.get(vid as usize)?;
                                Some(Vec3::from(openmm_data::odm::mm6_to_bevy(
                                    v.x as i32, v.y as i32, v.z as i32,
                                )))
                            })
                            .collect();
                        if verts.len() < 3 {
                            return None;
                        }
                        let mm6n = face.normal_f32();
                        let normal = Vec3::new(mm6n[0], mm6n[2], -mm6n[1]);
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
                        let verts: Vec<Vec3> = face
                            .vertex_ids
                            .iter()
                            .filter_map(|&vid| {
                                let v = blv.vertices.get(vid as usize)?;
                                Some(Vec3::from(openmm_data::odm::mm6_to_bevy(
                                    v.x as i32, v.y as i32, v.z as i32,
                                )))
                            })
                            .collect();
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
                    .filter(|(i, f)| {
                        !f.is_invisible() && !f.is_portal() && f.num_vertices >= 3 && !door_faces.contains(i)
                    })
                    .filter_map(|(_, face)| {
                        let verts: Vec<Vec3> = face
                            .vertex_ids
                            .iter()
                            .filter_map(|&vid| {
                                let v = blv.vertices.get(vid as usize)?;
                                Some(Vec3::from(openmm_data::odm::mm6_to_bevy(
                                    v.x as i32, v.y as i32, v.z as i32,
                                )))
                            })
                            .collect();
                        if verts.len() < 3 {
                            return None;
                        }
                        let mm6n = face.normal_f32();
                        let normal = Vec3::new(mm6n[0], mm6n[2], -mm6n[1]);
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
                            let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
                            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, tm.positions);
                            mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, tm.normals);
                            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, tm.uvs);
                            _ = mesh.generate_tangents();
                            let texture = game_assets.lod().bitmap(&tm.texture_name).map(|img| {
                                let mut image = crate::assets::dynamic_to_bevy_image(img);
                                image.sampler = crate::assets::repeat_sampler();
                                image
                            });
                            PreparedSubMesh {
                                mesh,
                                material: StandardMaterial {
                                    base_color: Color::WHITE,
                                    alpha_mode: AlphaMode::Opaque,
                                    // MM6 BSP uses CW winding; from inside the room these are
                                    // back faces in Bevy's CCW convention. cull_mode:None renders
                                    // them, and double_sided MUST be false so normals are not
                                    // flipped — the stored normals already point into the room,
                                    // which is correct for PBR lighting from interior lights.
                                    cull_mode: None,
                                    double_sided: false,
                                    perceptual_roughness: 1.0,
                                    reflectance: 0.0,
                                    metallic: 0.0,
                                    ..default()
                                },
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
                    // Search outdoor EVT files for a MoveToMap targeting this BLV.
                    // In MM6, indoor entry coordinates are in the outdoor map's EVT.
                    let blv_name = match &load_request.map_name {
                        crate::game::map_name::MapName::Indoor(name) => format!("{}.blv", name),
                        _ => String::new(),
                    };
                    // MM6 outdoor maps: outa3..oute3, outb1..oute1, etc.
                    let outdoor_bases: Vec<String> = ('a'..='e')
                        .flat_map(|c| (1..=3).map(move |n| format!("out{}{}", c, n)))
                        .collect();
                    let evt_entry = outdoor_bases.iter().find_map(|base| {
                        openmm_data::evt::EvtFile::parse(game_assets.lod_manager(), base)
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
                        let pos = Vec3::from(openmm_data::odm::mm6_to_bevy(x, y, z));
                        let yaw = (dir as f32) * std::f32::consts::TAU / 65536.0;
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
                            Vec3::from(openmm_data::odm::mm6_to_bevy(cx, cy, floor_z))
                        } else {
                            Vec3::ZERO
                        };
                        (pos, 0.0)
                    }
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
                    crate::game::map_name::MapName::Indoor(name) => name.clone(),
                    _ => load_request.map_name.to_string().replace(".blv", ""),
                };
                // Resolve BLV decorations (torches, chests, etc.)
                let decorations =
                    openmm_data::assets::Decorations::from_blv(game_assets.lod_manager(), &blv.decorations)
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
                        let bmin = Vec3::from(mm6_to_bevy(x0, y0, z0));
                        let bmax = Vec3::from(mm6_to_bevy(x1, y1, z1));
                        // mm6_to_bevy flips Y/Z, so ensure min ≤ max on all axes.
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
                        let pos = Vec3::from(mm6_to_bevy(x as i32, y as i32, z as i32));
                        (pos, l.brightness)
                    })
                    .collect();

                // Resolve DLV actors with dead-actor filtering applied.
                // Resolve BLV spawn-point actors (same pipeline as ODM monsters).
                let resolved_actors = openmm_data::assets::Monsters::load_for_blv(
                    &blv.spawn_points,
                    &load_request.map_name.to_string(),
                    game_assets.game_data(),
                    game_assets.lod_manager(),
                )
                .ok();

                crate::game::events::load_map_events(&mut commands, &game_assets, &map_base, true);
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
                commands.remove_resource::<LoadingProgress>();
                commands.remove_resource::<LoadRequest>();
                game_state.set(GameState::Game);
                return;
            }
            if let Some(odm) = &progress.odm {
                // Collect texture sizes for UV normalization
                let mut texture_sizes: HashMap<String, (u32, u32)> = HashMap::new();
                for b in &odm.bsp_models {
                    for name in &b.texture_names {
                        if !texture_sizes.contains_key(name)
                            && let Some(img) = game_assets.lod().bitmap(name)
                        {
                            texture_sizes.insert(name.clone(), (img.width(), img.height()));
                        }
                    }
                }

                let models = odm
                    .bsp_models
                    .iter()
                    .map(|b| {
                        let textured = b.textured_meshes(&texture_sizes);
                        let sub_meshes = textured
                            .into_iter()
                            .map(|tm| {
                                let mut mesh =
                                    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
                                mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, tm.positions);
                                mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, tm.normals);
                                mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, tm.uvs);
                                _ = mesh.generate_tangents();

                                let texture = game_assets.lod().bitmap(&tm.texture_name).map(|img| {
                                    let mut image = crate::assets::dynamic_to_bevy_image(img);
                                    image.sampler = crate::assets::repeat_sampler();
                                    image
                                });

                                let material = StandardMaterial {
                                    base_color: Color::srgb(1.8, 1.8, 1.8),
                                    alpha_mode: AlphaMode::Opaque,
                                    cull_mode: None,
                                    double_sided: false,
                                    perceptual_roughness: 1.0,
                                    reflectance: 0.0,
                                    metallic: 0.0,
                                    ..default()
                                };

                                PreparedSubMesh {
                                    mesh,
                                    material,
                                    texture,
                                    texture_name: tm.texture_name.clone(),
                                    face_indices: tm.face_indices.clone(),
                                }
                            })
                            .collect();
                        let pos = openmm_data::odm::mm6_to_bevy(
                            b.header.position[0],
                            b.header.position[1],
                            b.header.position[2],
                        );
                        let mut event_ids: Vec<u16> = b
                            .faces
                            .iter()
                            .filter_map(|f| {
                                if f.cog_trigger_id > 0 {
                                    Some(f.cog_trigger_id)
                                } else {
                                    None
                                }
                            })
                            .collect();
                        event_ids.sort_unstable();
                        event_ids.dedup();
                        PreparedModel {
                            sub_meshes,
                            name: b.header.name.clone(),
                            position: Vec3::from(pos),
                            event_ids,
                        }
                    })
                    .collect();
                progress.models = Some(models);
                progress.step = progress.step.next();
            }
        }
        LoadingStep::BuildBillboards => {
            if progress.odm.is_some() {
                let lod = game_assets.lod();
                let mut start_points = Vec::new();

                // Extract start/teleport markers from raw billboard list (Decorations filters these out)
                // Use a scoped block to release the immutable borrow before assigning to progress.
                let decorations = {
                    let odm = progress.odm.as_ref().unwrap();
                    for bb in &odm.billboards {
                        if bb.data.is_original_invisible() {
                            continue;
                        }
                        let is_marker = lod
                            .billboard_item(bb.data.declist_id)
                            .map(|item| item.is_marker() || item.is_no_draw())
                            .unwrap_or(false);
                        let name_lower = bb.declist_name.to_lowercase();
                        if name_lower.contains("start") || is_marker {
                            let pos = Vec3::from(openmm_data::odm::mm6_to_bevy(
                                bb.data.position[0],
                                bb.data.position[1],
                                bb.data.position[2],
                            ));
                            let yaw = bb.data.direction_degrees as f32 * std::f32::consts::PI / 1024.0;
                            start_points.push(StartPoint {
                                name: bb.declist_name.clone(),
                                position: pos,
                                yaw,
                            });
                        }
                    }
                    // Decorations::new filters invisible/marker/no-draw entries automatically
                    openmm_data::assets::Decorations::load(game_assets.lod_manager(), &odm.billboards).ok()
                };
                progress.start_points = Some(start_points);
                progress.decorations = decorations;
                progress.step = progress.step.next();
            }
        }
        LoadingStep::PreloadSprites => {
            // Time-budgeted sprite preloading: process a batch each frame so
            // the window event loop keeps running and GNOME doesn't flag us.
            const PRELOAD_BUDGET_MS: f32 = 8.0;
            let frame_start = std::time::Instant::now();

            // First frame: build the preload queue from map-specific data
            if progress.preload_queue.is_none() {
                let mut sprite_roots: Vec<(String, u8, u16)> = Vec::new();
                let mut seen = std::collections::HashSet::new();

                // DDM actors (NPCs): resolve once, cache for spawn_world reuse
                let map_key = load_request.map_name.to_string();
                let snapshot = world_state.as_ref().and_then(|ws| {
                    ws.game_vars.dead_actor_ids.get(&map_key).map(|ids| {
                        openmm_data::assets::provider::actors::MapStateSnapshot {
                            dead_actor_ids: ids.iter().filter_map(|&id| u16::try_from(id).ok()).collect(),
                        }
                    })
                });
                let lod_actors = openmm_data::assets::Actors::new(
                    game_assets.lod_manager(),
                    &load_request.map_name.to_string(),
                    snapshot.as_ref(),
                    game_assets.game_data(),
                )
                .ok();
                if let Some(ref actors) = lod_actors {
                    for actor in actors.get_actors() {
                        for root in [
                            actor.standing_sprite.clone(),
                            actor.walking_sprite.clone(),
                            actor.attacking_sprite.clone(),
                            actor.dying_sprite.clone(),
                        ] {
                            let key = format!("{}@v{}p{}", root, actor.variant, actor.palette_id);
                            if seen.insert(key) {
                                sprite_roots.push((root, actor.variant, actor.palette_id));
                            }
                        }
                    }
                }
                progress.resolved_actors = lod_actors;

                // Spawn-point monsters: outdoor uses ODM spawn points, indoor uses BLV spawn points.
                let lod_monsters = if load_request.map_name.is_outdoor() {
                    openmm_data::assets::Monsters::load(
                        game_assets.lod_manager(),
                        &load_request.map_name.to_string(),
                        game_assets.game_data(),
                    )
                    .ok()
                } else if let Some(ref blv) = progress.blv {
                    openmm_data::assets::Monsters::load_for_blv(
                        &blv.spawn_points,
                        &load_request.map_name.to_string(),
                        game_assets.game_data(),
                        game_assets.lod_manager(),
                    )
                    .ok()
                } else {
                    None
                };
                if let Some(ref monsters) = lod_monsters {
                    for m in monsters.iter() {
                        for root in [
                            m.standing_sprite.clone(),
                            m.walking_sprite.clone(),
                            m.attacking_sprite.clone(),
                            m.dying_sprite.clone(),
                        ] {
                            let key = format!("{}@v{}p{}", root, m.variant, m.palette_id);
                            if seen.insert(key) {
                                sprite_roots.push((root, m.variant, m.palette_id));
                            }
                        }
                    }
                }
                progress.resolved_monsters = lod_monsters;

                progress.preload_queue = Some(PreloadQueue {
                    sprite_roots,
                    billboard_idx: 0,
                    sprite_idx: 0,
                    music_resolved: false,
                });
            }

            // Resolve music track (cheap, do once)
            {
                let queue = progress.preload_queue.as_mut().unwrap();
                if !queue.music_resolved {
                    queue.music_resolved = true;
                    if let Some(cfg) = game_assets.game_data().mapstats.get(&load_request.map_name.to_string()) {
                        progress.music_track = cfg.music_track;
                    }
                }
            }

            // Preload sprite textures in batches
            {
                let mut cache = progress.sprite_cache.take().unwrap_or_default();
                let queue = progress.preload_queue.as_mut().unwrap();
                while queue.sprite_idx < queue.sprite_roots.len() {
                    if frame_start.elapsed().as_secs_f32() * 1000.0 > PRELOAD_BUDGET_MS {
                        break;
                    }
                    let (root, variant, palette_id) = &queue.sprite_roots[queue.sprite_idx];
                    cache.preload(
                        &[(root.as_str(), *variant, *palette_id)],
                        game_assets.lod_manager(),
                        &mut images,
                        &mut materials,
                    );
                    queue.sprite_idx += 1;
                }
                progress.sprite_cache = Some(cache);
            }

            // Preload billboard/decoration sprites in batches
            {
                let sprites_done = progress.preload_queue.as_ref().unwrap().sprite_idx
                    >= progress.preload_queue.as_ref().unwrap().sprite_roots.len();
                if sprites_done {
                    let mut bb_cache = progress.dec_sprite_cache.take().unwrap_or_default();
                    let lod = game_assets.lod();
                    // Take decorations to allow simultaneous mutable borrow of preload_queue
                    let decorations = progress.decorations.take();
                    if let Some(ref decs) = decorations {
                        let queue = progress.preload_queue.as_mut().unwrap();
                        {
                            while queue.billboard_idx < decs.len() {
                                if frame_start.elapsed().as_secs_f32() * 1000.0 > PRELOAD_BUDGET_MS {
                                    break;
                                }
                                let dec = &decs.entries()[queue.billboard_idx];
                                queue.billboard_idx += 1;
                                if dec.is_directional || dec.num_frames > 1 {
                                    continue;
                                } // directional and animated sprites loaded at spawn time
                                if bb_cache.contains_key(&dec.sprite_name) {
                                    continue;
                                }
                                if let Some(sprite) = lod.billboard(&dec.sprite_name, dec.declist_id) {
                                    let (w, h) = sprite.dimensions();
                                    let rgba = sprite.image.to_rgba8();
                                    let mask = std::sync::Arc::new(
                                        crate::game::entities::sprites::AlphaMask::from_image(&rgba),
                                    );
                                    let bevy_img =
                                        crate::assets::dynamic_to_bevy_image(image::DynamicImage::ImageRgba8(rgba));
                                    let tex = images.add(bevy_img);
                                    let m = materials.add(StandardMaterial {
                                        base_color_texture: Some(tex),
                                        alpha_mode: AlphaMode::Mask(0.5),
                                        unlit: true,
                                        cull_mode: None,
                                        double_sided: true,
                                        perceptual_roughness: 1.0,
                                        reflectance: 0.0,
                                        ..default()
                                    });
                                    let q = meshes.add(Rectangle::new(w, h));
                                    bb_cache.insert(dec.sprite_name.clone(), (m, q, w, h, mask));
                                }
                            }
                        }
                    }
                    progress.decorations = decorations;
                    progress.dec_sprite_cache = Some(bb_cache);
                }
            }

            // Check if all preloading is done
            let queue = progress.preload_queue.as_ref().unwrap();
            let dec_len = progress.decorations.as_ref().map_or(0, |d| d.len());
            if queue.sprite_idx >= queue.sprite_roots.len() && queue.billboard_idx >= dec_len {
                progress.preload_queue = None;
                progress.step = progress.step.next();
            }
        }
        LoadingStep::Done => {
            // Move all prepared data into PreparedWorld resource
            let odm = progress.odm.take();
            let terrain_mesh = progress.terrain_mesh.take();
            let terrain_texture = progress.terrain_texture.take();
            let models = progress.models.take();

            if let (Some(map), Some(mesh), Some(texture), Some(models)) = (odm, terrain_mesh, terrain_texture, models) {
                let water_cells = progress.water_cells.take().unwrap_or_default();
                let water_texture = progress.water_texture.take();
                let map_base = match &load_request.map_name {
                    crate::game::map_name::MapName::Outdoor(odm) => format!("out{}{}", odm.x, odm.y),
                    other => other.to_string(),
                };
                crate::game::events::load_map_events(&mut commands, &game_assets, &map_base, false);
                commands.insert_resource(PreparedWorld {
                    map,
                    terrain_mesh: mesh,
                    terrain_texture: texture,
                    water_mask: progress.water_mask.take(),
                    water_texture,
                    water_cells,
                    models,
                    decorations: progress
                        .decorations
                        .take()
                        .unwrap_or_else(openmm_data::assets::Decorations::empty),
                    resolved_actors: progress.resolved_actors.take(),
                    resolved_monsters: progress.resolved_monsters.take(),
                    start_points: progress.start_points.take().unwrap_or_default(),
                    sprite_cache: progress.sprite_cache.take().unwrap_or_default(),
                    dec_sprite_cache: progress.dec_sprite_cache.take().unwrap_or_default(),
                    terrain_lookup: progress
                        .terrain_lookup
                        .take()
                        .unwrap_or_else(openmm_data::terrain::TerrainLookup::empty),
                    music_track: progress.music_track,
                });
                commands.remove_resource::<LoadingProgress>();
                commands.remove_resource::<LoadRequest>();
                game_state.set(GameState::Game);
            }
        }
    }
}

/// Extract collision walls and floors from BLV face geometry.
/// `door_faces` contains face indices to exclude — animated door faces have their
/// own moving geometry and must not remain as static collision obstacles.
fn extract_blv_collision(
    blv: &Blv,
    door_faces: &std::collections::HashSet<usize>,
) -> (
    Vec<crate::game::collision::CollisionWall>,
    Vec<crate::game::collision::CollisionTriangle>,
    Vec<crate::game::collision::CollisionTriangle>,
) {
    use crate::game::collision::{CollisionTriangle, CollisionWall};
    use openmm_data::enums::PolygonType;
    use openmm_data::odm::mm6_to_bevy;

    let mut walls = Vec::new();
    let mut floors = Vec::new();
    let mut ceilings = Vec::new();

    for (face_idx, face) in blv.faces.iter().enumerate() {
        if face.num_vertices < 3 || face.is_invisible() || face.is_portal() {
            continue;
        }

        // Face normal: MM6 fixed-point (x,y,z) → Bevy float (x,z,-y)
        let mm6n = face.normal_f32();
        let normal = Vec3::new(mm6n[0], mm6n[2], -mm6n[1]);

        // Classify face by polygon_type, with normal-direction fallback for in-between types.
        // Pure Floor (3) and Ceiling (5) are authoritative by polygon_type.
        // InBetweenFloorAndWall (4) and InBetweenCeilingAndWall (6) can have normals anywhere
        // between horizontal and vertical — only classify as floor/ceiling when their normal
        // clearly points that way; otherwise they block lateral movement as walls.
        let poly = face.polygon_type_enum();
        let is_floor = matches!(poly, Some(PolygonType::Floor))
            || (matches!(poly, Some(PolygonType::InBetweenFloorAndWall)) && normal.y > 0.1);
        let is_ceiling = matches!(poly, Some(PolygonType::Ceiling))
            || (matches!(poly, Some(PolygonType::InBetweenCeilingAndWall)) && normal.y < -0.5);
        // VerticalWall is always a wall; anything else with a mostly-vertical normal also blocks.
        let is_wall =
            matches!(poly, Some(PolygonType::VerticalWall)) || (!is_floor && !is_ceiling && normal.y.abs() < 0.7);

        // Only skip wall door faces — they move and have their own DoorColliders.
        // Floor/ceiling door faces stay in static collision so the player doesn't fall
        // through at door thresholds.
        if door_faces.contains(&face_idx) && is_wall {
            continue;
        }

        // Collect vertices in Bevy coords
        let verts: Vec<Vec3> = face
            .vertex_ids
            .iter()
            .filter_map(|&vid| {
                let v = blv.vertices.get(vid as usize)?;
                Some(Vec3::from(mm6_to_bevy(v.x as i32, v.y as i32, v.z as i32)))
            })
            .collect();
        if verts.len() < 3 {
            continue;
        }

        if is_wall {
            let plane_dist = normal.dot(verts[0]);
            walls.push(CollisionWall::new(normal, plane_dist, &verts));
        }

        if is_floor || is_ceiling {
            for i in 0..verts.len().saturating_sub(2) {
                let tri = CollisionTriangle::new(verts[0], verts[i + 1], verts[i + 2], normal);
                if is_floor {
                    floors.push(tri.clone());
                }
                if is_ceiling {
                    ceilings.push(tri);
                }
            }
        }
    }

    (walls, floors, ceilings)
}
