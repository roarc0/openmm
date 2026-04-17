//! MM6 → Bevy coordinate conversions.
//!
//! MM6 uses a right-handed system with X right, Y forward, Z up.
//! Bevy uses a right-handed system with X right, Y up, -Z forward.
//! The conversion is `(x, y, z) -> (x, z, -y)`.
//!
//! These helpers live in the engine crate (not `openmm-data`) so the
//! data crate stays free of engine-specific knowledge.

/// Convert an MM6 integer position to a Bevy float position.
///
/// MM6 stores world positions as `i32` in `(X right, Y forward, Z up)`.
/// Bevy uses `f32` in `(X right, Y up, -Z forward)`. This performs the
/// axis swap with no scaling — units are preserved 1:1.
pub fn mm6_position_to_bevy(x: i32, y: i32, z: i32) -> [f32; 3] {
    [x as f32, z as f32, -(y as f32)]
}

/// Convert an MM6 fixed-point 16.16 plane normal `[nx, ny, nz]` (raw `i32`s
/// from BSP/BLV face data) into a Bevy-axis float normal.
///
/// Performs the same axis swap as [`mm6_position_to_bevy`] plus the
/// `/65536` fixed-point unscale that MM6 needs to recover the unit vector.
pub fn mm6_fixed_normal_to_bevy(normal: [i32; 3]) -> [f32; 3] {
    const FIXED_ONE: f32 = 65536.0;
    [
        normal[0] as f32 / FIXED_ONE,
        normal[2] as f32 / FIXED_ONE,
        -(normal[1] as f32) / FIXED_ONE,
    ]
}

/// Convert an MM6 "binary radian" angle to f32 radians.
///
/// MM6 stores angles as 16-bit integers covering a full turn (`0..=65535` →
/// `0..2π`). The argument is `i32` because EVT data uses 4-byte fields even
/// though only the low 16 bits are meaningful. Used for billboard facing
/// yaws, `MoveToMap` directions, and decoration orientations.
pub fn mm6_binary_angle_to_radians(angle: i32) -> f32 {
    (angle as f32) * std::f32::consts::TAU / 65536.0
}
