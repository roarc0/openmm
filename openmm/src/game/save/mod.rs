//! Bevy-side save integration: ActiveSave resource, state sync, slot helpers.

pub mod load;
pub mod slots;

use bevy::prelude::*;
use openmm_data::save::file::SaveFile;
use openmm_data::save::header::SaveHeader;
use openmm_data::save::party::SaveParty;
use openmm_data::utils::MapName;
use std::error::Error;
use std::path::PathBuf;

/// MM6 direction range: 0-2047 maps to a full circle (TAU radians).
const MM6_DIRECTION_RANGE: f32 = 2048.0;

/// Convert MM6 position [x, y, z] to Bevy Vec3.
/// MM6: X right, Y forward, Z up. Bevy: X right, Y up, Z = -Y_mm6.
fn mm6_to_bevy_position(mm6: &[i32; 3]) -> Vec3 {
    Vec3::new(mm6[0] as f32, mm6[2] as f32, -(mm6[1] as f32))
}

/// Convert Bevy Vec3 back to MM6 position [x, y, z].
fn bevy_to_mm6_position(pos: Vec3) -> [i32; 3] {
    [pos.x as i32, -(pos.z as i32), pos.y as i32]
}

/// Convert MM6 direction (0=east, 512=north, counterclockwise) to Bevy yaw.
/// Bevy rotation_y: 0 = -Z (north), positive = counterclockwise from above.
fn mm6_to_bevy_yaw(mm6_dir: i32) -> f32 {
    (mm6_dir as f32) * std::f32::consts::TAU / MM6_DIRECTION_RANGE - std::f32::consts::FRAC_PI_2
}

/// Convert Bevy yaw back to MM6 direction (0-2047).
fn bevy_to_mm6_direction(yaw: f32) -> i32 {
    let raw = ((yaw + std::f32::consts::FRAC_PI_2) * MM6_DIRECTION_RANGE / std::f32::consts::TAU) as i32;
    raw.rem_euclid(MM6_DIRECTION_RANGE as i32)
}

/// Live save state loaded from a `.mm6` archive.
/// Holds parsed header + party data and the converted Bevy-space spawn point.
#[derive(Resource)]
pub struct ActiveSave {
    pub path: PathBuf,
    pub header: SaveHeader,
    pub party: SaveParty,
    /// Spawn position in Bevy coordinates (converted from MM6).
    pub spawn_position: Vec3,
    /// Spawn yaw in Bevy radians.
    pub spawn_yaw: f32,
    /// Current map parsed from the header.
    pub map_name: MapName,
}

impl ActiveSave {
    /// Open a `.mm6` save file, parse header + party, convert coords to Bevy space.
    pub fn from_file(path: PathBuf) -> Result<Self, Box<dyn Error>> {
        let save_file = SaveFile::open(&path)?;
        let header = save_file.header();
        let party = save_file.party();

        let spawn_position = mm6_to_bevy_position(&party.position);
        let spawn_yaw = mm6_to_bevy_yaw(party.direction);

        // Detect current map from LOD directory name_tail matching.
        // MM6 doesn't store the map name explicitly — the DDM/DLV written in
        // the same save cycle as party.bin shares its name_tail bytes.
        let map_stem = save_file
            .detect_current_map()
            .ok_or_else(|| format!("could not detect current map from save '{}'", path.display()))?;
        let map_name = MapName::try_from(map_stem.as_str())
            .map_err(|e| format!("invalid map '{}' detected from save: {e}", map_stem))?;
        info!("save map: '{}' -> {:?}", map_stem, &map_name);

        Ok(Self {
            path,
            header,
            party,
            spawn_position,
            spawn_yaw,
            map_name,
        })
    }

    /// Sync spawn position from a Bevy transform back to MM6 coordinates.
    pub fn update_from_transform(&mut self, transform: &Transform) {
        let pos = transform.translation;
        self.party.position = bevy_to_mm6_position(pos);

        let (_, yaw, _) = transform.rotation.to_euler(EulerRot::YXZ);
        self.party.direction = bevy_to_mm6_direction(yaw);

        self.spawn_position = pos;
        self.spawn_yaw = yaw;
    }

    /// Update the map name in both header and local state.
    pub fn update_map(&mut self, map_name: &MapName) {
        self.map_name = map_name.clone();
        let map_str = match map_name {
            MapName::Outdoor(odm) => format!("{}.odm", odm),
            MapName::Indoor(name) => format!("{}.blv", name),
        };
        self.header.map_name = map_str;
    }
}

/// Try to load a save file and transition to the loading state.
/// Returns true on success, false on error (logged).
pub fn try_load_save(commands: &mut Commands, path: PathBuf) -> bool {
    match ActiveSave::from_file(path) {
        Ok(save) => {
            commands.insert_resource(save);
            commands.set_state(crate::GameState::Loading);
            true
        }
        Err(e) => {
            error!("Failed to load save: {e}");
            false
        }
    }
}
