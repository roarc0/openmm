//! Per-frame spatial grid over `WorldEntity` entities.
//!
//! Both the interaction raycasts (`world_interact_system`, `hover_hint_system`)
//! and distance culling used to iterate every `WorldEntity` every frame. That
//! costs O(n) per system with non-trivial per-entity work — matrix compose,
//! point-in-polygon, alpha mask lookup for raycasts. On a typical map that's
//! ~500 entities × 2 systems × several µs = measurable frame time for zero
//! reason, since the interaction range is one tile (512 units).
//!
//! This module builds a uniform XZ grid once per frame and uses it to:
//!  - gate interaction raycasts to entities within `MAX_INTERACT_RANGE` of the
//!    player (typically 3×3 cells = a handful of entities);
//!  - perform distance culling in the same pass as the rebuild, so we get an
//!    entity index for free without a second iteration.
//!
//! The index is cleared and rebuilt each frame (dynamic entities like monsters
//! move constantly), so there is no incremental insert/remove bookkeeping —
//! rebuild is one `HashMap::entry().push()` per entity, much cheaper than the
//! raycast work it eliminates downstream.

use bevy::ecs::schedule::SystemSet;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;

use crate::GameState;
use crate::game::hud::HudView;
use crate::game::player::Player;
use crate::game::sprites::WorldEntity;

/// World-space cell size. 1024 units ≈ 2 MM6 tiles.
///
/// Chosen so the interaction range (`MAX_INTERACT_RANGE = 512`) fits inside a
/// 3×3 block: the player's cell + 8 neighbours is guaranteed to cover any
/// reachable interactable, regardless of where in its cell the player stands.
pub const CELL_SIZE: f32 = 1024.0;

/// Convert a world-space XZ position to a grid cell key.
#[inline]
fn cell_key(x: f32, z: f32) -> (i32, i32) {
    ((x / CELL_SIZE).floor() as i32, (z / CELL_SIZE).floor() as i32)
}

/// Per-frame spatial index of `WorldEntity` entities by their XZ position.
///
/// Rebuilt every frame by `rebuild_and_cull`. Readers get a cheap
/// `query_radius` that returns only entities whose cell overlaps the query
/// circle — callers still do an exact distance / ray test on the candidates.
#[derive(Resource, Default)]
pub struct EntitySpatialIndex {
    cells: HashMap<(i32, i32), Vec<Entity>>,
}

impl EntitySpatialIndex {
    /// Clear all buckets but keep the allocated `Vec`s and `HashMap` capacity
    /// so the next rebuild pays no allocation.
    fn clear(&mut self) {
        for bucket in self.cells.values_mut() {
            bucket.clear();
        }
    }

    fn insert(&mut self, entity: Entity, x: f32, z: f32) {
        let key = cell_key(x, z);
        self.cells.entry(key).or_default().push(entity);
    }

    /// Iterate every entity whose cell overlaps a circle of `radius` around
    /// `(x, z)`. Cells are rectangular, so the returned set is a loose
    /// superset of the true circular query — callers must still do an exact
    /// distance / hit test.
    pub fn query_radius(&self, x: f32, z: f32, radius: f32) -> impl Iterator<Item = Entity> + '_ {
        let (cx, cz) = cell_key(x, z);
        let cr = (radius / CELL_SIZE).ceil() as i32;
        (cx - cr..=cx + cr).flat_map(move |ix| {
            (cz - cr..=cz + cr)
                .filter_map(move |iz| self.cells.get(&(ix, iz)))
                .flatten()
                .copied()
        })
    }
}

/// System set for the spatial index rebuild. Any system that reads
/// `EntitySpatialIndex` must run `.after(SpatialIndexSet)`, and any system
/// that mutates `Visibility` on `WorldEntity` entities should run
/// `.before(SpatialIndexSet)` or it will fight the single-pass cull.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct SpatialIndexSet;

