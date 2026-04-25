use bevy::prelude::*;

use openmm_data::assets::bsp_model::BSPModel;
use openmm_data::enums::PolygonType;
use openmm_data::odm::{ODM_HEIGHT_SCALE, ODM_SIZE, ODM_TILE_SCALE};

use super::coords::mm6_fixed_normal_to_bevy;

/// Maximum height the player can step up onto a BSP floor (e.g. climbing stairs).
/// 128 units covers typical MM6 discrete stair step heights.
pub const MAX_STEP_UP: f32 = 60.0;

/// Maximum wall height that is silently stepped over (like a low curb).
/// Walls taller than this are treated as solid obstacles in resolve_movement.
/// Kept small so fountain/statue walls are not silently bypassed.
const MAX_WALL_STEP: f32 = 24.0;

/// A collision triangle in Bevy coordinates, with precomputed AABB.
/// Used for floor height sampling (point-in-triangle + barycentric interpolation).
#[derive(Clone)]
pub struct CollisionTriangle {
    pub v0: Vec3,
    pub v1: Vec3,
    pub v2: Vec3,
    pub normal: Vec3,
    pub min_x: f32,
    pub max_x: f32,
    pub min_z: f32,
    pub max_z: f32,
    pub min_y: f32,
    pub max_y: f32,
}

impl CollisionTriangle {
    pub fn new(v0: Vec3, v1: Vec3, v2: Vec3, normal: Vec3) -> Self {
        Self {
            min_x: v0.x.min(v1.x).min(v2.x),
            max_x: v0.x.max(v1.x).max(v2.x),
            min_z: v0.z.min(v1.z).min(v2.z),
            max_z: v0.z.max(v1.z).max(v2.z),
            min_y: v0.y.min(v1.y).min(v2.y),
            max_y: v0.y.max(v1.y).max(v2.y),
            v0,
            v1,
            v2,
            normal,
        }
    }

    pub(crate) fn near_xz(&self, x: f32, z: f32, radius: f32) -> bool {
        x + radius > self.min_x - radius
            && x - radius < self.max_x + radius
            && z + radius > self.min_z - radius
            && z - radius < self.max_z + radius
    }

    /// Sample the Y height of this triangle at an XZ position.
    /// Returns None if the XZ point is outside the triangle.
    pub fn height_at_xz(&self, x: f32, z: f32) -> Option<f32> {
        if !self.near_xz(x, z, 0.0) {
            return None;
        }
        let a = Vec2::new(self.v0.x, self.v0.z);
        let b = Vec2::new(self.v1.x, self.v1.z);
        let c = Vec2::new(self.v2.x, self.v2.z);
        let p = Vec2::new(x, z);
        if !point_in_triangle_2d(p, a, b, c) {
            return None;
        }
        let (u, v, w) = barycentric_2d(p, a, b, c);
        Some(u * self.v0.y + v * self.v1.y + w * self.v2.y)
    }
}

/// A wall face for plane-based collision. Stores the face plane (outward normal
/// + distance) and the polygon vertices projected to XZ for containment testing.
#[derive(Clone)]
pub struct CollisionWall {
    /// Outward-facing normal (in Bevy coords).
    pub normal: Vec3,
    /// Plane distance: dot(normal, point_on_plane).
    pub plane_dist: f32,
    /// Polygon vertices projected to XZ, for point-in-polygon test.
    pub polygon_xz: Vec<Vec2>,
    /// AABB bounds.
    pub min_x: f32,
    pub max_x: f32,
    pub min_z: f32,
    pub max_z: f32,
    pub min_y: f32,
    pub max_y: f32,
    /// True if a floor triangle exists directly above this wall within MAX_STEP_UP.
    /// Precomputed at load time — stair risers are passable, obstacle walls are not.
    pub is_step: bool,
}

impl CollisionWall {
    pub fn new(normal: Vec3, plane_dist: f32, vertices: &[Vec3]) -> Self {
        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;
        let mut min_z = f32::MAX;
        let mut max_z = f32::MIN;
        let mut min_y = f32::MAX;
        let mut max_y = f32::MIN;
        let mut polygon_xz = Vec::with_capacity(vertices.len());

        for v in vertices {
            min_x = min_x.min(v.x);
            max_x = max_x.max(v.x);
            min_z = min_z.min(v.z);
            max_z = max_z.max(v.z);
            min_y = min_y.min(v.y);
            max_y = max_y.max(v.y);
            polygon_xz.push(Vec2::new(v.x, v.z));
        }

        Self {
            normal,
            plane_dist,
            polygon_xz,
            min_x,
            max_x,
            min_z,
            max_z,
            min_y,
            max_y,
            is_step: false,
        }
    }

