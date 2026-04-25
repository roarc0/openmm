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

        // MM6 -> Bevy coordinate conversion:
        // bevy_x = mm6_x, bevy_y = mm6_z (up), bevy_z = -mm6_y (forward)
        let pos = &party.position;
        let spawn_position = Vec3::new(pos[0] as f32, pos[2] as f32, -(pos[1] as f32));

        // MM6 direction: 0-2047, 0=east, 512=north (counterclockwise).
        // Bevy rotation_y: 0 = -Z (north), positive = counterclockwise from above.
        // Convert: bevy_yaw = (mm6_dir * TAU / 2048) - PI/2
        let spawn_yaw = (party.direction as f32) * std::f32::consts::TAU / 2048.0 - std::f32::consts::FRAC_PI_2;

        // Detect current map from LOD directory name_tail matching.
        // MM6 doesn't store the map name explicitly — the DDM/DLV written in
        // the same save cycle as party.bin shares its name_tail bytes.
        let map_stem = save_file.detect_current_map().unwrap_or_else(|| {
            panic!("could not detect current map from save '{}'", path.display());
        });
        let map_name = MapName::try_from(map_stem.as_str()).unwrap_or_else(|e| {
            panic!("invalid map '{}' detected from save: {e}", map_stem);
        });
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
        // Bevy -> MM6: mm6_x = bevy_x, mm6_y = -bevy_z, mm6_z = bevy_y
        self.party.position = [pos.x as i32, -(pos.z as i32), pos.y as i32];

        // Bevy yaw -> MM6 direction (0-2047): mm6_dir = (yaw + PI/2) * 2048 / TAU
        let (_, yaw, _) = transform.rotation.to_euler(EulerRot::YXZ);
        let mm6_dir = ((yaw + std::f32::consts::FRAC_PI_2) * 2048.0 / std::f32::consts::TAU) as i32;
        self.party.direction = mm6_dir.rem_euclid(2048);

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

/// Empty plugin placeholder — no systems needed in Phase 1.
pub struct SavePlugin;

impl Plugin for SavePlugin {
    fn build(&self, _app: &mut App) {}
}
