use bevy::math::Vec3;
use bevy::prelude::*;

/// Info about a single clickable indoor face.
pub struct FaceInfo {
    pub face_index: usize,
    pub event_id: u16,
    pub normal: Vec3,
    pub plane_dist: f32,
    pub vertices: Vec<Vec3>,
}

/// Resource holding all clickable face data for the current map.
#[derive(Resource)]
pub struct Faces {
    pub faces: Vec<FaceInfo>,
    /// True when loaded from a BLV (indoor) map; false for ODM outdoor BSP buildings.
    pub is_indoor: bool,
}
