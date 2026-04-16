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

use crate::Assets;
use crate::assets::enums::{DoorAction, EvtOpcode, EvtVariable};

mod evt_types;
pub use evt_types::{EvtStep, GameEvent};

/// Parsed events from a .evt file, keyed by event_id.
pub struct EvtFile {
    /// For each event_id, the list of steps (step number + action).
    pub events: HashMap<u16, Vec<EvtStep>>,
}

/// Parse a .str string table: null-separated strings indexed from 0.
fn parse_str_table(assets: &Assets, map_base: &str) -> Vec<String> {
    let data = assets
        .get_decompressed(format!("icons/{}.str", map_base))
        .or_else(|_| assets.get_decompressed(format!("games/{}.str", map_base)))
        .or_else(|_| assets.get_decompressed(format!("new/{}.str", map_base)));
    let Ok(data) = data else { return Vec::new() };

    data.split(|&b| b == 0)
        .filter_map(|s| std::str::from_utf8(s).ok())
        .filter(|s: &&str| !s.is_empty())
        .map(|s: &str| s.to_string())
        .collect()
}

// ── Binary helpers ──────────────────────────────────────────────────────

/// Read i32 LE from params at offset.
fn i32_at(params: &[u8], off: usize) -> i32 {
    i32::from_le_bytes([params[off], params[off + 1], params[off + 2], params[off + 3]])
}

/// Read u32 LE from params at offset.
fn u32_at(params: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([params[off], params[off + 1], params[off + 2], params[off + 3]])
}

/// Read first i32 from params, or None if too short.
fn read_i32(params: &[u8]) -> Option<i32> {
    if params.len() >= 4 {
        Some(i32_at(params, 0))
    } else {
        None
    }
}

/// Read a null-terminated string from params.
fn read_string(params: &[u8]) -> String {
    let end = params.iter().position(|&b| b == 0).unwrap_or(params.len());
    String::from_utf8_lossy(&params[..end]).to_string()
}