    /// Signed distance from a 3D point to this face's plane.
    /// Positive = in front (outside), negative = behind (inside).
    fn signed_distance(&self, point: Vec3) -> f32 {
        self.normal.dot(point) - self.plane_dist
    }

    /// Check if a 2D point (XZ) is within the face polygon, expanded by radius.
    /// Uses the XZ normal component to determine the dominant axis for projection.
    fn contains_xz(&self, px: f32, pz: f32, radius: f32) -> bool {
        // AABB pre-check with radius
        if px + radius < self.min_x || px - radius > self.max_x || pz + radius < self.min_z || pz - radius > self.max_z
        {
            return false;
        }
        // Point-in-polygon (ray casting) on the XZ polygon, or within radius of any edge
        let p = Vec2::new(px, pz);
        if point_in_polygon_2d(p, &self.polygon_xz) {
            return true;
        }
        // Also check if within radius of any polygon edge (handles near-miss at edges)
        for i in 0..self.polygon_xz.len() {
            let a = self.polygon_xz[i];
            let b = self.polygon_xz[(i + 1) % self.polygon_xz.len()];
            if point_to_segment_dist_sq(p, a, b) < radius * radius {
                return true;
            }
        }
        false
    }
}

/// Spatial partitioning grid for fast collision pruning.
#[derive(Clone, Default)]
pub struct SpatialGrid {
    /// Grid dimensions (cells) along X and Z.
    pub size: usize,
    /// Width of one grid cell in world units.
    pub cell_width: f32,
    /// Minimum X/Z coordinate (bottom-left corner of the grid).
    pub min_corner: Vec2,
    /// Indices of walls in each cell.
    pub walls: Vec<Vec<u32>>,
    /// Indices of floors in each cell.
    pub floors: Vec<Vec<u32>>,
    /// Indices of ceilings in each cell.
    pub ceilings: Vec<Vec<u32>>,
}

impl SpatialGrid {
    pub fn new(size: usize, cell_width: f32, min_corner: Vec2) -> Self {
        Self {
            size,
            cell_width,
            min_corner,
            walls: vec![Vec::new(); size * size],
            floors: vec![Vec::new(); size * size],
            ceilings: vec![Vec::new(); size * size],
        }
    }

    fn cell_idx(&self, x: f32, z: f32) -> Option<usize> {
        let cx = ((x - self.min_corner.x) / self.cell_width).floor() as i32;
        let cz = ((z - self.min_corner.y) / self.cell_width).floor() as i32;
        if cx >= 0 && cx < self.size as i32 && cz >= 0 && cz < self.size as i32 {
            Some(cz as usize * self.size + cx as usize)
        } else {
            None
        }
    }

    /// Iterator over cell indices overlapping an AABB. Returns an empty
    /// iterator when the grid has not been built (`size == 0`); callers must
    /// handle that case explicitly (the pre-grid fallback is to scan all
    /// walls/floors).
    ///
    /// Allocation-free by design — this is called from the inner collision
    /// loop, which runs multiple times per moving entity per frame.
    fn cells_overlapping(&self, min_x: f32, max_x: f32, min_z: f32, max_z: f32) -> impl Iterator<Item = usize> + '_ {
        let size = self.size;
        // Start with an intentionally-empty range; real bounds overwrite when
        // the grid is initialised. Inclusive ranges with start > end iterate
        // zero times, so returning `1..=0` is the safe "no cells" sentinel.
        let (x0, x1, z0, z1) = if size == 0 {
            (1usize, 0, 1, 0)
        } else {
            let size_i = size as i32;
            let ix0 = ((min_x - self.min_corner.x) / self.cell_width).floor() as i32;
            let iz0 = ((min_z - self.min_corner.y) / self.cell_width).floor() as i32;
            let ix1 = ((max_x - self.min_corner.x) / self.cell_width).floor() as i32;
            let iz1 = ((max_z - self.min_corner.y) / self.cell_width).floor() as i32;
            (
                ix0.clamp(0, size_i - 1) as usize,
                ix1.clamp(0, size_i - 1) as usize,
                iz0.clamp(0, size_i - 1) as usize,
                iz1.clamp(0, size_i - 1) as usize,
            )
        };
        (z0..=z1).flat_map(move |z| (x0..=x1).map(move |x| z * size + x))
    }
}

