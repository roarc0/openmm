//! Parser for MM6 .evt (event script) files.
//!
//! EVT files contain binary-encoded event scripts that are triggered by
//! BSP face interactions (click, step, etc.). Each BSP face's `cog_trigger_id`
//! maps to an event_id in the corresponding map's .evt file.
//!
//! Binary format (per instruction):
//!   byte 0: size_byte — bytes after this byte (total record = size_byte + 1)
//!   bytes 1-2: event_id (u16 LE)
//!   byte 3: step (u8)
//!   byte 4: opcode (u8)
//!   bytes 5+: params (opcode-dependent)

use std::collections::HashMap;
use std::error::Error;
use std::io::Read;

use crate::LodManager;

/// A parsed game event — the simplified result of executing an event script.
#[derive(Debug, Clone)]
pub enum GameEvent {
    /// Open a building UI. house_id indexes into 2devents.txt.
    SpeakInHouse { house_id: u32 },
    /// Move to another map (dungeon entrance, map transition).
    MoveToMap {
        x: i32,
        y: i32,
        z: i32,
        direction: i32,
        map_name: String,
    },
    /// Open a chest.
    OpenChest { id: u8 },
    /// Show hint text (tooltip on mouseover). `text` is resolved from the .str table.
    Hint { str_id: u8, text: String },
    /// Change a door's state (open/close/toggle).
    /// door_id indexes into the BLV door array.
    /// action: 0=Open, 1=Close, 2=Toggle.
    ChangeDoorState { door_id: u8, action: u8 },
}

/// Parsed events from a .evt file, keyed by event_id.
pub struct EvtFile {
    /// For each event_id, the list of actions (simplified from raw instructions).
    pub events: HashMap<u16, Vec<GameEvent>>,
}

/// Parse a .str string table: null-separated strings indexed from 0.
fn parse_str_table(lod: &LodManager, map_base: &str) -> Vec<String> {
    let data = lod.get_decompressed(format!("icons/{}.str", map_base))
        .or_else(|_| lod.get_decompressed(format!("games/{}.str", map_base)))
        .or_else(|_| lod.get_decompressed(format!("new/{}.str", map_base)));
    let Ok(data) = data else { return Vec::new() };

    data.split(|&b| b == 0)
        .filter_map(|s| std::str::from_utf8(s).ok())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

impl EvtFile {
    /// Parse an .evt file from raw (possibly compressed) LOD data.
    /// Also loads the corresponding .str file for hint text resolution.
    pub fn parse(lod: &LodManager, map_base: &str) -> Result<Self, Box<dyn Error>> {
        let str_table = parse_str_table(lod, map_base);

        // Try multiple archive locations
        let path = format!("icons/{}.evt", map_base);
        let raw = lod.try_get_bytes(&path)
            .or_else(|_| lod.try_get_bytes(&format!("games/{}.evt", map_base)))
            .or_else(|_| lod.try_get_bytes(&format!("new/{}.evt", map_base)))?;

        // Decompress if zlib-compressed
        let data = if let Some(zlib_pos) = raw.windows(2).position(|w| w[0] == 0x78 && w[1] == 0x9c) {
            let mut decoder = flate2::read::ZlibDecoder::new(&raw[zlib_pos..]);
            let mut decompressed = Vec::new();
            decoder.read_to_end(&mut decompressed)?;
            decompressed
        } else {
            raw.to_vec()
        };

        let mut events: HashMap<u16, Vec<GameEvent>> = HashMap::new();
        let mut pos = 0;

        while pos < data.len() {
            let size_byte = data[pos] as usize;
            let total = size_byte + 1;
            if total < 5 || pos + total > data.len() {
                break;
            }

            let event_id = u16::from_le_bytes([data[pos + 1], data[pos + 2]]);
            // byte 3 = step (not needed for simplified parsing)
            let opcode = data[pos + 4];
            let params = &data[pos + 5..pos + total];

            let action = match opcode {
                0x02 => {
                    // SpeakInHouse
                    if params.len() >= 4 {
                        Some(GameEvent::SpeakInHouse {
                            house_id: u32::from_le_bytes([params[0], params[1], params[2], params[3]]),
                        })
                    } else {
                        None
                    }
                }
                0x04 => {
                    // Hint — resolve text from .str table
                    let str_id = params.first().copied().unwrap_or(0);
                    let text = str_table
                        .get(str_id as usize)
                        .cloned()
                        .unwrap_or_default();
                    Some(GameEvent::Hint { str_id, text })
                }
                0x06 => {
                    // MoveToMap
                    if params.len() >= 26 {
                        let x = i32::from_le_bytes([params[0], params[1], params[2], params[3]]);
                        let y = i32::from_le_bytes([params[4], params[5], params[6], params[7]]);
                        let z = i32::from_le_bytes([params[8], params[9], params[10], params[11]]);
                        let direction = i32::from_le_bytes([params[12], params[13], params[14], params[15]]);
                        let name_start = 26;
                        let name_bytes = &params[name_start..];
                        let end = name_bytes.iter().position(|&b| b == 0).unwrap_or(name_bytes.len());
                        let map_name = String::from_utf8_lossy(&name_bytes[..end]).to_string();
                        Some(GameEvent::MoveToMap { x, y, z, direction, map_name })
                    } else {
                        None
                    }
                }
                0x07 => {
                    // OpenChest
                    Some(GameEvent::OpenChest {
                        id: params.first().copied().unwrap_or(0),
                    })
                }
                0x0F => {
                    // ChangeDoorState: door_id (u8), action (u8)
                    if params.len() >= 2 {
                        Some(GameEvent::ChangeDoorState {
                            door_id: params[0],
                            action: params[1],
                        })
                    } else {
                        None
                    }
                }
                _ => None,
            };

            if let Some(action) = action {
                events.entry(event_id).or_default().push(action);
            }

            pos += total;
        }

        Ok(EvtFile { events })
    }

    /// Get the primary action for an event (first SpeakInHouse or MoveToMap).
    pub fn primary_action(&self, event_id: u16) -> Option<&GameEvent> {
        self.events.get(&event_id)?.iter().find(|a| matches!(a,
            GameEvent::SpeakInHouse { .. } | GameEvent::MoveToMap { .. }
        ))
    }
}
