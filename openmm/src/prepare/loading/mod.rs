//! Map loading pipeline — step-based loader that parses, builds, and preloads map data.
mod helpers;
mod indoor;
mod outdoor;

use bevy::prelude::*;

use crate::{GameState, assets::GameAssets, despawn_all, game::map::CurrentMap, system::config::GameConfig};
use openmm_data::{
    blv::Blv,
    dtile::{Dtile, TileTable},
    odm::{Odm, OdmData},
    utils::MapName,
};

// Re-export types so existing `crate::states::loading::X` paths keep working.
pub use super::prepared::{
    ClickableFaceData, OccluderFaceData, PreparedDoorCollision, PreparedDoorFace, PreparedIndoorWorld, PreparedModel,
    PreparedSubMesh, PreparedWorld, SectorAmbient, StartPoint, TouchTriggerFaceData, texture_emissive,
};

pub struct LoadingPlugin;

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(GameState::Loading),
            (despawn_all::<crate::game::InGame>, loading_setup).chain(),
        )
        .add_systems(Update, loading_step.run_if(in_state(GameState::Loading)))
        .add_systems(OnExit(GameState::Loading), cleanup_loading_step);
    }
}

fn cleanup_loading_step(mut commands: Commands) {
    commands.remove_resource::<LoadingStep>();
}

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
pub(crate) struct LoadingProgress {
    pub(crate) step: LoadingStep,
    pub(crate) odm: Option<Odm>,
    pub(crate) tile_table: Option<TileTable>,
    pub(crate) odm_data: Option<OdmData>,
    pub(crate) terrain_mesh: Option<Mesh>,
    pub(crate) terrain_texture: Option<Image>,
    pub(crate) water_mask: Option<Image>,
    pub(crate) water_texture: Option<Image>,
    pub(crate) models: Option<Vec<PreparedModel>>,
    pub(crate) decorations: Option<openmm_data::assets::Decorations>,
    pub(crate) resolved_actors: Option<openmm_data::assets::Actors>,
    pub(crate) resolved_monsters: Option<openmm_data::assets::Monsters>,
    pub(crate) start_points: Option<Vec<StartPoint>>,
    pub(crate) sprite_cache: Option<crate::game::sprites::loading::SpriteCache>,
    pub(crate) dec_sprite_cache: Option<
        std::collections::HashMap<
            String,
            (
                Handle<crate::game::sprites::material::SpriteMaterial>,
                Handle<Mesh>,
                f32,
                f32,
                std::sync::Arc<crate::game::sprites::loading::AlphaMask>,
            ),
        >,
    >,
    pub(crate) water_cells: Option<Vec<bool>>,
    pub(crate) terrain_lookup: Option<openmm_data::terrain::TerrainLookup>,
    pub(crate) music_track: u8,
    pub(crate) blv: Option<Blv>,
    /// Queued sprite preload work, processed in batches across frames.
    pub(crate) preload_queue: Option<PreloadQueue>,
}

/// Queued sprite preload work items, processed across multiple frames.
pub(crate) struct PreloadQueue {
    /// (sprite_root, variant, palette_id) triples to preload into SpriteCache.
    pub(crate) sprite_roots: Vec<(String, u8, u16)>,
    /// Index into progress.decorations.entries() for billboard preloading.
    pub(crate) billboard_idx: usize,
    /// Current position in sprite_roots.
    pub(crate) sprite_idx: usize,
    /// Whether music track has been resolved.
    pub(crate) music_resolved: bool,
}

/// Current loading pipeline step — published as a resource so the screen
/// binding system can drive the loading animation independently.
#[derive(Default, Clone, Copy, PartialEq, Eq, Resource)]
pub enum LoadingStep {
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
    pub fn label(&self) -> &'static str {
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

    /// Ordinal index (0-based) for animation frame sequencing.
    pub fn index(&self) -> usize {
        match self {
            Self::ParseMap => 0,
            Self::BuildTerrain => 1,
            Self::BuildAtlas => 2,
            Self::BuildModels => 3,
            Self::BuildBillboards => 4,
            Self::PreloadSprites => 5,
            Self::Done => 6,
        }
    }