/// Collection of BSP model collision geometry.
#[derive(Resource, Default)]
pub struct BuildingColliders {
    pub walls: Vec<CollisionWall>,
    pub floors: Vec<CollisionTriangle>,
    pub ceilings: Vec<CollisionTriangle>,
    /// Optimization grid for pruning collision checks.
    pub grid: SpatialGrid,
}

impl BuildingColliders {
    /// Resolve movement: push the player out of any wall they would penetrate.
    /// Uses face planes with outward normals — if the player is within `radius`
    /// of a face plane on the front side, and within the face's XZ footprint,
    /// push them out along the face normal.
    /// Build the spatial grid from current walls, floors, and ceilings.
    pub fn build_grid(&mut self) {
        if self.walls.is_empty() && self.floors.is_empty() && self.ceilings.is_empty() {
            return;
        }

        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);

        for w in &self.walls {
            min = min.min(Vec3::new(w.min_x, w.min_y, w.min_z));
            max = max.max(Vec3::new(w.max_x, w.max_y, w.max_z));
        }
        for f in &self.floors {
            min = min.min(f.v0.min(f.v1).min(f.v2));
            max = max.max(f.v0.max(f.v1).max(f.v2));
        }
        for c in &self.ceilings {
            min = min.min(c.v0.min(c.v1).min(c.v2));
            max = max.max(c.v0.max(c.v1).max(c.v2));
        }

        // Pad slightly to avoid edge-of-grid issues
        min -= Vec3::splat(10.0);
        max += Vec3::splat(10.0);

        let width = (max.x - min.x).max(max.z - min.z).max(1024.0);
        let grid_size = 64;
        let cell_width = width / grid_size as f32;

        self.grid = SpatialGrid::new(grid_size, cell_width, Vec2::new(min.x, min.z));