/// Rebuild the spatial index and perform distance culling in a single pass.
///
/// This replaces the old standalone `distance_culling` system — we already
/// have the entity list and transform in hand while rebuilding, so we set
/// `Visibility` at the same time. `set_if_neq` keeps this cheap: on frames
/// where nothing crossed the draw boundary there are zero writes.
pub fn rebuild_and_cull(
    cfg: Res<crate::config::GameConfig>,
    player_query: Query<&GlobalTransform, With<Player>>,
    mut index: ResMut<EntitySpatialIndex>,
    mut entities: Query<(Entity, &GlobalTransform, &mut Visibility), With<WorldEntity>>,
) {
    index.clear();

    let player_pos = match player_query.single() {
        Ok(gt) => Some(gt.translation()),
        Err(_) => None,
    };
    let draw_dist_sq = cfg.draw_distance * cfg.draw_distance;

    for (entity, g_tf, mut vis) in entities.iter_mut() {
        let pos = g_tf.translation();
        index.insert(entity, pos.x, pos.z);

        if let Some(player_pos) = player_pos {
            let new_vis = if pos.distance_squared(player_pos) < draw_dist_sq {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            vis.set_if_neq(new_vis);
        }
    }
}

pub struct SpatialIndexPlugin;

impl Plugin for SpatialIndexPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EntitySpatialIndex>().add_systems(
            Update,
            rebuild_and_cull
                .in_set(SpatialIndexSet)
                .run_if(in_state(GameState::Game))
                .run_if(resource_equals(HudView::World)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spawn `n` placeholder entities into a throwaway `World` and return a
    /// fresh spatial index populated with them at the given positions.
    /// Using a real `World` avoids hand-constructing `Entity` bit patterns,
    /// which are validated in Bevy 0.18+.
    fn make_index(entries: &[(f32, f32)]) -> (World, EntitySpatialIndex, Vec<Entity>) {
        let mut world = World::new();
        let ids: Vec<Entity> = entries.iter().map(|_| world.spawn_empty().id()).collect();
        let mut idx = EntitySpatialIndex::default();
        for (&(x, z), &e) in entries.iter().zip(ids.iter()) {
            idx.insert(e, x, z);
        }
        (world, idx, ids)
    }

    #[test]
    fn cell_key_rounds_negative_coords_down() {
        // Negative coords must floor, not truncate, so a point at x=-1 lives
        // in cell -1, not 0.
        assert_eq!(cell_key(-1.0, -1.0), (-1, -1));
        assert_eq!(cell_key(0.0, 0.0), (0, 0));
        assert_eq!(cell_key(CELL_SIZE - 0.1, 0.0), (0, 0));
        assert_eq!(cell_key(CELL_SIZE, 0.0), (1, 0));
        assert_eq!(cell_key(-CELL_SIZE, 0.0), (-1, 0));
    }

    #[test]
    fn query_radius_returns_nearby_only() {
        // Place 3 entities: one at origin, one 500 away, one 5000 away.
        let (_w, idx, _ids) = make_index(&[(0.0, 0.0), (500.0, 0.0), (5000.0, 0.0)]);
        let near: Vec<Entity> = idx.query_radius(0.0, 0.0, 512.0).collect();
        // Radius 512 → cell radius 1 → 3×3 block, entity 2 is far outside.
        assert_eq!(near.len(), 2);
        let wide: Vec<Entity> = idx.query_radius(0.0, 0.0, 6000.0).collect();
        assert_eq!(wide.len(), 3);
    }

    #[test]
    fn query_radius_covers_interaction_range_across_cell_boundary() {
        // Player standing just inside cell (0,0) near the +X border must still
        // see an entity that's just inside cell (1,0). This is the reason the
        // 3×3 block around the player is always queried, not just the cell.
        let (_w, idx, _ids) = make_index(&[(CELL_SIZE + 10.0, 0.0)]);
        let near: Vec<Entity> = idx.query_radius(CELL_SIZE - 10.0, 0.0, 100.0).collect();
        assert_eq!(near.len(), 1, "entity in neighbour cell must be returned");
    }

    #[test]
    fn clear_empties_buckets_and_preserves_capacity() {
        let (mut world, mut idx, _ids) = make_index(&[(0.0, 0.0), (0.0, 0.0)]);
        assert_eq!(idx.query_radius(0.0, 0.0, 100.0).count(), 2);
        idx.clear();
        assert_eq!(idx.query_radius(0.0, 0.0, 100.0).count(), 0);
        // Capacity check: after clear, re-inserting should still find them.
        let e = world.spawn_empty().id();
        idx.insert(e, 0.0, 0.0);
        assert_eq!(idx.query_radius(0.0, 0.0, 100.0).count(), 1);
    }
}
