//! Parser for 2devents.txt — the master building/house database.
//!
//! Each row defines a "house" (building interior): its type, name, proprietor,
//! background picture ID, opening hours, etc. House IDs are referenced by
//! EVT SpeakInHouse instructions.

use std::collections::HashMap;
use std::error::Error;
use std::io::Read;

use crate::LodManager;

/// A building/house entry from 2devents.txt.
#[derive(Debug, Clone)]
pub struct HouseEntry {
    /// Global house ID (1-indexed, row number in 2devents.txt).
    pub id: u32,
    /// Building type string (e.g. "Weapon Shop", "Tavern", "Temple").
    pub building_type: String,
    /// Map location code (e.g. "E3", "D2").
    pub map: String,
    /// Background picture ID.
    pub picture_id: u16,
    /// Building name (e.g. "The Knife Shoppe").
    pub name: String,
    /// Proprietor/owner name.
    pub proprietor: String,
    /// Proprietor title (e.g. "Blacksmith", "Innkeeper").
    pub title: String,
}

/// All house entries from 2devents.txt.
pub struct TwoDEvents {
    pub houses: HashMap<u32, HouseEntry>,
}

impl TwoDEvents {
    /// Parse 2devents.txt from the LOD archive.
    pub fn parse(lod: &LodManager) -> Result<Self, Box<dyn Error>> {
        let raw = lod.try_get_bytes("icons/2devents.txt")
            .or_else(|_| lod.try_get_bytes("new/2devents.txt"))?;

        // Decompress if zlib-compressed (LOD entries may have a header before zlib data)
        let data = if let Some(zlib_pos) = raw.windows(2).position(|w| w[0] == 0x78 && w[1] == 0x9c) {
            let mut decoder = flate2::read::ZlibDecoder::new(&raw[zlib_pos..]);
            let mut decompressed = Vec::new();
            decoder.read_to_end(&mut decompressed)?;
            decompressed
        } else {
            raw.to_vec()
        };

        let text = String::from_utf8_lossy(&data);
        let mut houses = HashMap::new();

        for line in text.lines().skip(2) {
            // Skip header rows
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with("2D") {
                continue;
            }

            // Tab-separated, but some fields may be quoted with commas inside
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() < 8 {
                continue;
            }

            let id: u32 = match cols[0].trim().parse() {
                Ok(v) => v,
                Err(_) => continue,
            };

            let building_type = cols[2].trim().trim_matches('"').to_string();
            let map = cols[3].trim().trim_matches('"').to_string();
            let picture_id: u16 = cols[4].trim().parse().unwrap_or(0);
            let name = cols[5].trim().trim_matches('"').to_string();
            let proprietor = cols.get(6).map(|s| s.trim().trim_matches('"').to_string()).unwrap_or_default();
            let title = cols.get(7).map(|s| s.trim().trim_matches('"').to_string()).unwrap_or_default();

            houses.insert(id, HouseEntry {
                id,
                building_type,
                map,
                picture_id,
                name,
                proprietor,
                title,
            });
        }

        Ok(TwoDEvents { houses })
    }
}