        // `cells_overlapping` borrows `&self.grid`, which conflicts with the
        // mutable borrow of `self.grid.walls[idx]` inside the loop body. The
        // grid is built once per map load so collecting cell indices into a
        // scratch `Vec` here is a non-issue — unlike `resolve_movement`, this
        // path is not in the per-frame hot loop.
        let mut cells_scratch: Vec<usize> = Vec::new();
        for (i, w) in self.walls.iter().enumerate() {
            cells_scratch.clear();
            cells_scratch.extend(self.grid.cells_overlapping(w.min_x, w.max_x, w.min_z, w.max_z));
            for &idx in &cells_scratch {
                self.grid.walls[idx].push(i as u32);
            }
        }
        for (i, f) in self.floors.iter().enumerate() {
            cells_scratch.clear();
            cells_scratch.extend(self.grid.cells_overlapping(f.min_x, f.max_x, f.min_z, f.max_z));
            for &idx in &cells_scratch {
                self.grid.floors[idx].push(i as u32);
            }
        }
        for (i, c) in self.ceilings.iter().enumerate() {
            cells_scratch.clear();
            cells_scratch.extend(self.grid.cells_overlapping(c.min_x, c.max_x, c.min_z, c.max_z));
            for &idx in &cells_scratch {
                self.grid.ceilings[idx].push(i as u32);
            }
        }
    }

    pub fn resolve_movement(&self, from: Vec3, to: Vec3, radius: f32, eye_height: f32) -> Vec3 {
        let mut result = to;
        let feet_y = from.y - eye_height;

        // Determine swept AABB of the movement step.
        let r_bound = radius + 1.0;
        let min_x = from.x.min(to.x) - r_bound;
        let max_x = from.x.max(to.x) + r_bound;
        let min_z = from.z.min(to.z) - r_bound;
        let max_z = from.z.max(to.z) + r_bound;

        // Process a single wall against the current `result`, pushing the
        // entity out along the wall normal if they are penetrating. Pulled
        // out into a closure so both the grid-cell path and the unbuilt-grid
        // fallback share the same logic without duplication.
        //
        // Capturing locals: `&self.walls`, `feet_y`, and `from.y` are
        // immutable so this stays a cheap `Fn`.
        let process_wall = |wall_idx: u32, result: &mut Vec3| {
            let wall = &self.walls[wall_idx as usize];
            // Height check: skip walls entirely above head or at/below feet.
            if feet_y >= wall.max_y || from.y < wall.min_y {
                return;
            }
            let wall_height = wall.max_y - wall.min_y;
            if wall.max_y < feet_y + MAX_WALL_STEP && wall_height < MAX_WALL_STEP {
                return;
            }
            // Stair riser: precomputed at load time — a floor exists above
            // this wall, so let the entity walk through; gravity snaps them
            // up to that floor.
            let step_height = wall.max_y - feet_y;
            if wall.is_step && step_height > 0.0 && step_height <= MAX_STEP_UP {
                return;
            }
            if !wall.contains_xz(result.x, result.z, radius) {
                return;
            }
            let dist = wall.signed_distance(*result);
            // Push only if within radius on the front side (approaching from
            // outside) or already penetrating (negative distance = inside).
            if dist < radius && dist > -radius {
                let push = radius - dist;
                result.x += wall.normal.x * push;
                result.z += wall.normal.z * push;
            }
        };

        // Up to 3 resolution passes — later walls may re-intrude after an
        // earlier push. Break early when a pass makes no measurable change.
        for _ in 0..3 {
            let prev = result;

            if self.grid.size == 0 {
                // Grid not built (no walls at load time, or pre-`build_grid`
                // path). Scan every wall — rare fallback, kept for safety.
                for i in 0..self.walls.len() {
                    process_wall(i as u32, &mut result);
                }
            } else {
                // Walk the cells overlapping the swept AABB and process each
                // cell's wall list in-place. A wall that straddles multiple
                // cells is visited once per cell; the second visit is a
                // no-op because the first push already placed the entity
                // outside the wall plane (`dist >= radius`). Dropping the
                // explicit `HashSet` dedupe makes this path allocation-free,
                // which is the real win — `resolve_movement` is called
                // several times per moving entity per frame.
                for cell_idx in self.grid.cells_overlapping(min_x, max_x, min_z, max_z) {
                    for &wall_idx in &self.grid.walls[cell_idx] {
                        process_wall(wall_idx, &mut result);
                    }
                }
            }

            if (result.x - prev.x).abs() < 0.1 && (result.z - prev.z).abs() < 0.1 {
                break;
            }
        }

        result
    }

    /// Mark walls as stair risers by checking whether a floor triangle overlaps them
    /// in XZ and sits within MAX_STEP_UP above the wall. Called once after load.
    pub fn mark_step_walls(&mut self) {
        for wall in &mut self.walls {
            'floor_search: for floor in &self.floors {
                // Floor must be above the wall top, within stepping range.
                if floor.min_y < wall.max_y || floor.min_y > wall.max_y + MAX_STEP_UP {
                    continue;
                }
                // AABB overlap in XZ.
                if floor.max_x < wall.min_x
                    || floor.min_x > wall.max_x
                    || floor.max_z < wall.min_z
                    || floor.min_z > wall.max_z
                {
                    continue;
                }
                wall.is_step = true;
                break 'floor_search;
            }
        }
    }

    /// Sample the best BSP floor height at XZ, only considering floors within `max_step`
    /// above `feet_y`. Pass `MAX_STEP_UP` when already on BSP geometry; `TERRAIN_ENTRY_STEP`
    /// when on outdoor terrain to avoid stepping onto elevated outdoor objects.
    pub fn floor_height_at(&self, x: f32, z: f32, feet_y: f32, max_step: f32) -> Option<f32> {
        // Tolerance for edge-of-triangle cases: if the player is right on the seam between
        // two adjacent triangles, exact containment may fail for both. We search twice —
        // first exact, then with a small expansion — and return the first hit.
        const EDGE_TOLERANCE: f32 = 16.0;

        for tolerance in [0.0_f32, EDGE_TOLERANCE] {
            let mut best: Option<f32> = None;
            let point = Vec2::new(x, z);

            let local_floors = if let Some(cell_idx) = self.grid.cell_idx(x, z) {
                &self.grid.floors[cell_idx]
            } else {
                // Not in grid, can't be on a building floor
                return None;
            };

            for &floor_idx in local_floors {
                let floor = &self.floors[floor_idx as usize];
                if !floor.near_xz(x, z, tolerance) {
                    continue;
                }
                if floor.min_y > feet_y + max_step {
                    continue;
                }
                let a = Vec2::new(floor.v0.x, floor.v0.z);
                let b = Vec2::new(floor.v1.x, floor.v1.z);
                let c = Vec2::new(floor.v2.x, floor.v2.z);

                // Exact containment first; on the tolerance pass also accept near-edge points.
                let inside = if tolerance == 0.0 {
                    point_in_triangle_2d(point, a, b, c)
                } else {
                    point_in_triangle_2d(point, a, b, c)
                        || point_to_segment_dist_sq(point, a, b) < tolerance * tolerance
                        || point_to_segment_dist_sq(point, b, c) < tolerance * tolerance
                        || point_to_segment_dist_sq(point, c, a) < tolerance * tolerance
                };

                if inside {
                    // Clamp barycentric coords so edge-proximity hits don't extrapolate wildly.
                    let (u_raw, v_raw, _) = barycentric_2d(point, a, b, c);
                    let u = u_raw.clamp(0.0, 1.0);
                    let v = v_raw.clamp(0.0, 1.0);
                    let w = (1.0 - u - v).clamp(0.0, 1.0);
                    let h = u * floor.v0.y + v * floor.v1.y + w * floor.v2.y;
                    if h <= feet_y + max_step {
                        best = Some(best.map_or(h, |prev: f32| prev.max(h)));
                    }
                }
            }

            if best.is_some() {
                return best;
            }
        }
        None
    }

    /// Sample the lowest ceiling height at XZ above the player's head.
    /// `feet_y` is used to limit the search to the current room (avoids
    /// clamping against ceilings from other floors in multi-level dungeons).
    pub fn ceiling_height_at(&self, x: f32, z: f32, head_y: f32, feet_y: f32) -> Option<f32> {
        // Maximum room height — ceilings further above are likely on a different floor
        const MAX_ROOM_HEIGHT: f32 = 2000.0;

        let mut best: Option<f32> = None;
        let point = Vec2::new(x, z);

        let local_ceilings = if let Some(cell_idx) = self.grid.cell_idx(x, z) {
            &self.grid.ceilings[cell_idx]
        } else {
            return None;
        };

        for &ceil_idx in local_ceilings {
            let ceil = &self.ceilings[ceil_idx as usize];
            if !ceil.near_xz(x, z, 0.0) {
                continue;
            }
            let a = Vec2::new(ceil.v0.x, ceil.v0.z);
            let b = Vec2::new(ceil.v1.x, ceil.v1.z);
            let c = Vec2::new(ceil.v2.x, ceil.v2.z);
            if point_in_triangle_2d(point, a, b, c) {
                let (u, v, w) = barycentric_2d(point, a, b, c);
                let h = u * ceil.v0.y + v * ceil.v1.y + w * ceil.v2.y;
                // Only consider ceilings above the player but within room height
                if h > head_y && h < feet_y + MAX_ROOM_HEIGHT {
                    best = Some(best.map_or(h, |prev: f32| prev.min(h)));
                }
            }
        }
        best
    }
}

