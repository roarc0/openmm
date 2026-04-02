use super::*;
use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};

#[test]
fn direction_front_when_camera_faces_entity() {
    // Camera is directly in front of entity (same angle as facing)
    let (dir, mirror) = direction_for_angle(0.0, 0.0);
    assert_eq!(dir, 0);
    assert!(!mirror);
}

#[test]
fn direction_back_when_camera_behind_entity() {
    // Camera is directly behind (facing 0, camera at PI)
    let (dir, mirror) = direction_for_angle(0.0, PI);
    assert_eq!(dir, 4);
    assert!(!mirror);
}

#[test]
fn direction_right_side() {
    // Camera is 90° to the right of entity facing
    let (dir, mirror) = direction_for_angle(0.0, -FRAC_PI_2);
    assert_eq!(dir, 2);
    assert!(!mirror);
}

#[test]
fn direction_left_side_mirrors() {
    // Camera is 90° to the left — should mirror direction 2
    let (dir, mirror) = direction_for_angle(0.0, FRAC_PI_2);
    assert_eq!(dir, 2);
    assert!(mirror);
}

#[test]
fn direction_symmetry_octants_5_6_7_mirror() {
    // Octants 5, 6, 7 should mirror 3, 2, 1 respectively
    // Octant 5: relative ≈ 5*PI/4
    let (dir, mirror) = direction_for_angle(0.0, -5.0 * FRAC_PI_4);
    assert_eq!(dir, 3, "octant 5 should use direction 3");
    assert!(mirror, "octant 5 should be mirrored");

    // Octant 7: relative ≈ 7*PI/4
    let (dir, mirror) = direction_for_angle(0.0, -7.0 * FRAC_PI_4);
    assert_eq!(dir, 1, "octant 7 should use direction 1");
    assert!(mirror, "octant 7 should be mirrored");
}

#[test]
fn direction_wraps_around_tau() {
    // Angles that differ by TAU should give the same result
    let (d1, m1) = direction_for_angle(0.5, 1.0);
    let (d2, m2) = direction_for_angle(0.5 + TAU, 1.0);
    assert_eq!(d1, d2);
    assert_eq!(m1, m2);

    let (d3, m3) = direction_for_angle(0.5, 1.0 + TAU);
    assert_eq!(d1, d3);
    assert_eq!(m1, m3);
}

#[test]
fn direction_negative_angles() {
    // Negative facing and camera angles should work correctly
    let (dir, mirror) = direction_for_angle(-PI, -PI);
    assert_eq!(dir, 0, "same direction should always be front");
    assert!(!mirror);
}

#[test]
fn all_eight_octants_covered() {
    // Walk around the full circle in 8 steps and verify we get all expected directions
    let expected = [
        (0, false), // 0: front
        (1, false), // 1: front-right
        (2, false), // 2: right
        (3, false), // 3: back-right
        (4, false), // 4: back
        (3, true),  // 5: back-left (mirror of 3)
        (2, true),  // 6: left (mirror of 2)
        (1, true),  // 7: front-left (mirror of 1)
    ];
    for (i, &(exp_dir, exp_mirror)) in expected.iter().enumerate() {
        let angle = i as f32 * FRAC_PI_4;
        let (dir, mirror) = direction_for_angle(angle, 0.0);
        assert_eq!(
            (dir, mirror),
            (exp_dir, exp_mirror),
            "octant {}: facing={:.2} camera=0.0",
            i,
            angle
        );
    }
}

/// Regression: preloaded cache entries (palette_id=0) must not collide with
/// spawn-time entries that use a specific DSFT palette_id. Before the fix,
/// "gstfly@v2" was cached by preload with palette_id=0 (sprite-header offset path),
/// then reused at spawn time when the correct DSFT palette was 223 — causing the
/// walking animation to display with the wrong palette while standing was correct
/// (it used a different cache key due to min_w/min_h padding).
#[test]
fn cache_key_includes_palette_id_when_variant_and_palette_nonzero() {
    // No palette: key should not include palette suffix
    assert_eq!(cache_key("gstfly", 2, 0, 0, 0), "gstfly@v2");
    // With DSFT palette: key must differ so preloaded (palette=0) and spawn-time entries don't collide
    assert_eq!(cache_key("gstfly", 2, 0, 0, 223), "gstfly@v2p223");
    assert_ne!(
        cache_key("gstfly", 2, 0, 0, 0),
        cache_key("gstfly", 2, 0, 0, 223),
        "palette_id=0 and palette_id=223 must produce distinct cache keys"
    );
    // variant=1 never uses the DSFT palette path, so palette_id is irrelevant
    assert_eq!(cache_key("gstfly", 1, 0, 0, 223), "gstfly");
    assert_eq!(cache_key("gstfly", 1, 0, 0, 0),   "gstfly");
    // Palette must also be distinct when min dimensions are present
    assert_ne!(
        cache_key("gstfly", 2, 64, 128, 0),
        cache_key("gstfly", 2, 64, 128, 223),
    );
}
