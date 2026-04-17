//! Time-budgeted per-frame spawning of decorations, NPC actors and ODM monsters.
//!
//! `spawn_world` builds the distance-sorted spawn order; this module drains it
//! over multiple frames so the loading→game transition stays smooth.

use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;

use crate::assets::GameAssets;
use crate::game::coords::mm6_position_to_bevy;
use crate::game::optional::OptionalWrite;
use crate::game::spawn::SpawnCtx;
use crate::game::spawn::decoration::{DecSpriteCache, spawn_decoration};
use crate::game::sprites::loading as sprites;
use crate::game::sprites::material::SpriteMaterial;
use crate::prepare::loading::PreparedWorld;

use super::spawn_actors::{spawn_npc_actors, spawn_odm_monsters};

/// Spawn progress visible to the debug HUD.
#[derive(Resource, Default)]
pub struct SpawnProgress {
    pub total: usize,
    pub done: usize,
}

/// Pending entities sorted by distance from player, spawned gradually.
#[derive(Resource)]
pub(super) struct PendingSpawns {
    pub billboard_order: Vec<usize>,
    pub actor_order: Vec<usize>,
    pub idx: usize,
    pub frames_elapsed: u32,
    pub sprite_cache: sprites::SpriteCache,
    /// Cached billboard materials: key = sprite name, value = (material, mesh, width, height, mask)
    pub dec_sprite_cache: DecSpriteCache,
    /// Pre-resolved decoration entries for this map (directional detection, sprite names, dimensions).
    pub decorations: openmm_data::assets::Decorations,
    /// Pre-resolved DDM actors (NPCs only for outdoor maps) for this map.
    pub actors: Option<openmm_data::assets::Actors>,
    /// ODM spawn-point monsters (outdoor only). Each entry is one group member.
    pub monsters: Option<openmm_data::assets::Monsters>,
    pub monster_order: Vec<usize>,
    pub terrain_entity: Entity,
}

/// Max time budget per frame for entity spawning (milliseconds).
/// Keeps frame time from ballooning when spawning many entities.
pub(super) const SPAWN_TIME_BUDGET_MS: f32 = 4.0;
/// Hard cap on entities per frame even if time budget allows.
pub(super) const SPAWN_BATCH_MAX: usize = 12;
/// On the first frame, spawn all entities with no budget limit.
/// The loading-to-game transition masks this single long frame.
pub(super) const EAGER_SPAWN_FRAMES: u32 = 1;

/// Sort indices by distance from player using MM6 coords (works with i16 or i32).
pub(super) fn sort_by_distance_mm6<T>(
    items: &[T],
    player: Vec3,
    pos_x: impl Fn(&T) -> f32,
    pos_y: impl Fn(&T) -> f32,
) -> Vec<usize> {
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by(|&a, &b| {
        let da = (pos_x(&items[a]) - player.x).powi(2) + (pos_y(&items[a]) + player.z).powi(2);
        let db = (pos_x(&items[b]) - player.x).powi(2) + (pos_y(&items[b]) + player.z).powi(2);
        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
    });
    order
}