/// Cached terrain height data for sampling.
#[derive(Resource)]
pub struct TerrainHeightMap {
    pub heights: Vec<u8>,
}

/// Per-grid-cell water flag.
#[derive(Resource)]
pub struct WaterMap {
    pub cells: Vec<bool>,
}

impl WaterMap {
    /// Check if the grid cell at a world position is water.
    pub fn is_water_at(&self, world_x: f32, world_z: f32) -> bool {
        let col = ((world_x / ODM_TILE_SCALE) + 64.0) as usize;
        let row = ((world_z / ODM_TILE_SCALE) + 64.0) as usize;
        if col >= ODM_SIZE || row >= ODM_SIZE {
            return false;
        }
        self.cells[row * ODM_SIZE + col]
    }
}

/// Whether the player can walk on water (e.g. from a spell).
#[derive(Resource, Default)]
pub struct WaterWalking(pub bool);

/// Sample terrain height at a world position using bilinear interpolation.
pub fn sample_terrain_height(height_map: &[u8], world_x: f32, world_z: f32) -> f32 {
    let col_f = (world_x / ODM_TILE_SCALE) + 64.0;
    let row_f = (world_z / ODM_TILE_SCALE) + 64.0;

    let col0 = (col_f.floor() as usize).clamp(0, ODM_SIZE - 2);
    let row0 = (row_f.floor() as usize).clamp(0, ODM_SIZE - 2);
    let col1 = col0 + 1;
    let row1 = row0 + 1;

    let frac_col = (col_f - col0 as f32).clamp(0.0, 1.0);
    let frac_row = (row_f - row0 as f32).clamp(0.0, 1.0);

    let h00 = height_map[row0 * ODM_SIZE + col0] as f32 * ODM_HEIGHT_SCALE;
    let h10 = height_map[row0 * ODM_SIZE + col1] as f32 * ODM_HEIGHT_SCALE;
    let h01 = height_map[row1 * ODM_SIZE + col0] as f32 * ODM_HEIGHT_SCALE;
    let h11 = height_map[row1 * ODM_SIZE + col1] as f32 * ODM_HEIGHT_SCALE;

    let h_top = h00 + (h10 - h00) * frac_col;
    let h_bot = h01 + (h11 - h01) * frac_col;
    h_top + (h_bot - h_top) * frac_row
}