impl EvtFile {
    /// Parse an .evt file from raw (possibly compressed) LOD data.
    /// Also loads the corresponding .str file for hint text resolution.
    pub fn parse(assets: &Assets, map_base: &str) -> Result<Self, Box<dyn Error>> {
        let str_table = parse_str_table(assets, map_base);

        // Try multiple archive locations
        let path = format!("icons/{}.evt", map_base);
        let raw = assets
            .get_bytes(&path)
            .or_else(|_| assets.get_bytes(format!("games/{}.evt", map_base)))
            .or_else(|_| assets.get_bytes(format!("new/{}.evt", map_base)))?;

        // Decompress if zlib-compressed
        let data = if let Some(zlib_pos) = raw.windows(2).position(|w| w[0] == 0x78 && w[1] == 0x9c) {
            let mut decoder = flate2::read::ZlibDecoder::new(&raw[zlib_pos..]);
            let mut decompressed = Vec::new();
            decoder.read_to_end(&mut decompressed)?;
            decompressed
        } else {
            raw.to_vec()
        };

        let mut events: HashMap<u16, Vec<EvtStep>> = HashMap::new();
        let mut pos = 0;

        while pos < data.len() {
            let size_byte = data[pos] as usize;
            let total = size_byte + 1;
            if total < 5 || pos + total > data.len() {
                break;
            }

            let event_id = u16::from_le_bytes([data[pos + 1], data[pos + 2]]);
            let step = data[pos + 3];
            let opcode = data[pos + 4];
            let params = &data[pos + 5..pos + total];

            let evt_opcode = EvtOpcode::from_u8(opcode);

            let action = match evt_opcode {
                Some(EvtOpcode::Exit) => Some(GameEvent::Exit),
                Some(EvtOpcode::SpeakInHouse) => {
                    read_i32(params).map(|v| GameEvent::SpeakInHouse { house_id: v as u32 })
                }
                Some(EvtOpcode::PlaySound) => read_i32(params).map(|v| GameEvent::PlaySound { sound_id: v as u32 }),
                Some(EvtOpcode::MouseOver) => {
                    let str_id = params.first().copied().unwrap_or(0);
                    let text = str_table.get(str_id as usize).cloned().unwrap_or_default();
                    Some(GameEvent::Hint { str_id, text })
                }
                Some(EvtOpcode::LocationName) => {
                    let str_id = params.first().copied().unwrap_or(0);
                    let text = str_table.get(str_id as usize).cloned().unwrap_or_default();
                    Some(GameEvent::LocationName { str_id, text })
                }
                Some(EvtOpcode::MoveToMap) => {
                    if params.len() >= 26 {
                        let x = i32_at(params, 0);
                        let y = i32_at(params, 4);
                        let z = i32_at(params, 8);
                        let direction = i32_at(params, 12);
                        let name_bytes = &params[26..];
                        let end = name_bytes.iter().position(|&b| b == 0).unwrap_or(name_bytes.len());
                        let map_name = String::from_utf8_lossy(&name_bytes[..end]).to_string();
                        Some(GameEvent::MoveToMap {
                            x,
                            y,
                            z,
                            direction,
                            map_name,
                        })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::OpenChest) => Some(GameEvent::OpenChest {
                    id: params.first().copied().unwrap_or(0),
                }),
                Some(EvtOpcode::ShowMessage) => {
                    let str_id = params.first().copied().unwrap_or(0);
                    let text = str_table.get(str_id as usize).cloned().unwrap_or_default();
                    Some(GameEvent::ShowMessage { str_id, text })
                }
                Some(EvtOpcode::StatusText) => {
                    let str_id = params.first().copied().unwrap_or(0);
                    let text = str_table.get(str_id as usize).cloned().unwrap_or_default();
                    Some(GameEvent::StatusText { str_id, text })
                }
                Some(EvtOpcode::ChangeDoorState) => {
                    if params.len() >= 2 {
                        Some(GameEvent::ChangeDoorState {
                            door_id: params[0],
                            action: DoorAction::from_u8(params[1]).unwrap_or(DoorAction::Toggle),
                        })
                    } else {
                        None
                    }
                }
                // ── Control flow ────────────────────────────────────────
                // MM6: Compare = var_id(u8) + value(i32 LE) + jump_step(u8) = 6 bytes
                Some(EvtOpcode::Compare) => {
                    if params.len() >= 6 {
                        Some(GameEvent::Compare {
                            var: EvtVariable(params[0]),
                            value: i32_at(params, 1),
                            jump_step: params[5],
                        })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::Jmp) => Some(GameEvent::Jmp {
                    target_step: params.first().copied().unwrap_or(0),
                }),
                Some(EvtOpcode::ForPartyMember) => Some(GameEvent::ForPartyMember {
                    player: params.first().copied().unwrap_or(0),
                }),
                // ── Variable operations ─────────────────────────────────
                // MM6: Add/Subtract/Set = var_id(u8) + value(i32 LE) = 5 bytes
                Some(EvtOpcode::Add) => {
                    if params.len() >= 5 {
                        Some(GameEvent::Add {
                            var: EvtVariable(params[0]),
                            value: i32_at(params, 1),
                        })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::Subtract) => {
                    if params.len() >= 5 {
                        Some(GameEvent::Subtract {
                            var: EvtVariable(params[0]),
                            value: i32_at(params, 1),
                        })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::Set) => {
                    if params.len() >= 5 {
                        Some(GameEvent::Set {
                            var: EvtVariable(params[0]),
                            value: i32_at(params, 1),
                        })
                    } else {
                        None
                    }
                }
                // ── NPC / item operations ───────────────────────────────
                // MM6: GiveItem = strength(u8) + type(u8) + id(u32 LE) = 6 bytes
                Some(EvtOpcode::GiveItem) => {
                    if params.len() >= 6 {
                        Some(GameEvent::GiveItem {
                            strength: params[0],
                            item_type: params[1],
                            item_id: u32_at(params, 2),
                        })
                    } else {
                        None
                    }
                }
                // MM6: SetNPCTopic = npc_id(i32) + index(u8) + event_id(i32) = 9 bytes
                Some(EvtOpcode::SetNPCTopic) => {
                    if params.len() >= 9 {
                        Some(GameEvent::SetNPCTopic {
                            npc_id: i32_at(params, 0),
                            topic_index: params[4],
                            event_id: i32_at(params, 5),
                        })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::MoveNPC) => {
                    if params.len() >= 8 {
                        Some(GameEvent::MoveNPC {
                            npc_id: i32_at(params, 0),
                            map_id: i32_at(params, 4),
                        })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::SpeakNPC) => read_i32(params).map(|v| GameEvent::SpeakNPC { npc_id: v }),
                Some(EvtOpcode::ChangeEvent) => {
                    if params.len() >= 8 {
                        Some(GameEvent::ChangeEvent {
                            target: i32_at(params, 0),
                            new_event_id: i32_at(params, 4),
                        })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::SetNPCGreeting) => {
                    if params.len() >= 8 {
                        Some(GameEvent::SetNPCGreeting {
                            npc_id: i32_at(params, 0),
                            greeting_id: i32_at(params, 4),
                        })
                    } else {
                        None
                    }
                }
                // ── World operations ────────────────────────────────────
                // SetFacesBit = face_id(i32) + bit(i32) + on(u8) = 9 bytes
                Some(EvtOpcode::SetFacesBit) => {
                    if params.len() >= 9 {
                        Some(GameEvent::SetFacesBit {
                            face_id: i32_at(params, 0),
                            bit: i32_at(params, 4),
                            on: params[8],
                        })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::ToggleActorFlag) => {
                    if params.len() >= 9 {
                        Some(GameEvent::ToggleActorFlag {
                            actor_id: i32_at(params, 0),
                            flag: i32_at(params, 4),
                            on: params[8],
                        })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::SetTexture) => {
                    if params.len() >= 5 {
                        let face_id = i32_at(params, 0);
                        let name = read_string(&params[4..]);
                        Some(GameEvent::SetTexture {
                            face_id,
                            texture_name: name,
                        })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::SetSprite) => {
                    // Format: cog(u32) + hide(u8) + name(null-terminated string)
                    if params.len() >= 6 {
                        let decoration_id = i32_at(params, 0);
                        let name = read_string(&params[5..]);
                        Some(GameEvent::SetSprite {
                            decoration_id,
                            sprite_name: name,
                        })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::ToggleIndoorLight) => {
                    if params.len() >= 5 {
                        Some(GameEvent::ToggleIndoorLight {
                            light_id: i32_at(params, 0),
                            on: params[4],
                        })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::SetSnow) => Some(GameEvent::SetSnow {
                    on: params.first().copied().unwrap_or(0),
                }),
                Some(EvtOpcode::SummonMonsters) => {
                    if params.len() >= 20 {
                        Some(GameEvent::SummonMonsters {
                            monster_id: i32_at(params, 0),
                            count: i32_at(params, 4),
                            x: i32_at(params, 8),
                            y: i32_at(params, 12),
                            z: i32_at(params, 16),
                        })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::CastSpell) => {
                    if params.len() >= 36 {
                        Some(GameEvent::CastSpell {
                            spell_id: i32_at(params, 0),
                            skill_level: i32_at(params, 4),
                            skill_mastery: i32_at(params, 8),
                            from_x: i32_at(params, 12),
                            from_y: i32_at(params, 16),
                            from_z: i32_at(params, 20),
                            to_x: i32_at(params, 24),
                            to_y: i32_at(params, 28),
                            to_z: i32_at(params, 32),
                        })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::ReceiveDamage) => {
                    if params.len() >= 8 {
                        Some(GameEvent::ReceiveDamage {
                            damage_type: i32_at(params, 0),
                            amount: i32_at(params, 4),
                        })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::ShowFace) => {
                    if params.len() >= 5 {
                        Some(GameEvent::ShowFace {
                            player: params[0],
                            expression: i32_at(params, 1),
                        })
                    } else {
                        None
                    }
                }
                // ── Timer / conditional ─────────────────────────────────
                Some(EvtOpcode::OnTimer) => {
                    if params.len() >= 8 {
                        Some(GameEvent::OnTimer {
                            year: u16::from_le_bytes([params[0], params[1]]),
                            month: params[2],
                            week: params[3],
                            day: u16::from_le_bytes([params[4], params[5]]),
                            hour: params[6],
                            minute: params[7],
                        })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::OnLongTimer) => Some(GameEvent::OnLongTimer {
                    timer_data: params.to_vec(),
                }),
                Some(EvtOpcode::OnMapReload) => Some(GameEvent::OnMapReload),
                Some(EvtOpcode::OnMapLeave) => Some(GameEvent::OnMapLeave),
                Some(EvtOpcode::OnCanShowDialogItemCmp) => {
                    if params.len() >= 5 {
                        Some(GameEvent::OnCanShowDialogItemCmp {
                            var: EvtVariable(params[0]),
                            value: i32_at(params, 1),
                        })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::EndCanShowDialogItem) => Some(GameEvent::EndCanShowDialogItem),
                Some(EvtOpcode::SetCanShowDialogItem) => Some(GameEvent::SetCanShowDialogItem {
                    on: params.first().copied().unwrap_or(0),
                }),
                // ── Misc ────────────────────────────────────────────────
                Some(EvtOpcode::IsActorKilled) => {
                    if params.len() >= 9 {
                        Some(GameEvent::IsActorKilled {
                            actor_group: i32_at(params, 0),
                            count: i32_at(params, 4),
                            jump_step: params[8],
                        })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::CheckSkill) => {
                    if params.len() >= 3 {
                        Some(GameEvent::CheckSkill {
                            skill_id: params[0],
                            skill_level: params[1],
                            jump_step: params[2],
                        })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::RandomGoTo) => Some(GameEvent::RandomGoTo { steps: params.to_vec() }),
                Some(EvtOpcode::SummonItem) => {
                    if params.len() >= 16 {
                        Some(GameEvent::SummonItem {
                            item_id: i32_at(params, 0),
                            x: i32_at(params, 4),
                            y: i32_at(params, 8),
                            z: i32_at(params, 12),
                        })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::CharacterAnimation) => {
                    if params.len() >= 2 {
                        Some(GameEvent::CharacterAnimation {
                            player: params[0],
                            anim_id: params[1],
                        })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::PressAnyKey) => Some(GameEvent::PressAnyKey),
                Some(EvtOpcode::SetTextureOutdoors) => {
                    // MM6 opcode 0x0C: SetTextureOutdoors
                    // Params: model(u32) + facet(u32) + texture_name(null-terminated)
                    if params.len() >= 9 {
                        let model = u32_at(params, 0);
                        let facet = u32_at(params, 4);
                        let texture_name = read_string(&params[8..]);
                        Some(GameEvent::SetTextureOutdoors {
                            model,
                            facet,
                            texture_name,
                        })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::CheckItemsCount) => {
                    if params.len() >= 9 {
                        Some(GameEvent::CheckItemsCount {
                            item_id: i32_at(params, 0),
                            count: i32_at(params, 4),
                            jump_step: params[8],
                        })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::RemoveItems) => {
                    if params.len() >= 8 {
                        Some(GameEvent::RemoveItems {
                            item_id: i32_at(params, 0),
                            count: i32_at(params, 4),
                        })
                    } else {
                        None
                    }
                }
                // ── Remaining opcodes with param parsing ─────────
                Some(EvtOpcode::InputString) => Some(GameEvent::InputString {
                    params: params.to_vec(),
                }),
                Some(EvtOpcode::SetNPCGroupNews) => {
                    if params.len() >= 8 {
                        Some(GameEvent::SetNPCGroupNews {
                            npc_group: i32_at(params, 0),
                            news_id: i32_at(params, 4),
                        })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::SetActorGroup) => {
                    if params.len() >= 8 {
                        Some(GameEvent::SetActorGroup {
                            actor_id: i32_at(params, 0),
                            group_id: i32_at(params, 4),
                        })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::NPCSetItem) => {
                    if params.len() >= 9 {
                        Some(GameEvent::NPCSetItem {
                            npc_id: i32_at(params, 0),
                            item_id: i32_at(params, 4),
                            on: params[8],
                        })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::CanShowTopicIsActorKilled) => {
                    if params.len() >= 8 {
                        Some(GameEvent::CanShowTopicIsActorKilled {
                            actor_group: i32_at(params, 0),
                            count: i32_at(params, 4),
                        })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::ChangeGroup) => {
                    if params.len() >= 8 {
                        Some(GameEvent::ChangeGroup {
                            old_group: i32_at(params, 0),
                            new_group: i32_at(params, 4),
                        })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::ChangeGroupAlly) => {
                    if params.len() >= 8 {
                        Some(GameEvent::ChangeGroupAlly {
                            group_id: i32_at(params, 0),
                            ally_group: i32_at(params, 4),
                        })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::CheckSeason) => {
                    if params.len() >= 5 {
                        Some(GameEvent::CheckSeason {
                            season: i32_at(params, 0),
                            jump_step: params[4],
                        })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::ToggleActorGroupFlag) => {
                    if params.len() >= 9 {
                        Some(GameEvent::ToggleActorGroupFlag {
                            group_id: i32_at(params, 0),
                            flag: i32_at(params, 4),
                            on: params[8],
                        })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::ToggleChestFlag) => {
                    if params.len() >= 9 {
                        Some(GameEvent::ToggleChestFlag {
                            chest_id: i32_at(params, 0),
                            flag: i32_at(params, 4),
                            on: params[8],
                        })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::SetActorItem) => {
                    if params.len() >= 9 {
                        Some(GameEvent::SetActorItem {
                            actor_id: i32_at(params, 0),
                            item_id: i32_at(params, 4),
                            on: params[8],
                        })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::OnDateTimer) => Some(GameEvent::OnDateTimer {
                    timer_data: params.to_vec(),
                }),
                Some(EvtOpcode::EnableDateTimer) => {
                    if params.len() >= 5 {
                        Some(GameEvent::EnableDateTimer {
                            timer_id: i32_at(params, 0),
                            on: params[4],
                        })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::StopAnimation) => {
                    read_i32(params).map(|v| GameEvent::StopAnimation { decoration_id: v })
                }
                Some(EvtOpcode::SpecialJump) => read_i32(params).map(|v| GameEvent::SpecialJump { jump_value: v }),
                Some(EvtOpcode::IsTotalBountyHuntingAwardInRange) => {
                    if params.len() >= 9 {
                        Some(GameEvent::IsTotalBountyHuntingAwardInRange {
                            min: i32_at(params, 0),
                            max: i32_at(params, 4),
                            jump_step: params[8],
                        })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::IsNPCInParty) => {
                    if params.len() >= 5 {
                        Some(GameEvent::IsNPCInParty {
                            npc_id: i32_at(params, 0),
                            jump_step: params[4],
                        })
                    } else {
                        None
                    }
                }
                _ => Some(GameEvent::Unhandled {
                    opcode,
                    opcode_name: evt_opcode.map_or("Unknown", |o| o.name()),
                    params: params.to_vec(),
                }),
            };

            if let Some(event) = action {
                events.entry(event_id).or_default().push(EvtStep { step, event });
            }

            pos += total;
        }

        Ok(EvtFile { events })
    }

    /// Get the primary action for an event (first SpeakInHouse or MoveToMap).
    pub fn primary_action(&self, event_id: u16) -> Option<&GameEvent> {
        self.events.get(&event_id)?.iter().find_map(|s| match &s.event {
            GameEvent::SpeakInHouse { .. } | GameEvent::MoveToMap { .. } => Some(&s.event),
            _ => None,
        })
    }
}
