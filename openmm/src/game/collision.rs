use bevy::prelude::*;

use lod::odm::{ODM_HEIGHT_SCALE, ODM_SIZE, ODM_TILE_SCALE};

/// A collision triangle in Bevy coordinates, with a precomputed AABB for fast rejection.
#[derive(Clone)]
pub struct CollisionTriangle {
    pub v0: Vec3,
    pub v1: Vec3,
    pub v2: Vec3,
    pub normal: Vec3,
    // XZ bounding box for fast spatial rejection
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

    /// Fast check: is a point (with radius) anywhere near this triangle's XZ bounds?
    fn near_xz(&self, x: f32, z: f32, radius: f32) -> bool {
        x + radius > self.min_x - radius
            && x - radius < self.max_x + radius
            && z + radius > self.min_z - radius
            && z - radius < self.max_z + radius
    }
}

/// Collection of BSP model collision geometry.
#[derive(Resource, Default)]
pub struct BuildingColliders {
    pub walls: Vec<CollisionTriangle>,
    pub floors: Vec<CollisionTriangle>,
}

impl BuildingColliders {
    /// Check if moving from `from` to `to` would cross any wall.
    pub fn blocked_by_wall(&self, from: Vec3, to: Vec3, radius: f32) -> bool {
        let check_x = from.x.min(to.x);
        let check_z = from.z.min(to.z);
        let check_max_x = from.x.max(to.x);
        let check_max_z = from.z.max(to.z);

        for wall in &self.walls {
            // Quick AABB rejection
            if wall.max_x + radius < check_x - radius
                || wall.min_x - radius > check_max_x + radius
                || wall.max_z + radius < check_z - radius
                || wall.min_z - radius > check_max_z + radius
            {
                continue;
            }
            // Height check: skip walls entirely above or below player
            if from.y < wall.min_y || from.y - 300.0 > wall.max_y {
                continue;
            }
            if segment_hits_wall(from, to, wall, radius) {
                return true;
            }
        }
        false
    }

    /// Sample the highest BSP floor height at a given XZ position.
    pub fn floor_height_at(&self, x: f32, z: f32) -> Option<f32> {
        let mut best: Option<f32> = None;
        let point = Vec2::new(x, z);
        for floor in &self.floors {
            if !floor.near_xz(x, z, 0.0) {
                continue;
            }
            let a = Vec2::new(floor.v0.x, floor.v0.z);
            let b = Vec2::new(floor.v1.x, floor.v1.z);
            let c = Vec2::new(floor.v2.x, floor.v2.z);
            if point_in_triangle_2d(point, a, b, c) {
                let (u, v, w) = barycentric_2d(point, a, b, c);
                let h = u * floor.v0.y + v * floor.v1.y + w * floor.v2.y;
                best = Some(best.map_or(h, |prev: f32| prev.max(h)));
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

/// Get the effective ground height considering both terrain and BSP floors.
pub fn ground_height_at(
    height_map: &[u8],
    colliders: Option<&BuildingColliders>,
    x: f32,
    z: f32,
) -> f32 {
    let terrain_h = sample_terrain_height(height_map, x, z);
    if let Some(colliders) = colliders {
        if let Some(bsp_h) = colliders.floor_height_at(x, z) {
            return terrain_h.max(bsp_h);
        }
    }
    terrain_h
}

// --- Wall collision ---

fn segment_hits_wall(from: Vec3, to: Vec3, wall: &CollisionTriangle, radius: f32) -> bool {
    // Project wall normal to XZ plane
    let wall_normal_2d = Vec2::new(wall.normal.x, wall.normal.z);
    let len = wall_normal_2d.length();
    if len < 0.3 {
        return false; // Nearly horizontal normal = floor/ceiling, not a wall
    }
    let wall_normal_2d = wall_normal_2d / len;

    let wall_point = Vec2::new(wall.v0.x, wall.v0.z);
    let to_2d = Vec2::new(to.x, to.z);
    let from_2d = Vec2::new(from.x, from.z);

    // Signed distance from wall plane
    let dist_to = (to_2d - wall_point).dot(wall_normal_2d);
    let dist_from = (from_2d - wall_point).dot(wall_normal_2d);

    // Block if crossing from front (positive) side to within radius
    // Don't block if already behind the wall or destination is far away
    if dist_to > radius {
        return false;
    }
    if dist_from < -radius {
        return false; // Already well behind the wall
    }

    // Find where the movement crosses the wall+radius plane
    let denom = dist_from - dist_to;
    if denom.abs() < 0.001 {
        // Moving parallel to wall — only block if very close
        if dist_from.abs() > radius {
            return false;
        }
    }

    // Check if the crossing point is actually within the triangle
    let t = if denom.abs() > 0.001 {
        ((dist_from - radius) / denom).clamp(0.0, 1.0)
    } else {
        0.5
    };
    let hit = from_2d + (to_2d - from_2d) * t;

    let a = Vec2::new(wall.v0.x, wall.v0.z);
    let b = Vec2::new(wall.v1.x, wall.v1.z);
    let c = Vec2::new(wall.v2.x, wall.v2.z);
    point_in_triangle_2d_expanded(hit, a, b, c, radius)
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

fn point_in_triangle_2d_expanded(p: Vec2, a: Vec2, b: Vec2, c: Vec2, expand: f32) -> bool {
    let center = (a + b + c) / 3.0;
    let ea = a + (a - center).normalize_or_zero() * expand;
    let eb = b + (b - center).normalize_or_zero() * expand;
    let ec = c + (c - center).normalize_or_zero() * expand;
    point_in_triangle_2d(p, ea, eb, ec)
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