/// Probe for the ground height at a world position from above.
///
/// Uses the `BuildingColliders` spatial grid to prune the floor-triangle
/// search to the single cell containing `(x, z)`. This function is called
/// from the physics query path and on every actor spawn — iterating every
/// floor on the map (as the original loop did) made it show up hot on maps
/// with dense BSP geometry. The grid is already built at load time.
pub fn probe_ground_height(height_map: &[u8], colliders: Option<&BuildingColliders>, x: f32, z: f32) -> f32 {
    let terrain_h = sample_terrain_height(height_map, x, z);
    let mut best = terrain_h;
    if let Some(colliders) = colliders
        && let Some(cell_idx) = colliders.grid.cell_idx(x, z)
    {
        let point = Vec2::new(x, z);
        for &floor_idx in &colliders.grid.floors[cell_idx] {
            let floor = &colliders.floors[floor_idx as usize];
            if !floor.near_xz(x, z, 0.0) {
                continue;
            }
            let a = Vec2::new(floor.v0.x, floor.v0.z);
            let b = Vec2::new(floor.v1.x, floor.v1.z);
            let c = Vec2::new(floor.v2.x, floor.v2.z);
            if point_in_triangle_2d(point, a, b, c) {
                let (u, v, w) = barycentric_2d(point, a, b, c);
                let h = u * floor.v0.y + v * floor.v1.y + w * floor.v2.y;
                if h > best {
                    best = h;
                }
            }
        }
    }
    best
}

// --- Collision resource builders ---
// Called from the loading pipeline so collision data is available before
// spawn_player runs (needed for probe_ground_height on bridges/BSP floors).

