//! Prepared-world data structures shared by the outdoor and indoor loading pipelines.

use bevy::prelude::*;

/// Detect self-illuminated MM6 textures by well-known name patterns so they
/// visibly glow in the dark (lava pools, torch flames, magic runes, etc.).
/// Returned values are in linear HDR space — values > 1.0 produce bloom.
pub fn texture_emissive(name: &str) -> LinearRgba {
    let n = name.to_ascii_lowercase();
    if n.contains("lava") {
        LinearRgba::rgb(3.0, 0.9, 0.15)
    } else if n.contains("fire") || n.contains("flam") {
        LinearRgba::rgb(2.5, 1.2, 0.3)
    } else if n.contains("magic") {
        LinearRgba::rgb(0.3, 0.5, 1.6)
    } else if n.contains("lite") {
        LinearRgba::rgb(1.2, 1.15, 0.9)
    } else {
        LinearRgba::BLACK
    }
}

/// Resource for indoor (BLV) maps — the indoor equivalent of PreparedWorld.
#[derive(Resource)]
pub struct PreparedIndoorWorld {
    pub models: Vec<PreparedModel>,
    pub start_points: Vec<StartPoint>,
    /// Wall collision geometry extracted from BLV faces.
    pub collision_walls: Vec<crate::game::collision::CollisionWall>,
    /// Floor collision geometry extracted from BLV faces.
    pub collision_floors: Vec<crate::game::collision::CollisionTriangle>,
    /// Ceiling collision geometry extracted from BLV faces.
    pub collision_ceilings: Vec<crate::game::collision::CollisionTriangle>,
    /// Door definitions from DLV.
    pub doors: Vec<openmm_data::blv::BlvDoor>,
    /// Individual door face meshes for animation.
    pub door_face_meshes: Vec<PreparedDoorFace>,
    /// Collision geometry for ALL door faces (including invisible), for DoorColliders.
    pub door_collision_geometry: Vec<PreparedDoorCollision>,
    /// Clickable face data for indoor interaction.
    pub clickable_faces: Vec<ClickableFaceData>,
    /// Touch-triggered faces (EVENT_BY_TOUCH) for proximity events.
    pub touch_trigger_faces: Vec<TouchTriggerFaceData>,
    /// All solid faces (wall/floor/ceiling, non-portal, non-invisible) for ray occlusion.
    pub occluder_faces: Vec<OccluderFaceData>,
    /// Map base name for EVT loading (e.g. "d01").
    pub map_base: String,
    /// Resolved decorations from BLV decoration list.
    pub decorations: openmm_data::assets::Decorations,
    /// Resolved monsters from BLV spawn_points → mapstats (same pipeline as ODM).
    pub resolved_actors: Option<openmm_data::assets::Monsters>,
    /// Static point lights from the BLV file (position in Bevy coords, brightness 0–65535).
    /// These are the designer-placed lights that illuminate campfires, cauldrons, etc.
    pub blv_lights: Vec<(Vec3, u16)>,
    /// Per-sector ambient data: (bbox_min, bbox_max in Bevy coords, min_ambient_light 0–255).
    /// Used to set global ambient based on which sector the player currently occupies.
    pub sector_ambients: Vec<SectorAmbient>,
}

/// Bounding box + minimum ambient light for one BLV sector.
/// `min_ambient` is the original MM6 value (0–255); scale to Bevy lux in the lighting system.
#[derive(Clone)]
pub struct SectorAmbient {
    pub bbox_min: Vec3,
    pub bbox_max: Vec3,
    /// 0–255 ambient floor. 0 = pitch black, 255 = fully lit.
    pub min_ambient: u8,
}

/// Collision-only geometry for a single door face, including invisible blocking surfaces.
/// This covers faces excluded from `door_face_meshes` (e.g. empty-texture invisible blockers)
/// that still need to block player movement.
pub struct PreparedDoorCollision {
    pub door_index: usize,
    /// Triangulated vertex positions in Bevy coords at door distance=0 (open/retracted state).
    pub base_positions: Vec<Vec3>,
    /// Face normal in Bevy coords (from BLV face.normal_f32(), without rendering sign flip).
    pub normal: Vec3,
    /// Per triangle-vertex: whether it moves with the door.
    pub is_moving: Vec<bool>,
}

