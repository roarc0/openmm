use bevy::prelude::*;

use lod::odm::{ODM_HEIGHT_SCALE, ODM_SIZE, ODM_TILE_SCALE};

/// Maximum height the player can step up onto a BSP floor.
const MAX_STEP_UP: f32 = 50.0;

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
            v0, v1, v2, normal,
        }
    }

    fn near_xz(&self, x: f32, z: f32, radius: f32) -> bool {
        x + radius > self.min_x - radius
            && x - radius < self.max_x + radius
            && z + radius > self.min_z - radius
            && z - radius < self.max_z + radius
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

        Self { normal, plane_dist, polygon_xz, min_x, max_x, min_z, max_z, min_y, max_y }
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
        if px + radius < self.min_x || px - radius > self.max_x
            || pz + radius < self.min_z || pz - radius > self.max_z
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

/// Collection of BSP model collision geometry.
#[derive(Resource, Default)]
pub struct BuildingColliders {
    pub walls: Vec<CollisionWall>,
    pub floors: Vec<CollisionTriangle>,
}

impl BuildingColliders {
    /// Resolve movement: push the player out of any wall they would penetrate.
    /// Uses face planes with outward normals — if the player is within `radius`
    /// of a face plane on the front side, and within the face's XZ footprint,
    /// push them out along the face normal.
    pub fn resolve_movement(&self, from: Vec3, to: Vec3, radius: f32, eye_height: f32) -> Vec3 {
        let mut result = to;
        let feet_y = from.y - eye_height;

        for _ in 0..3 {
            let prev = result;

            for wall in &self.walls {
                // Height check: skip walls entirely above head or below feet
                if feet_y > wall.max_y || from.y < wall.min_y {
                    continue;
                }
                let wall_height = wall.max_y - wall.min_y;
                if wall.max_y < feet_y + MAX_STEP_UP && wall_height < MAX_STEP_UP {
                    continue;
                }

                // Check if player is within the face's XZ footprint
                if !wall.contains_xz(result.x, result.z, radius) {
                    continue;
                }

                // Signed distance to face plane
                let dist = wall.signed_distance(result);

                // Only push if within radius on the front side (approaching from outside)
                // or already penetrating (negative distance = inside the building)
                if dist < radius && dist > -radius {
                    let push = radius - dist;
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

    /// Sample the best BSP floor height at XZ, only considering floors
    /// the player could actually step onto.
    pub fn floor_height_at(&self, x: f32, z: f32, feet_y: f32) -> Option<f32> {
        let mut best: Option<f32> = None;
        let point = Vec2::new(x, z);

        for floor in &self.floors {
            if !floor.near_xz(x, z, 0.0) {
                continue;
            }
            if floor.min_y > feet_y + MAX_STEP_UP {
                continue;
            }
            let a = Vec2::new(floor.v0.x, floor.v0.z);
            let b = Vec2::new(floor.v1.x, floor.v1.z);
            let c = Vec2::new(floor.v2.x, floor.v2.z);
            if point_in_triangle_2d(point, a, b, c) {
                let (u, v, w) = barycentric_2d(point, a, b, c);
                let h = u * floor.v0.y + v * floor.v1.y + w * floor.v2.y;
                if h <= feet_y + MAX_STEP_UP {
                    best = Some(best.map_or(h, |prev: f32| prev.max(h)));
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
#[derive(Resource)]
pub struct WaterWalking(pub bool);

impl Default for WaterWalking {
    fn default() -> Self {
        Self(false)
    }
}

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
pub fn probe_ground_height(
    height_map: &[u8],
    colliders: Option<&BuildingColliders>,
    x: f32,
    z: f32,
) -> f32 {
    let terrain_h = sample_terrain_height(height_map, x, z);
    let mut best = terrain_h;
    if let Some(colliders) = colliders {
        let point = Vec2::new(x, z);
        for floor in &colliders.floors {
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
        if ((vi.y > p.y) != (vj.y > p.y))
            && (p.x < (vj.x - vi.x) * (p.y - vi.y) / (vj.y - vi.y) + vi.x)
        {
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