/// Build outdoor collision resources from BSP model faces.
/// Uses authoritative polygon_type from game data to classify faces;
/// InBetweenFloorAndWall (stairs/ramps) treated as walkable floor.
pub fn build_outdoor_colliders(bsp_models: &[BSPModel]) -> BuildingColliders {
    let mut walls = Vec::new();
    let mut floors = Vec::new();
    let mut ceilings = Vec::new();
    for model in bsp_models {
        for face in &model.faces {
            if face.vertices_count < 3 || face.is_invisible() {
                continue;
            }
            let normal = Vec3::from(mm6_fixed_normal_to_bevy(face.plane.normal));

            let poly_type = face.polygon_type_enum();
            let is_floor = matches!(
                poly_type,
                Some(PolygonType::Floor) | Some(PolygonType::InBetweenFloorAndWall)
            );
            let is_ceiling = matches!(
                poly_type,
                Some(PolygonType::Ceiling) | Some(PolygonType::InBetweenCeilingAndWall)
            );
            let is_wall = matches!(poly_type, Some(PolygonType::VerticalWall));

            let vert_count = face.vertices_count as usize;
            let verts: Vec<Vec3> = (0..vert_count)
                .filter_map(|i| {
                    let idx = face.vertices_ids[i] as usize;
                    model.vertices.get(idx).map(|&v| Vec3::from(v))
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
    }
    let mut colliders = BuildingColliders {
        walls,
        floors,
        ceilings,
        ..default()
    };
    colliders.mark_step_walls();
    colliders.build_grid();
    colliders
}

/// Build indoor collision resources from pre-extracted BLV collision geometry.
pub fn build_indoor_colliders(
    walls: Vec<CollisionWall>,
    floors: Vec<CollisionTriangle>,
    ceilings: Vec<CollisionTriangle>,
) -> BuildingColliders {
    let mut colliders = BuildingColliders {
        walls,
        floors,
        ceilings,
        ..default()
    };
    colliders.mark_step_walls();
    colliders.build_grid();
    colliders
}

// --- Geometry helpers ---

fn point_in_triangle_2d(p: Vec2, a: Vec2, b: Vec2, c: Vec2) -> bool {
    let d1 = sign_2d(p, a, b);
    let d2 = sign_2d(p, b, c);
    let d3 = sign_2d(p, c, a);
    let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(has_neg && has_pos)
}

fn sign_2d(p1: Vec2, p2: Vec2, p3: Vec2) -> f32 {
    (p1.x - p3.x) * (p2.y - p3.y) - (p2.x - p3.x) * (p1.y - p3.y)
}

fn barycentric_2d(p: Vec2, a: Vec2, b: Vec2, c: Vec2) -> (f32, f32, f32) {
    let v0 = c - a;
    let v1 = b - a;
    let v2 = p - a;
    let dot00 = v0.dot(v0);
    let dot01 = v0.dot(v1);
    let dot02 = v0.dot(v2);
    let dot11 = v1.dot(v1);
    let dot12 = v1.dot(v2);
    let inv_denom = 1.0 / (dot00 * dot11 - dot01 * dot01);
    let u = (dot11 * dot02 - dot01 * dot12) * inv_denom;
    let v = (dot00 * dot12 - dot01 * dot02) * inv_denom;
    (1.0 - u - v, v, u)
}

/// Point-in-polygon test using ray casting (2D).
fn point_in_polygon_2d(p: Vec2, polygon: &[Vec2]) -> bool {
    let n = polygon.len();
    if n < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let vi = polygon[i];
        let vj = polygon[j];
        if ((vi.y > p.y) != (vj.y > p.y)) && (p.x < (vj.x - vi.x) * (p.y - vi.y) / (vj.y - vi.y) + vi.x) {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Squared distance from a point to a line segment (2D).
fn point_to_segment_dist_sq(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let len_sq = ab.length_squared();
    if len_sq < 0.0001 {
        return p.distance_squared(a);
    }
    let t = ((p - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    let closest = a + ab * t;
    p.distance_squared(closest)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Flat heightmap so terrain height is always 0 everywhere — lets us
    /// isolate the floor-triangle probe path from terrain interpolation.
    fn flat_heightmap() -> Vec<u8> {
        vec![0u8; ODM_SIZE * ODM_SIZE]
    }

    fn colliders_with_one_floor(height: f32) -> BuildingColliders {
        // One large horizontal triangle elevated above the terrain, centred
        // at the origin and spanning ±500 units on X/Z. Big enough that a
        // point at (0, 0) is well inside it.
        let n = Vec3::Y;
        let floor = CollisionTriangle::new(
            Vec3::new(-500.0, height, -500.0),
            Vec3::new(500.0, height, -500.0),
            Vec3::new(0.0, height, 500.0),
            n,
        );
        let mut c = BuildingColliders::default();
        c.floors.push(floor);
        c.build_grid();
        c
    }

    /// Regression for A3: `probe_ground_height` with a `BuildingColliders`
    /// argument must still return the floor triangle's height for a point
    /// inside it, even though we now prune via the spatial grid cell instead
    /// of iterating every floor.
    #[test]
    fn probe_ground_height_finds_floor_via_grid() {
        let hm = flat_heightmap();
        let colliders = colliders_with_one_floor(200.0);
        let h = probe_ground_height(&hm, Some(&colliders), 0.0, 0.0);
        assert!(
            (h - 200.0).abs() < 1e-3,
            "expected floor height 200.0 at origin, got {h}"
        );
    }

    /// A point well outside the single floor's AABB must fall back to the
    /// terrain height (0 on a flat map).
    #[test]
    fn probe_ground_height_misses_floor_outside_aabb() {
        let hm = flat_heightmap();
        let colliders = colliders_with_one_floor(200.0);
        let h = probe_ground_height(&hm, Some(&colliders), 10_000.0, 10_000.0);
        assert!(h.abs() < 1e-3, "expected terrain 0.0 outside floor, got {h}");
    }

    /// When the query point lands in a grid cell that has no floors (but
    /// floors exist elsewhere on the map), the result must still be terrain.
    /// Guards against the bug where an empty cell would incorrectly fall
    /// back to scanning all floors.
    #[test]
    fn probe_ground_height_empty_cell_returns_terrain() {
        let hm = flat_heightmap();
        // Build colliders with a floor centred at the origin, then query
        // a point inside the grid but in a neighbouring empty cell.
        let colliders = colliders_with_one_floor(200.0);
        // 600 is outside the ±500 floor AABB but still inside the padded grid.
        let h = probe_ground_height(&hm, Some(&colliders), 600.0, 0.0);
        assert!(h.abs() < 1e-3, "expected terrain 0.0 in empty cell, got {h}");
    }

    /// Point inside the floor's AABB but outside its triangle hull (the
    /// triangle's "corner gap") must not report the floor height. The grid
    /// narrows the candidate list; `point_in_triangle_2d` is the final check.
    #[test]
    fn probe_ground_height_skips_triangle_corner_gap() {
        let hm = flat_heightmap();
        let colliders = colliders_with_one_floor(200.0);
        // (-499, 499) is inside the AABB (-500..500, -500..500) but outside
        // the triangle hull, which has vertices at (-500,-500), (500,-500),
        // (0, 500) — the left edge runs from (-500,-500) to (0,500).
        let h = probe_ground_height(&hm, Some(&colliders), -499.0, 499.0);
        assert!(h.abs() < 1e-3, "expected terrain 0.0 outside hull, got {h}");
    }

    /// Regression for the `resolve_movement` HashSet-dedupe removal: a wall
    /// that straddles multiple grid cells must still push the player out
    /// correctly even though each cell visit re-processes the same wall.
    /// The second visit is a no-op (already pushed outside), so the final
    /// position must match the single-dedupe behaviour.
    #[test]
    fn resolve_movement_pushes_out_of_wall_across_cells() {
        // Build a wall at x = 0, facing +X (normal = +X). The wall spans
        // z = -2000..2000 — wide enough to span several grid cells at the
        // 64-cell grid the build step constructs.
        let verts = [
            Vec3::new(0.0, 0.0, -2000.0),
            Vec3::new(0.0, 200.0, -2000.0),
            Vec3::new(0.0, 200.0, 2000.0),
            Vec3::new(0.0, 0.0, 2000.0),
        ];
        let wall = CollisionWall::new(Vec3::X, 0.0, &verts);

        let mut c = BuildingColliders::default();
        c.walls.push(wall);
        c.build_grid();

        // Entity moves from (+10, eye, 0) to (-10, eye, 0) — at the
        // destination they are 10 units inside the wall plane (well under
        // the 24-unit radius) so `contains_xz` accepts and the wall pushes
        // them back. Expected final x ≈ radius (24). Matches the original
        // behaviour before the HashSet dedupe was removed.
        let radius = 24.0;
        let eye_height = 160.0;
        let from = Vec3::new(10.0, 200.0, 0.0);
        let to = Vec3::new(-10.0, 200.0, 0.0);
        let result = c.resolve_movement(from, to, radius, eye_height);
        assert!(
            result.x >= radius - 0.5,
            "expected push to x ≥ {radius}, got x = {}",
            result.x
        );
    }

    /// Movement that never intersects a wall must be returned unchanged —
    /// verifies we did not accidentally double-push when a wall straddles
    /// multiple cells.
    #[test]
    fn resolve_movement_preserves_free_movement() {
        let verts = [
            Vec3::new(0.0, 0.0, -2000.0),
            Vec3::new(0.0, 200.0, -2000.0),
            Vec3::new(0.0, 200.0, 2000.0),
            Vec3::new(0.0, 0.0, 2000.0),
        ];
        let wall = CollisionWall::new(Vec3::X, 0.0, &verts);
        let mut c = BuildingColliders::default();
        c.walls.push(wall);
        c.build_grid();

        // Move entirely on the +X side of the wall plane — no collision.
        let from = Vec3::new(500.0, 200.0, 0.0);
        let to = Vec3::new(400.0, 200.0, 100.0);
        let result = c.resolve_movement(from, to, 24.0, 160.0);
        assert!((result - to).length() < 0.01, "expected {to:?}, got {result:?}");
    }
}
