//! Time-budgeted per-frame spawning of decorations, NPC actors and ODM monsters.
//!
//! `spawn_world` builds the distance-sorted spawn order; this module drains it
//! over multiple frames so the loading→game transition stays smooth.

use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;

use crate::assets::GameAssets;
use crate::game::entities::sprites;
use crate::game::game_time::GameTime;
use crate::game::lighting::sprite_tint_from_time;
use crate::game::sprite_material::SpriteMaterial;
use crate::states::loading::PreparedWorld;

use super::spawn_actors::{spawn_npc_actors, spawn_odm_monsters};
use super::spawn_decorations::spawn_decorations;

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
    pub dec_sprite_cache: std::collections::HashMap<
        String,
        (
            Handle<SpriteMaterial>,
            Handle<Mesh>,
            f32,
            f32,
            std::sync::Arc<crate::game::entities::sprites::AlphaMask>,
        ),
    >,
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

/// Common context passed down into the per-kind spawn helpers, so each one
/// only takes a handful of parameters instead of 14+.
pub(super) struct SpawnCtx<'a> {
    pub game_assets: &'a GameAssets,
    pub images: &'a mut Assets<Image>,
    pub meshes: &'a mut Assets<Mesh>,
    pub sprite_materials: Option<&'a mut Assets<SpriteMaterial>>,
    pub spawn_tint: Vec4,
    pub start: std::time::Instant,
    pub time_budget: f32,
    pub batch_max: usize,
    pub terrain_entity: Entity,
}

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
    prepared: Option<Res<PreparedWorld>>,
    game_assets: Res<GameAssets>,
    game_time: Res<GameTime>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    _materials: ResMut<Assets<StandardMaterial>>,
    mut sprite_materials: Option<ResMut<Assets<SpriteMaterial>>>,
    mut progress: ResMut<SpawnProgress>,
    mut sound_events: Option<MessageWriter<crate::game::sound::effects::PlaySoundEvent>>,
    mut map_events: Option<ResMut<crate::game::events::MapEvents>>,
) {
    let (Some(mut pending), Some(prepared)) = (pending, prepared) else {
        return;
    };

    // Compute the correct day/night tint for this frame so new materials are pre-tinted.
    // animate_day_cycle uses a change-threshold and won't re-tint materials that weren't in
    // the ECS when it ran — baking the tint at creation time avoids the brief bright flash.
    let tint_color = sprite_tint_from_time(game_time.time_of_day());
    let tl = tint_color.to_linear();
    let spawn_tint = Vec4::new(tl.red, tl.green, tl.blue, 1.0);

    let p = &mut *pending;
    let terrain_entity = p.terrain_entity;
    let mut spawned = 0;
    let start = std::time::Instant::now();

    // On the first frame, spawn all entities with no budget — the loading-to-game
    // transition masks this single long frame. After that, use the normal budget.
    let eager = p.frames_elapsed < EAGER_SPAWN_FRAMES;
    let time_budget = if eager { f32::MAX } else { SPAWN_TIME_BUDGET_MS };
    let batch_max = if eager { usize::MAX } else { SPAWN_BATCH_MAX };
    p.frames_elapsed += 1;

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

    let mut ctx = SpawnCtx {
        game_assets: &game_assets,
        images: &mut images,
        meshes: &mut meshes,
        sprite_materials: sprite_materials.as_deref_mut(),
        spawn_tint,
        start,
        time_budget,
        batch_max,
        terrain_entity,
    };

    spawn_decorations(&mut commands, &mut ctx, p, &mut bb_idx, &mut spawned, &mut sound_events);
    spawn_npc_actors(
        &mut commands,
        &mut ctx,
        p,
        &prepared,
        &mut actor_idx,
        &mut spawned,
        &mut map_events,
    );
    spawn_odm_monsters(&mut commands, &mut ctx, p, &prepared, &mut monster_idx, &mut spawned);

    p.idx = bb_idx + actor_idx + monster_idx;
    progress.done = p.idx;

    if p.idx >= bb_len + actor_len + monster_len {
        commands.remove_resource::<PendingSpawns>();
    }
}
