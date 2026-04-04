//! Parser for 2devents.txt — the master building/house database.
//!
//! Each row defines a "house" (building interior): its type, name, proprietor,
//! background picture ID, opening hours, etc. House IDs are referenced by
//! EVT SpeakInHouse instructions.

use std::collections::HashMap;
use std::error::Error;
use std::io::{Cursor, Read};

use csv::ReaderBuilder;

use crate::LodManager;

/// A building/house entry from 2devents.txt.
///
/// Column layout (0-indexed):
///   0: #, 1: #, 2: Type, 3: Map, 4: Picture, 5: Name, 6: Proprietor, 7: Title,
///   8: OwnerPicture, 9: State, 10: Rep, 11: Per, 12: Val, 13: A, 14: B, 15: C,
///   16: Notes, 17: (blank), 18: Open, 19: Closed, 20: ExitPic, 21: ExitMap,
///   22: Restrictions, 23: Text
///
/// Maps to MMExtension `Events2DItem` (MM6: 0x30 bytes).
#[derive(Debug, Clone)]
pub struct HouseEntry {
    /// Global house ID (1-indexed, row number in 2devents.txt). Col 0.
    pub id: u32,
    /// Building type string (e.g. "Weapon Shop", "Tavern", "Temple"). Col 2.
    pub building_type: String,
    /// Map location code (e.g. "E3", "D2"). Col 3.
    pub map: String,
    /// Background picture ID (MMExtension `Picture`). Col 4.
    pub picture_id: u16,
    /// Building name (e.g. "The Knife Shoppe"). Col 5.
    pub name: String,
    /// Proprietor/owner name (MMExtension `OwnerName`). Col 6.
    pub proprietor: String,
    /// Proprietor title (e.g. "Blacksmith"; MMExtension `OwnerTitle`). Col 7.
    pub title: String,
    /// NPC portrait index for the proprietor (MMExtension `OwnerPicture`). Col 8.
    pub owner_picture: i16,
    /// Building state (MMExtension `State`). Col 9.
    pub state: i16,
    /// Reputation effect when buying/interacting (MMExtension `Rep`). Col 10.
    pub rep: i16,
    /// Personality modifier (MMExtension `Per`). Col 11.
    pub per: i16,
    /// Price multiplier or bonus value (MMExtension `Val`, r4). Col 12.
    pub val: f32,
    /// Shop stock category string A (e.g. "L1 Weap"). Col 13.
    pub shop_a: String,
    /// Shop stock category string B (e.g. "L2 Dagger"). Col 14.
    pub shop_b: String,
    /// Integer field C (MMExtension `C`, i2). Col 15.
    pub c: i16,
    /// Designer/level-editor notes (not used by engine at runtime). Col 16.
    pub notes: String,
    /// Opening hour (0-23; MMExtension `OpenHour`). Col 18.
    pub open_hour: i16,
    /// Closing hour (0-23; MMExtension `CloseHour`). Col 19.
    pub close_hour: i16,
    /// Exit-screen picture index (MMExtension `ExitPic`). Col 20.
    pub exit_pic: i16,
    /// Exit map reference (MMExtension `ExitMap`). Col 21.
    pub exit_map: i16,
    /// Quest-bit restriction — building only accessible if this quest bit is set (MMExtension `QuestBitRestriction`). Col 22.
    pub quest_restriction: i16,
    /// Dialogue/enter text shown on arrival (MMExtension `EnterText`). Col 23.
    pub enter_text: String,
}

/// All house entries from 2devents.txt.
pub struct TwoDEvents {
    pub houses: HashMap<u32, HouseEntry>,
}

impl TwoDEvents {
    /// Parse 2devents.txt from the LOD archive.
    pub fn parse(lod: &LodManager) -> Result<Self, Box<dyn Error>> {
        let raw = lod
            .try_get_bytes("icons/2devents.txt")
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
        // Skip 2 header lines; non-numeric rows (category headers) are filtered below.
        let body: String = text.lines().skip(2).collect::<Vec<_>>().join("\n");
        let mut rdr = ReaderBuilder::new()
            .delimiter(b'\t')
            .has_headers(false)
            .flexible(true)
            .from_reader(Cursor::new(body.as_bytes()));

        let mut houses = HashMap::new();
        for result in rdr.records() {
            let rec = result?;

            let id: u32 = match rec.get(0).unwrap_or("").trim().parse() {
                Ok(v) => v,
                Err(_) => continue, // skip category header rows (#, "2D Events", etc.)
            };
            if id == 0 {
                continue;
            }

            houses.insert(
                id,
                HouseEntry {
                    id,
                    building_type: rec.get(2).unwrap_or("").trim().to_string(),
                    map: rec.get(3).unwrap_or("").trim().to_string(),
                    picture_id: rec.get(4).unwrap_or("0").trim().parse().unwrap_or(0),
                    name: rec.get(5).unwrap_or("").trim().to_string(),
                    proprietor: rec.get(6).unwrap_or("").trim().to_string(),
                    title: rec.get(7).unwrap_or("").trim().to_string(),
                    owner_picture: rec.get(8).unwrap_or("0").trim().parse().unwrap_or(0),
                    state: rec.get(9).unwrap_or("0").trim().parse().unwrap_or(0),
                    rep: rec.get(10).unwrap_or("0").trim().parse().unwrap_or(0),
                    per: rec.get(11).unwrap_or("0").trim().parse().unwrap_or(0),
                    val: rec.get(12).unwrap_or("0").trim().parse().unwrap_or(0.0),
                    shop_a: rec.get(13).unwrap_or("").trim().to_string(),
                    shop_b: rec.get(14).unwrap_or("").trim().to_string(),
                    c: rec.get(15).unwrap_or("0").trim().parse().unwrap_or(0),
                    notes: rec.get(16).unwrap_or("").trim().to_string(),
                    open_hour: rec.get(18).unwrap_or("0").trim().parse().unwrap_or(0),
                    close_hour: rec.get(19).unwrap_or("0").trim().parse().unwrap_or(0),
                    exit_pic: rec.get(20).unwrap_or("0").trim().parse().unwrap_or(0),
                    exit_map: rec.get(21).unwrap_or("0").trim().parse().unwrap_or(0),
                    quest_restriction: rec.get(22).unwrap_or("0").trim().parse().unwrap_or(0),
                    enter_text: rec.get(23).unwrap_or("").trim().to_string(),
                },
            );
        }

        Ok(TwoDEvents { houses })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_lod;

    #[test]
    fn parse_loads_houses() {
        let Some(lod) = test_lod() else {
            return;
        };
        let events = TwoDEvents::parse(&lod).unwrap();
        assert!(!events.houses.is_empty(), "2devents.txt should have house entries");
    }

    #[test]
    fn house_id_1_has_valid_fields() {
        let Some(lod) = test_lod() else {
            return;
        };
        let events = TwoDEvents::parse(&lod).unwrap();
        let house = events.houses.get(&1).expect("house id 1 should exist in MM6");
        assert!(!house.building_type.is_empty(), "building_type should not be empty");
        assert!(!house.name.is_empty(), "house name should not be empty");
        assert_eq!(house.id, 1);
    }

    #[test]
    fn house_ids_match_keys() {
        let Some(lod) = test_lod() else {
            return;
        };
        let events = TwoDEvents::parse(&lod).unwrap();
        // Every entry's id field should match its map key
        for (&key, entry) in &events.houses {
            assert_eq!(key, entry.id, "house map key and id field should match");
        }
    }
}