/// Spawn a small batch of entities per frame from the pending queue.
/// Uses a time budget to avoid spending too long per frame.
pub(super) fn lazy_spawn(
    mut commands: Commands,
    pending: Option<ResMut<PendingSpawns>>,
    prepared: Res<PreparedWorld>,
    game_assets: Res<GameAssets>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    _materials: ResMut<Assets<StandardMaterial>>,
    mut sprite_materials: Option<ResMut<Assets<SpriteMaterial>>>,
    mut progress: ResMut<SpawnProgress>,
    mut sound_events: Option<MessageWriter<crate::game::sound::effects::PlaySoundEvent>>,
    mut map_events: Option<ResMut<crate::game::state::MapEvents>>,
    cfg: Res<crate::system::config::GameConfig>,
) {
    let Some(mut pending) = pending else {
        return;
    };

    let p = &mut *pending;
    let terrain_entity = p.terrain_entity;
    let start = std::time::Instant::now();

    // On the first frame, spawn all entities with no budget — the loading-to-game
    // transition masks this single long frame. After that, use the normal budget.
    let eager = p.frames_elapsed < EAGER_SPAWN_FRAMES;
    let time_budget = if eager { f32::MAX } else { SPAWN_TIME_BUDGET_MS };
    let batch_max = if eager { usize::MAX } else { SPAWN_BATCH_MAX };
    p.frames_elapsed += 1;
    let mut spawned = 0;

    let (bb_len, actor_len, monster_len) = (p.billboard_order.len(), p.actor_order.len(), p.monster_order.len());
    let mut bb_idx = p.idx.min(bb_len);
    let mut actor_idx = p.idx.saturating_sub(bb_len).min(actor_len);
    let mut monster_idx = p.idx.saturating_sub(bb_len + actor_len).min(monster_len);

    if p.frames_elapsed == 1 {
        let n_ddm_npcs = p.actors.as_ref().map(|a| a.get_npcs().count()).unwrap_or(0);
        warn!(
            "lazy_spawn: {} decorations, {} DDM NPCs, {} ODM monsters",
            bb_len, n_ddm_npcs, monster_len
        );
    }

    // Unwrap sprite_materials — the material plugin is always present in practice.
    let sprite_mats = sprite_materials
        .as_deref_mut()
        .expect("SpriteMaterial asset store missing");

    // Temporarily take the sprite cache out of PendingSpawns so it can live
    // inside SpawnCtx without conflicting with the &mut PendingSpawns borrows
    // that decorations/actors still need for their own fields.
    let mut sprite_cache = std::mem::take(&mut p.sprite_cache);

    let mut ctx = SpawnCtx {
        game_assets: &game_assets,
        images: &mut images,
        meshes: &mut meshes,
        sprite_materials: sprite_mats,
        sprite_cache: &mut sprite_cache,
        shadows: cfg.shadows,
        billboard_shadows: cfg.billboard_shadows,
        actor_shadows: cfg.actor_shadows,
    };

    // Decorations
    {
        let bb_len = p.billboard_order.len();
        while bb_idx < bb_len && spawned < batch_max && start.elapsed().as_secs_f32() * 1000.0 < time_budget {
            let dec_idx = p.billboard_order[bb_idx];
            bb_idx += 1;
            let dec = &p.decorations.entries()[dec_idx];
            let dec_pos = Vec3::from(mm6_position_to_bevy(dec.position[0], dec.position[1], dec.position[2]));

            if spawn_decoration(
                &mut commands,
                &mut ctx,
                dec,
                dec_pos,
                Some(terrain_entity),
                &mut p.dec_sprite_cache,
            )
            .is_some()
            {
                spawned += 1;
            }
            if dec.sound_id > 0 {
                sound_events.try_write(crate::game::sound::effects::PlaySoundEvent {
                    sound_id: dec.sound_id as u32,
                    position: dec_pos,
                });
            }
        }
    }
    spawn_npc_actors(
        &mut commands,
        &mut ctx,
        p,
        &prepared,
        start,
        time_budget,
        batch_max,
        terrain_entity,
        &mut actor_idx,
        &mut spawned,
        &mut map_events,
    );
    spawn_odm_monsters(
        &mut commands,
        &mut ctx,
        p,
        &prepared,
        start,
        time_budget,
        batch_max,
        terrain_entity,
        &mut monster_idx,
        &mut spawned,
    );

    // Put the sprite cache back.
    p.sprite_cache = sprite_cache;

    p.idx = bb_idx + actor_idx + monster_idx;
    progress.done = p.idx;

    if p.idx >= bb_len + actor_len + monster_len {
        commands.remove_resource::<PendingSpawns>();
    }
}