/// A prepared door face mesh ready for spawning.
pub struct PreparedDoorFace {
    pub face_index: usize,
    pub door_index: usize,
    pub mesh: Mesh,
    pub material: StandardMaterial,
    pub texture: Option<Image>,
    /// Per triangle-vertex: whether it moves with the door.
    pub is_moving_vertex: Vec<bool>,
    /// Base vertex positions (Bevy coords) at door distance=0 (open/retracted state).
    pub base_positions: Vec<[f32; 3]>,
    /// UV change per unit of door displacement for moving vertices.
    pub uv_rate: [f32; 2],
    /// Base UV values per triangle vertex (at distance=0).
    pub base_uvs: Vec<[f32; 2]>,
    /// Whether this face has the MOVES_BY_DOOR flag (needs UV scrolling).
    pub moves_by_door: bool,
}

/// Data for a clickable indoor face.
pub struct ClickableFaceData {
    pub face_index: usize,
    pub event_id: u16,
    pub normal: Vec3,
    pub plane_dist: f32,
    pub vertices: Vec<Vec3>,
}

/// Data for a solid indoor face used for ray occlusion.
pub struct OccluderFaceData {
    pub normal: Vec3,
    pub plane_dist: f32,
    pub vertices: Vec<Vec3>,
}

/// Data for a touch-triggered indoor face (EVENT_BY_TOUCH flag).
/// These fire events when the player walks near/over them.
pub struct TouchTriggerFaceData {
    pub face_index: usize,
    pub event_id: u16,
    /// Center of the face in Bevy coordinates (for distance check).
    pub center: Vec3,
    /// Trigger radius — half the bounding box diagonal for floor faces.
    pub radius: f32,
}

pub struct PreparedModel {
    /// Sub-meshes, one per unique texture in the BSP model.
    pub sub_meshes: Vec<PreparedSubMesh>,
    /// BSP model name (e.g. "TavFrntW", "ArmoryW", "GenStorE").
    pub name: String,
    /// Model center position in Bevy coordinates.
    pub position: Vec3,
    /// Unique event IDs from this model's faces (cog_trigger_id values > 0).
    pub event_ids: Vec<u16>,
}

pub struct PreparedSubMesh {
    pub mesh: Mesh,
    pub material: StandardMaterial,
    pub texture: Option<Image>,
    /// Texture name used by this sub-mesh.
    pub texture_name: String,
    /// Original face indices (into BSPModel::faces) that contributed to this sub-mesh.
    pub face_indices: Vec<u32>,
}

/// A named start/teleport point extracted from map decorations.
pub struct StartPoint {
    pub name: String,
    pub position: Vec3,
    pub yaw: f32,
}

/// Resource containing everything needed to spawn the world after loading.
#[derive(Resource)]
pub struct PreparedWorld {
    pub map: openmm_data::odm::Odm,
    pub terrain_mesh: Mesh,
    pub terrain_texture: Image,
    pub water_mask: Option<Image>,
    pub water_texture: Option<Image>,
    pub models: Vec<PreparedModel>,
    pub decorations: openmm_data::assets::Decorations,
    pub resolved_actors: Option<openmm_data::assets::Actors>,
    pub resolved_monsters: Option<openmm_data::assets::Monsters>,
    pub start_points: Vec<StartPoint>,
    pub sprite_cache: crate::game::sprites::loading::SpriteCache,
    pub dec_sprite_cache: std::collections::HashMap<
        String,
        (
            Handle<crate::game::sprites::material::SpriteMaterial>,
            Handle<Mesh>,
            f32,
            f32,
            std::sync::Arc<crate::game::sprites::loading::AlphaMask>,
        ),
    >,
    pub water_cells: Vec<bool>,
    pub terrain_lookup: openmm_data::terrain::TerrainLookup,
    /// Music track ID from mapstats.txt (maps to Music/{track}.mp3). 0 = no music.
    pub music_track: u8,
}

impl PreparedWorld {
    /// Get the terrain tileset at a Bevy world position.
    pub fn terrain_at(&self, x: f32, z: f32) -> Option<openmm_data::dtile::Tileset> {
        self.terrain_lookup.tileset_at(&self.map, x, z)
    }
}
