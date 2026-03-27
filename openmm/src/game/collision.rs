use bevy::prelude::*;

use lod::odm::{ODM_HEIGHT_SCALE, ODM_SIZE, ODM_TILE_SCALE};

/// A collision triangle in Bevy coordinates.
#[derive(Clone)]
pub struct CollisionTriangle {
    pub v0: Vec3,
    pub v1: Vec3,
    pub v2: Vec3,
    pub normal: Vec3,
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
        for wall in &self.walls {
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

// --- Geometry helpers ---

fn segment_hits_wall(from: Vec3, to: Vec3, wall: &CollisionTriangle, radius: f32) -> bool {
    // Skip walls entirely above or below the player
    let player_y = from.y;
    let wall_min_y = wall.v0.y.min(wall.v1.y).min(wall.v2.y);
    let wall_max_y = wall.v0.y.max(wall.v1.y).max(wall.v2.y);
    if player_y < wall_min_y || player_y - 300.0 > wall_max_y {
        return false;
    }

    // XZ plane normal of the wall
    let wall_normal_2d = Vec2::new(wall.normal.x, wall.normal.z);
    if wall_normal_2d.length_squared() < 0.001 {
        return false;
    }
    let wall_normal_2d = wall_normal_2d.normalize();

    let wall_point = Vec2::new(wall.v0.x, wall.v0.z);
    let to_2d = Vec2::new(to.x, to.z);
    let from_2d = Vec2::new(from.x, from.z);

    let dist_to = (to_2d - wall_point).dot(wall_normal_2d);
    let dist_from = (from_2d - wall_point).dot(wall_normal_2d);

    // Only block if crossing from front to within radius
    if dist_to > radius || dist_from < 0.0 {
        return false;
    }

    // Find the crossing point on the XZ plane
    let t = if (dist_from - dist_to).abs() > 0.001 {
        (dist_from - radius) / (dist_from - dist_to)
    } else {
        0.5
    };
    let t = t.clamp(0.0, 1.0);
    let hit = from_2d + (to_2d - from_2d) * t;

    let a = Vec2::new(wall.v0.x, wall.v0.z);
    let b = Vec2::new(wall.v1.x, wall.v1.z);
    let c = Vec2::new(wall.v2.x, wall.v2.z);
    point_in_triangle_2d_expanded(hit, a, b, c, radius)
}

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
    let a2 = a + (a - center).normalize_or_zero() * expand;
    let b2 = b + (b - center).normalize_or_zero() * expand;
    let c2 = c + (c - center).normalize_or_zero() * expand;
    point_in_triangle_2d(p, a2, b2, c2)
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
