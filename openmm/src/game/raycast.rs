use bevy::prelude::*;

/// Ray-plane intersection. Returns distance `t` along ray if hit (positive = in front).
pub fn ray_plane_intersect(origin: Vec3, dir: Vec3, normal: Vec3, plane_dist: f32) -> Option<f32> {
    let denom = normal.dot(dir);
    if denom.abs() < 1e-6 {
        return None;
    }
    let t = (plane_dist - normal.dot(origin)) / denom;
    if t > 0.0 { Some(t) } else { None }
}

/// Test if a 3D point lies inside a convex/concave polygon using winding number.
/// All points assumed coplanar. Projects to the best 2D plane based on normal.
pub fn point_in_polygon(point: Vec3, vertices: &[Vec3], normal: Vec3) -> bool {
    if vertices.len() < 3 {
        return false;
    }
    let abs_n = normal.abs();
    let (ax1, ax2) = if abs_n.x >= abs_n.y && abs_n.x >= abs_n.z {
        (1usize, 2usize)
    } else if abs_n.y >= abs_n.z {
        (0, 2)
    } else {
        (0, 1)
    };
    let get = |v: Vec3, axis: usize| -> f32 {
        match axis { 0 => v.x, 1 => v.y, _ => v.z }
    };
    let px = get(point, ax1);
    let py = get(point, ax2);
    let mut winding = 0i32;
    let n = vertices.len();
    for i in 0..n {
        let v1 = vertices[i];
        let v2 = vertices[(i + 1) % n];
        let y1 = get(v1, ax2);
        let y2 = get(v2, ax2);
        if y1 <= py {
            if y2 > py {
                let x1 = get(v1, ax1);
                let x2 = get(v2, ax1);
                if (x2 - x1) * (py - y1) - (px - x1) * (y2 - y1) > 0.0 {
                    winding += 1;
                }
            }
        } else if y2 <= py {
            let x1 = get(v1, ax1);
            let x2 = get(v2, ax1);
            if (x2 - x1) * (py - y1) - (px - x1) * (y2 - y1) < 0.0 {
                winding -= 1;
            }
        }
    }
    winding != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ray_plane_hit() {
        let t = ray_plane_intersect(Vec3::new(0.0, 5.0, 0.0), Vec3::NEG_Y, Vec3::Y, 0.0);
        assert!((t.unwrap() - 5.0).abs() < 1e-4);
    }

    #[test]
    fn ray_plane_parallel_miss() {
        let t = ray_plane_intersect(Vec3::new(0.0, 1.0, 0.0), Vec3::X, Vec3::Y, 0.0);
        assert!(t.is_none());
    }

    #[test]
    fn ray_plane_behind_miss() {
        let t = ray_plane_intersect(Vec3::new(0.0, -1.0, 0.0), Vec3::NEG_Y, Vec3::Y, 0.0);
        assert!(t.is_none());
    }

    #[test]
    fn point_in_square_polygon() {
        let verts = vec![
            Vec3::new(-1.0, 0.0, -1.0),
            Vec3::new( 1.0, 0.0, -1.0),
            Vec3::new( 1.0, 0.0,  1.0),
            Vec3::new(-1.0, 0.0,  1.0),
        ];
        assert!(point_in_polygon(Vec3::new(0.0, 0.0, 0.0), &verts, Vec3::Y));
        assert!(!point_in_polygon(Vec3::new(2.0, 0.0, 0.0), &verts, Vec3::Y));
    }
}
