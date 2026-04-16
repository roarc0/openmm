//! Door types, collision, occlusion, and touch trigger data structures.

use bevy::prelude::*;

use openmm_data::blv::DoorState;

use crate::game::interaction::raycast::{point_in_polygon, ray_plane_intersect};

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
            // Panel between feet and eyes -> body intersects it -> blocked
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

/// XZ cell size for the occluder spatial grid. 512 units = 1 MM6 tile.
/// Matches `MAX_INTERACT_RANGE` so a ray of that length crosses at most
/// a 3x3 block of cells.
const OCCLUDER_CELL_SIZE: f32 = 512.0;

/// Resource holding all solid face geometry for ray-occlusion tests.
///
/// Present for both outdoor (BSP model faces) and indoor (BLV wall/floor/ceiling faces)
/// maps. Used by hover and interact systems to gate hits — an NPC or decoration
/// behind a building wall should not be targetable.
///
/// Faces are spatially indexed by XZ cell so `min_hit_t` only tests faces
/// overlapping the ray's swept area instead of brute-forcing all 2000+ faces.
#[derive(Resource, Default)]
pub struct OccluderFaces {
    pub faces: Vec<OccluderFaceInfo>,
    /// XZ grid: cell key -> list of face indices into `faces`.
    grid: bevy::platform::collections::HashMap<(i32, i32), Vec<u32>>,
    /// Per-face generation stamp for dedup without per-frame allocation.
    /// Incremented each `min_hit_t_max` call; a face is "tested" if
    /// `tested_gen[i] == current_gen`.
    tested_gen: Vec<u32>,
    current_gen: u32,
}

impl OccluderFaces {
    /// Create from a face list and build the spatial grid.
    pub fn new(faces: Vec<OccluderFaceInfo>) -> Self {
        let len = faces.len();
        let mut s = Self {
            faces,
            grid: Default::default(),
            tested_gen: vec![0; len],
            current_gen: 1,
        };
        s.build_grid();
        s
    }

    /// Build the spatial grid from the face list. Call once at map load.
    fn build_grid(&mut self) {
        self.grid.clear();
        for (i, face) in self.faces.iter().enumerate() {
            // Compute XZ AABB of this face.
            let (mut min_x, mut max_x, mut min_z, mut max_z) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
            for v in &face.vertices {
                min_x = min_x.min(v.x);
                max_x = max_x.max(v.x);
                min_z = min_z.min(v.z);
                max_z = max_z.max(v.z);
            }
            // Insert into every cell the AABB overlaps.
            let cx0 = (min_x / OCCLUDER_CELL_SIZE).floor() as i32;
            let cx1 = (max_x / OCCLUDER_CELL_SIZE).floor() as i32;
            let cz0 = (min_z / OCCLUDER_CELL_SIZE).floor() as i32;
            let cz1 = (max_z / OCCLUDER_CELL_SIZE).floor() as i32;
            for cx in cx0..=cx1 {
                for cz in cz0..=cz1 {
                    self.grid.entry((cx, cz)).or_default().push(i as u32);
                }
            }
        }
    }

    /// Returns the smallest `t` along `(origin, dir)` that hits any solid face
    /// within `max_t`, or `f32::MAX` if the ray misses.
    ///
    /// Uses the XZ spatial grid to test only faces near the ray path.
    /// Also applies back-face culling (skip faces whose normal faces the same
    /// direction as the ray — the ray hits the back side, which never occludes).
    pub fn min_hit_t_max(&mut self, origin: Vec3, dir: Vec3, max_t: f32) -> f32 {
        // If grid was built, use spatial lookup. Otherwise fall back to brute force.
        if self.grid.is_empty() && !self.faces.is_empty() {
            return self.min_hit_t_brute(origin, dir, max_t);
        }

        // Collect the set of cells the ray passes through within max_t.
        let end = origin + dir * max_t.min(10000.0);
        let cx0 = (origin.x.min(end.x) / OCCLUDER_CELL_SIZE).floor() as i32;
        let cx1 = (origin.x.max(end.x) / OCCLUDER_CELL_SIZE).floor() as i32;
        let cz0 = (origin.z.min(end.z) / OCCLUDER_CELL_SIZE).floor() as i32;
        let cz1 = (origin.z.max(end.z) / OCCLUDER_CELL_SIZE).floor() as i32;

        let mut min_t = max_t;
        // Bump generation counter for dedup — no per-frame allocation needed.
        self.current_gen = self.current_gen.wrapping_add(1);
        if self.current_gen == 0 {
            // Wraparound: reset all stamps so 0 != current_gen.
            self.tested_gen.fill(0);
            self.current_gen = 1;
        }
        let cur_gen = self.current_gen;

        for cx in cx0..=cx1 {
            for cz in cz0..=cz1 {
                let Some(indices) = self.grid.get(&(cx, cz)) else {
                    continue;
                };
                for &fi in indices {
                    let fi = fi as usize;
                    if self.tested_gen[fi] == cur_gen {
                        continue;
                    }
                    self.tested_gen[fi] = cur_gen;

                    let face = &self.faces[fi];
                    // Back-face cull: if the ray is going the same direction as the
                    // face normal, we're hitting the back side — skip.
                    if face.normal.dot(dir) >= 0.0 {
                        continue;
                    }
                    if let Some(t) = ray_plane_intersect(origin, dir, face.normal, face.plane_dist)
                        && t > 0.0
                        && t < min_t
                    {
                        let hit = origin + dir * t;
                        if point_in_polygon(hit, &face.vertices, face.normal) {
                            min_t = t;
                        }
                    }
                }
            }
        }
        min_t
    }

    /// Brute-force fallback when grid isn't built (shouldn't happen in practice).
    fn min_hit_t_brute(&self, origin: Vec3, dir: Vec3, max_t: f32) -> f32 {
        let mut min_t = max_t;
        for face in &self.faces {
            if face.normal.dot(dir) >= 0.0 {
                continue;
            }
            if let Some(t) = ray_plane_intersect(origin, dir, face.normal, face.plane_dist)
                && t > 0.0
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
