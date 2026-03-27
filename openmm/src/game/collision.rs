use bevy::prelude::*;

use lod::odm::{ODM_HEIGHT_SCALE, ODM_SIZE, ODM_TILE_SCALE};

/// Maximum height the player can step up onto a BSP floor.
const MAX_STEP_UP: f32 = 200.0;

/// A collision triangle in Bevy coordinates, with a precomputed AABB for fast rejection.
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

/// Collection of BSP model collision geometry.
#[derive(Resource, Default)]
pub struct BuildingColliders {
    pub walls: Vec<CollisionTriangle>,
    pub floors: Vec<CollisionTriangle>,
}

impl BuildingColliders {
    /// Check if moving from `from` to `to` would cross any wall.
    /// If blocked, returns the push-out position. Otherwise returns None.
    pub fn check_wall_collision(&self, from: Vec3, to: Vec3, radius: f32) -> Option<Vec3> {
        let check_min_x = from.x.min(to.x) - radius;
        let check_max_x = from.x.max(to.x) + radius;
        let check_min_z = from.z.min(to.z) - radius;
        let check_max_z = from.z.max(to.z) + radius;

        let mut pushed = to;
        let mut was_pushed = false;

        for wall in &self.walls {
            // AABB rejection
            if wall.max_x < check_min_x || wall.min_x > check_max_x
                || wall.max_z < check_min_z || wall.min_z > check_max_z
            {
                continue;
            }
            // Height check: wall must overlap player height range
            let feet_y = from.y - 280.0; // approximate feet position below eye
            if feet_y > wall.max_y || from.y < wall.min_y {
                continue;
            }

            if let Some(push) = wall_push_out(&pushed, wall, radius) {
                pushed = push;
                was_pushed = true;
            }
        }

        if was_pushed { Some(pushed) } else { None }
    }

    /// Sample the best BSP floor height at XZ, only considering floors
    /// the player could actually step onto (not roofs above them).
    /// `feet_y` is the player's foot height (eye_y - eye_height).
    pub fn floor_height_at(&self, x: f32, z: f32, feet_y: f32) -> Option<f32> {
        let mut best: Option<f32> = None;
        let point = Vec2::new(x, z);

        for floor in &self.floors {
            if !floor.near_xz(x, z, 0.0) {
                continue;
            }
            // Skip floors that are way above the player (roofs)
            if floor.min_y > feet_y + MAX_STEP_UP {
                continue;
            }
            let a = Vec2::new(floor.v0.x, floor.v0.z);
            let b = Vec2::new(floor.v1.x, floor.v1.z);
            let c = Vec2::new(floor.v2.x, floor.v2.z);
            if point_in_triangle_2d(point, a, b, c) {
                let (u, v, w) = barycentric_2d(point, a, b, c);
                let h = u * floor.v0.y + v * floor.v1.y + w * floor.v2.y;
                // Only consider floors at or below feet + step up height
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
/// `feet_y` is the player's approximate foot position.
pub fn ground_height_at(
    height_map: &[u8],
    colliders: Option<&BuildingColliders>,
    x: f32,
    z: f32,
    feet_y: f32,
) -> f32 {
    let terrain_h = sample_terrain_height(height_map, x, z);
    if let Some(colliders) = colliders {
        if let Some(bsp_h) = colliders.floor_height_at(x, z, feet_y) {
            return terrain_h.max(bsp_h);
        }
    }
    terrain_h
}

// --- Wall collision with push-out ---

/// If the point is within `radius` of the wall plane and inside the triangle,
/// push it out along the wall normal. Returns the pushed position or None.
fn wall_push_out(pos: &Vec3, wall: &CollisionTriangle, radius: f32) -> Option<Vec3> {
    let wall_normal_2d = Vec2::new(wall.normal.x, wall.normal.z);
    let len = wall_normal_2d.length();
    if len < 0.3 {
        return None; // Floor/ceiling normal, not a wall
    }
    let wall_normal_2d = wall_normal_2d / len;

    let wall_point = Vec2::new(wall.v0.x, wall.v0.z);
    let pos_2d = Vec2::new(pos.x, pos.z);

    // Signed distance from wall plane in XZ
    let dist = (pos_2d - wall_point).dot(wall_normal_2d);

    // Only push out if we're within radius (either side)
    if dist.abs() > radius {
        return None;
    }

    // Check if the point is within the triangle's XZ extent
    let a = Vec2::new(wall.v0.x, wall.v0.z);
    let b = Vec2::new(wall.v1.x, wall.v1.z);
    let c = Vec2::new(wall.v2.x, wall.v2.z);
    if !point_in_triangle_2d_expanded(pos_2d, a, b, c, radius) {
        return None;
    }

    // Push out: move the point so it's exactly `radius` away from the wall
    // Push toward the closest side (front or back)
    let push_dir = if dist >= 0.0 { 1.0 } else { -1.0 };
    let push_amount = radius - dist * push_dir;
    let new_pos_2d = pos_2d + wall_normal_2d * push_dir * push_amount;

    Some(Vec3::new(new_pos_2d.x, pos.y, new_pos_2d.y))
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