    pub(crate) fn next(&self) -> Self {
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

fn loading_setup(
    mut commands: Commands,
    load_request: Option<Res<LoadRequest>>,
    active_save: Res<crate::game::save::ActiveSave>,
    cfg: Res<GameConfig>,
    mut world_state: ResMut<crate::game::state::WorldState>,
    mut party: ResMut<crate::game::player::party::Party>,
    mut game_time: ResMut<crate::game::state::GameTime>,
) {
    // Clean up resources from previous map (indoor or outdoor)
    commands.remove_resource::<PreparedWorld>();
    commands.remove_resource::<PreparedIndoorWorld>();
    commands.remove_resource::<crate::game::interaction::clickable::Faces>();
    commands.remove_resource::<crate::game::map::indoor::BlvDoors>();
    commands.remove_resource::<crate::game::map::indoor::DoorColliders>();
    commands.remove_resource::<crate::game::map::indoor::TouchTriggerFaces>();
    commands.remove_resource::<crate::game::map::indoor::OccluderFaces>();
    commands.remove_resource::<crate::game::ui::MapOverviewImage>();
    commands.remove_resource::<CurrentMap>();

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
            .unwrap_or_else(|| active_save.map_name.clone());
        (name, None, None)
    };

    // Populate live game state from the save file
    crate::game::save::load::populate_state_from_save(&active_save, &mut world_state, &mut party, &mut game_time);

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

    // Publish loading step so the screen binding can drive the animation.
    commands.insert_resource(LoadingStep::default());
}

fn loading_step(
    mut progress: ResMut<LoadingProgress>,
    game_assets: Res<GameAssets>,
    load_request: Res<LoadRequest>,
    mut game_state: ResMut<NextState<GameState>>,
    mut commands: Commands,
    mut step_res: ResMut<LoadingStep>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut sprite_materials: Option<ResMut<Assets<crate::game::sprites::material::SpriteMaterial>>>,
    world_state: Option<Res<crate::game::state::WorldState>>,
    active_save: Res<crate::game::save::ActiveSave>,
) {
    // Keep the public resource in sync with internal progress.
    step_res.set_if_neq(progress.step);

    match progress.step {
        LoadingStep::ParseMap => {
            step_parse_map(
                &mut progress,
                &game_assets,
                &load_request,
                &mut commands,
                &mut game_state,
            );
        }
        LoadingStep::BuildTerrain => {
            outdoor::step_build_terrain(&mut progress);
        }
        LoadingStep::BuildAtlas => {
            outdoor::step_build_atlas(&mut progress, &game_assets);
        }
        LoadingStep::BuildModels => {
            step_build_models(
                &mut progress,
                &game_assets,
                &load_request,
                &mut commands,
                &mut game_state,
                &active_save,
                world_state.as_deref(),
            );
        }
        LoadingStep::BuildBillboards => {
            outdoor::step_build_billboards(&mut progress, &game_assets);
        }
        LoadingStep::PreloadSprites => {
            step_preload_sprites(
                &mut progress,
                &game_assets,
                &load_request,
                &mut images,
                &mut meshes,
                sprite_materials.as_deref_mut(),
                world_state.as_deref(),
                &active_save,
            );
        }
        LoadingStep::Done => {
            outdoor::step_done(
                &mut progress,
                &game_assets,
                &load_request,
                &mut commands,
                &mut game_state,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Step helpers — one per LoadingStep variant. Each receives only the subset
// of system params it actually needs.
// ---------------------------------------------------------------------------

fn step_parse_map(
    progress: &mut LoadingProgress,
    game_assets: &GameAssets,
    load_request: &LoadRequest,
    commands: &mut Commands,
    game_state: &mut NextState<GameState>,
) {
    let map_name = load_request.map_name.to_string();
    if load_request.map_name.is_indoor() {
        match Blv::load(game_assets.assets(), &map_name) {
            Ok(blv) => {
                progress.blv = Some(blv);
                // Skip terrain — jump straight to BuildModels
                progress.step = LoadingStep::BuildModels;
            }
            Err(e) => {
                error!("Failed to parse indoor map {}: {}", map_name, e);
                commands.remove_resource::<LoadRequest>();
                game_state.set(GameState::Menu);
            }
        }
        return;
    }
    match Odm::load(game_assets.assets(), &map_name) {
        Ok(odm) => {
            match odm.tile_table(game_assets.assets()) {
                Ok(tile_table) => {
                    // Build water map from tile data
                    if let Ok(dtile) = Dtile::load(game_assets.assets()) {
                        let water_cells: Vec<bool> =
                            odm.tile_map.iter().map(|&idx| dtile.is_deep_water_tile(idx)).collect();
                        progress.water_cells = Some(water_cells);
                        progress.terrain_lookup = Some(openmm_data::terrain::TerrainLookup::new(&dtile, odm.tile_data));
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

fn step_build_models(
    progress: &mut LoadingProgress,
    game_assets: &GameAssets,
    load_request: &LoadRequest,
    commands: &mut Commands,
    game_state: &mut NextState<GameState>,
    active_save: &crate::game::save::ActiveSave,
    world_state: Option<&crate::game::state::WorldState>,
) {
    if progress.blv.is_some() {
        indoor::step_build_models_indoor(
            progress,
            game_assets,
            load_request,
            commands,
            game_state,
            active_save,
            world_state,
        );
        return;
    }
    if progress.odm.is_some() {
        outdoor::step_build_models_outdoor(progress, game_assets);
    }
}

fn step_preload_sprites(
    progress: &mut LoadingProgress,
    game_assets: &GameAssets,
    load_request: &LoadRequest,
    images: &mut Assets<Image>,
    meshes: &mut Assets<Mesh>,
    mut sprite_materials: Option<&mut Assets<crate::game::sprites::material::SpriteMaterial>>,
    world_state: Option<&crate::game::state::WorldState>,
    active_save: &crate::game::save::ActiveSave,
) {
    // Time-budgeted sprite preloading: process a batch each frame so
    // the window event loop keeps running and GNOME doesn't flag us.
    const PRELOAD_BUDGET_MS: f32 = 8.0;
    let frame_start = std::time::Instant::now();

    // First frame: build the preload queue from map-specific data
    if progress.preload_queue.is_none() {
        build_preload_queue(progress, game_assets, load_request, world_state, active_save);
    }

    // Resolve music track (cheap, do once)
    {
        let queue = progress.preload_queue.as_mut().unwrap();
        if !queue.music_resolved {
            queue.music_resolved = true;
            if let Some(cfg) = game_assets.data().mapstats.get(&load_request.map_name.to_string()) {
                progress.music_track = cfg.music_track;
            }
        }
    }

    // Preload sprite textures in batches
    if let Some(ref mut sprite_materials) = sprite_materials {
        let mut cache = progress.sprite_cache.take().unwrap_or_default();
        let queue = progress.preload_queue.as_mut().unwrap();
        while queue.sprite_idx < queue.sprite_roots.len() {
            if frame_start.elapsed().as_secs_f32() * 1000.0 > PRELOAD_BUDGET_MS {
                break;
            }
            let (root, variant, palette_id) = &queue.sprite_roots[queue.sprite_idx];
            cache.preload(
                &[(root.as_str(), *variant, *palette_id)],
                game_assets.assets(),
                images,
                sprite_materials,
                false,
            );
            queue.sprite_idx += 1;
        }
        progress.sprite_cache = Some(cache);
    } else {
        // Sprite materials plugin disabled — skip all sprite + billboard preloading.
        let queue = progress.preload_queue.as_mut().unwrap();
        queue.sprite_idx = queue.sprite_roots.len();
        queue.billboard_idx = progress.decorations.as_ref().map_or(0, |d| d.len());
    }
    if let Some(sprite_materials) = sprite_materials {
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
                            let (m, mask) = crate::game::sprites::loading::sprite_to_material_with_mask(
                                rgba,
                                images,
                                sprite_materials,
                                false,
                            );
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

/// Try loading DDM actors from the active save file. Returns `Some(Actors)` if the save
/// contains a DDM for this map (i.e. the map was previously visited and saved), `None` otherwise.
pub(super) fn try_load_actors_from_save(
    active_save: &crate::game::save::ActiveSave,
    map_name: &str,
    state: Option<&openmm_data::assets::provider::actors::MapStateSnapshot>,
    game_assets: &GameAssets,
) -> Option<openmm_data::assets::Actors> {
    let ddm_filename = format!("{}.ddm", map_name);
    let dlv_filename = format!("{}.dlv", map_name);
    let save_file = openmm_data::save::SaveFile::open(&active_save.path).ok()?;
    let (data, is_dlv) = if let Some(d) = save_file.get_file_ci(&ddm_filename) {
        (d, false)
    } else if let Some(d) = save_file.get_file_ci(&dlv_filename) {
        (d, true)
    } else {
        return None;
    };

    info!(
        "loaded {} for '{}' from save file ({} bytes)",
        if is_dlv { "DLV" } else { "DDM" },
        map_name,
        data.len()
    );

    let raw_actors = openmm_data::assets::ddm::Ddm::parse_from_data(&data).ok()?;
    openmm_data::assets::Actors::from_raw_actors(game_assets.assets(), &raw_actors, state, game_assets.data()).ok()
}

/// First-frame initialization for PreloadSprites: resolve actors and monsters,
/// collect unique sprite roots, and build the preload queue.
fn build_preload_queue(
    progress: &mut LoadingProgress,
    game_assets: &GameAssets,
    load_request: &LoadRequest,
    world_state: Option<&crate::game::state::WorldState>,
    active_save: &crate::game::save::ActiveSave,
) {
    let mut sprite_roots: Vec<(String, u8, u16)> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    let map_key = load_request.map_name.to_string();
    let snapshot = world_state.and_then(|ws| {
        ws.game_vars
            .dead_actor_ids
            .get(&map_key)
            .map(|ids| openmm_data::assets::provider::actors::MapStateSnapshot {
                dead_actor_ids: ids.iter().filter_map(|&id| u16::try_from(id).ok()).collect(),
            })
    });

    // Try loading DDM/DLV from save file first (preserves killed monster state),
    // fall back to LOD archives for maps not yet visited.
    let mut loaded_from_save = false;
    let lod_actors = try_load_actors_from_save(active_save, &map_key, snapshot.as_ref(), game_assets)
        .map(|a| {
            loaded_from_save = true;
            a
        })
        .or_else(|| {
            openmm_data::assets::Actors::new(
                game_assets.assets(),
                &map_key,
                snapshot.as_ref(),
                game_assets.data(),
            )
            .ok()
        });

    // Collect unique sprite roots from a set of entities with sprite fields.
    let mut collect_sprites = |sprites: &[(String, String, String, String, u8, u16)]| {
        for (standing, walking, attacking, dying, variant, palette_id) in sprites {
            for root in [standing, walking, attacking, dying] {
                let key = format!("{}@v{}p{}", root, variant, palette_id);
                if seen.insert(key) {
                    sprite_roots.push((root.clone(), *variant, *palette_id));
                }
            }
        }
    };

    if let Some(ref actors) = lod_actors {
        let entries: Vec<_> = actors
            .get_actors()
            .iter()
            .map(|a| {
                (
                    a.standing_sprite.clone(),
                    a.walking_sprite.clone(),
                    a.attacking_sprite.clone(),
                    a.dying_sprite.clone(),
                    a.variant,
                    a.palette_id,
                )
            })
            .collect();
        collect_sprites(&entries);
    }

    progress.resolved_actors = lod_actors;

    // Spawn-point monsters (outdoor only — indoor transitions directly to Game).
    // Skip if we already loaded the entire map state from a save file.
    let lod_monsters = if load_request.map_name.is_outdoor() && !loaded_from_save {
        openmm_data::assets::Monsters::load(
            game_assets.assets(),
            &load_request.map_name.to_string(),
            game_assets.data(),
        )
        .ok()
    } else {
        None
    };

    if let Some(ref monsters) = lod_monsters {
        let entries: Vec<_> = monsters
            .entries()
            .iter()
            .map(|m| {
                (
                    m.standing_sprite.clone(),
                    m.walking_sprite.clone(),
                    m.attacking_sprite.clone(),
                    m.dying_sprite.clone(),
                    m.variant,
                    m.palette_id,
                )
            })
            .collect();
        collect_sprites(&entries);
    }
    progress.resolved_monsters = lod_monsters;

    // Outdoor decorations
    if let Some(ref _odm) = progress.odm {
        let entries: Vec<_> = progress
            .decorations
            .as_ref()
            .unwrap()
            .entries()
            .iter()
            .map(|d| {
                (
                    d.sprite_name.clone(),
                    String::new(),
                    String::new(),
                    String::new(),
                    1,
                    0,
                )
            })
            .collect();
        collect_sprites(&entries);

        // Map music track
        progress.music_track = game_assets
            .data()
            .mapstats
            .get(&load_request.map_name.to_string())
            .map(|mi| mi.music_track)
            .unwrap_or(0);
    }

    progress.preload_queue = Some(PreloadQueue {
        sprite_roots,
        billboard_idx: 0,
        sprite_idx: 0,
        music_resolved: false,
    });
}
