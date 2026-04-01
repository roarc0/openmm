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

use crate::enums::{DoorAction, EvtOpcode, EvtTargetCharacter, EvtVariable};
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
    ChangeDoorState { door_id: u8, action: DoorAction },
    /// Play a sound effect. sound_id indexes into dsounds.bin.
    PlaySound { sound_id: u32 },
    /// Show status bar text. Uses same .str table as Hint.
    StatusText { str_id: u8, text: String },
    /// Show a location name on the screen.
    LocationName { str_id: u8, text: String },
    /// Show a message box with text.
    ShowMessage { str_id: u8, text: String },
    /// Exit/stop processing this event sequence.
    Exit,

    // ── Control flow ────────────────────────────────────────────────────
    /// Compare variable against value; if false, jump to `jump_step`.
    Compare { var: EvtVariable, value: i32, jump_step: u8 },
    /// Jump unconditionally to step.
    Jmp { target_step: u8 },
    /// Set target character for subsequent operations.
    ForPartyMember { player: u8 },

    // ── Variable operations ─────────────────────────────────────────────
    /// Add value to variable.
    Add { var: EvtVariable, value: i32 },
    /// Subtract value from variable.
    Subtract { var: EvtVariable, value: i32 },
    /// Set variable to value.
    Set { var: EvtVariable, value: i32 },

    // ── NPC / item operations ───────────────────────────────────────────
    /// Give an item to the party.
    GiveItem { strength: u8, item_type: u8, item_id: u32 },
    /// Set an NPC dialogue topic.
    SetNPCTopic { npc_id: i32, topic_index: u8, event_id: i32 },
    /// Move an NPC to a new location.
    MoveNPC { npc_id: i32, map_id: i32 },
    /// Speak to an NPC by id.
    SpeakNPC { npc_id: i32 },
    /// Change the event associated with a face/object.
    ChangeEvent { target: i32, new_event_id: i32 },
    /// Set an NPC greeting.
    SetNPCGreeting { npc_id: i32, greeting_id: i32 },

    // ── World operations ────────────────────────────────────────────────
    /// Set or clear face attribute bits.
    SetFacesBit { face_id: i32, bit: i32, on: u8 },
    /// Toggle an actor flag.
    ToggleActorFlag { actor_id: i32, flag: i32, on: u8 },
    /// Set a texture on a face.
    SetTexture { face_id: i32, texture_name: String },
    /// Set a sprite on a decoration.
    SetSprite { decoration_id: i32, sprite_name: String },
    /// Toggle an indoor light.
    ToggleIndoorLight { light_id: i32, on: u8 },
    /// Set snow/weather state.
    SetSnow { on: u8 },
    /// Summon monsters.
    SummonMonsters { monster_id: i32, count: i32, x: i32, y: i32, z: i32 },
    /// Cast a spell.
    CastSpell { spell_id: i32, skill_level: i32, skill_mastery: i32, from_x: i32, from_y: i32, from_z: i32, to_x: i32, to_y: i32, to_z: i32 },
    /// Receive damage.
    ReceiveDamage { damage_type: i32, amount: i32 },
    /// Show a character face animation.
    ShowFace { player: u8, expression: i32 },

    // ── Timer / conditional ─────────────────────────────────────────────
    /// Timer-based event (fires after delay).
    OnTimer { year: u16, month: u8, week: u8, day: u16, hour: u8, minute: u8 },
    /// Long timer (date-based).
    OnLongTimer { timer_data: Vec<u8> },
    /// Map reload hook.
    OnMapReload,
    /// Map leave hook.
    OnMapLeave,

    // ── Dialogue ────────────────────────────────────────────────────────
    /// Check if dialog item can be shown (compare variant).
    OnCanShowDialogItemCmp { var: EvtVariable, value: i32 },
    /// End can-show dialog item block.
    EndCanShowDialogItem,
    /// Set can-show dialog item flag.
    SetCanShowDialogItem { on: u8 },

    // ── Misc ────────────────────────────────────────────────────────────
    /// Check if an actor group is killed.
    IsActorKilled { actor_group: i32, count: i32, jump_step: u8 },
    /// Check skill level.
    CheckSkill { skill_id: u8, skill_level: u8, jump_step: u8 },
    /// Random goto — jump to one of several steps randomly.
    RandomGoTo { steps: Vec<u8> },
    /// Summon an item at a location.
    SummonItem { item_id: i32, x: i32, y: i32, z: i32 },
    /// Character animation.
    CharacterAnimation { player: u8, anim_id: u8 },
    /// Wait for key press.
    PressAnyKey,
    /// Show a movie.
    ShowMovie { movie_name: String },
    /// Check items count.
    CheckItemsCount { item_id: i32, count: i32, jump_step: u8 },
    /// Remove items from inventory.
    RemoveItems { item_id: i32, count: i32 },

    // ── Remaining opcodes (parsed with params, not yet executed) ──────
    /// Input string prompt (MM6-era text input).
    InputString { params: Vec<u8> },
    /// Set NPC group news.
    SetNPCGroupNews { npc_group: i32, news_id: i32 },
    /// Set actor to a group.
    SetActorGroup { actor_id: i32, group_id: i32 },
    /// Give or take an item from an NPC.
    NPCSetItem { npc_id: i32, item_id: i32, on: u8 },
    /// Can-show dialog variant of IsActorKilled.
    CanShowTopicIsActorKilled { actor_group: i32, count: i32 },
    /// Change monster group.
    ChangeGroup { old_group: i32, new_group: i32 },
    /// Change monster group ally.
    ChangeGroupAlly { group_id: i32, ally_group: i32 },
    /// Check season (0=winter, 1=spring, 2=summer, 3=autumn).
    CheckSeason { season: i32, jump_step: u8 },
    /// Toggle a flag on all actors in a group.
    ToggleActorGroupFlag { group_id: i32, flag: i32, on: u8 },
    /// Toggle a chest flag.
    ToggleChestFlag { chest_id: i32, flag: i32, on: u8 },
    /// Give or take an item on a specific actor.
    SetActorItem { actor_id: i32, item_id: i32, on: u8 },
    /// Date timer trigger.
    OnDateTimer { timer_data: Vec<u8> },
    /// Enable/disable a date timer.
    EnableDateTimer { timer_id: i32, on: u8 },
    /// Stop a decoration animation.
    StopAnimation { decoration_id: i32 },
    /// Special jump (e.g. to another event).
    SpecialJump { jump_value: i32 },
    /// Check if total bounty hunting award is in range.
    IsTotalBountyHuntingAwardInRange { min: i32, max: i32, jump_step: u8 },
    /// Check if NPC is in party.
    IsNPCInParty { npc_id: i32, jump_step: u8 },

    /// Unhandled opcode — unknown or truly unparseable.
    Unhandled { opcode: u8, opcode_name: &'static str, params: Vec<u8> },
}

/// Display implementation for readable logging.
impl std::fmt::Display for GameEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SpeakInHouse { house_id } => write!(f, "SpeakInHouse(house={})", house_id),
            Self::MoveToMap { x, y, z, direction, map_name } =>
                write!(f, "MoveToMap('{}' pos=({},{},{}) dir={})", map_name, x, y, z, direction),
            Self::OpenChest { id } => write!(f, "OpenChest({})", id),
            Self::Hint { text, .. } => write!(f, "Hint('{}')", text),
            Self::ChangeDoorState { door_id, action } =>
                write!(f, "ChangeDoorState(door={} action={})", door_id, action),
            Self::PlaySound { sound_id } => write!(f, "PlaySound({})", sound_id),
            Self::StatusText { text, .. } => write!(f, "StatusText('{}')", text),
            Self::LocationName { text, .. } => write!(f, "LocationName('{}')", text),
            Self::ShowMessage { text, .. } => write!(f, "ShowMessage('{}')", text),
            Self::Exit => write!(f, "Exit"),
            Self::Compare { var, value, jump_step } => {
                if *var == EvtVariable::QBITS {
                    write!(f, "Compare(QBit[{}] set? else step {})", value, jump_step)
                } else if *var == EvtVariable::AUTONOTES_BITS {
                    write!(f, "Compare(Autonote[{}] set? else step {})", value, jump_step)
                } else if *var == EvtVariable::INVENTORY {
                    write!(f, "Compare(HasItem({})? else step {})", value, jump_step)
                } else {
                    write!(f, "Compare({} >= {}? else step {})", var, value, jump_step)
                }
            }
            Self::Jmp { target_step } => write!(f, "Jmp(step {})", target_step),
            Self::ForPartyMember { player } => {
                let name = EvtTargetCharacter::from_u8(*player)
                    .map(|t| format!("{:?}", t))
                    .unwrap_or_else(|| player.to_string());
                write!(f, "ForPartyMember({})", name)
            }
            Self::Add { var, value } => {
                if *var == EvtVariable::QBITS { write!(f, "Add(QBit[{}] = true)", value) }
                else if *var == EvtVariable::AUTONOTES_BITS { write!(f, "Add(Autonote[{}] = true)", value) }
                else { write!(f, "Add({} += {})", var, value) }
            }
            Self::Subtract { var, value } => {
                if *var == EvtVariable::QBITS { write!(f, "Subtract(QBit[{}] = false)", value) }
                else if *var == EvtVariable::AUTONOTES_BITS { write!(f, "Subtract(Autonote[{}] = false)", value) }
                else { write!(f, "Subtract({} -= {})", var, value) }
            }
            Self::Set { var, value } => {
                if *var == EvtVariable::QBITS { write!(f, "Set(QBit[{}] = true)", value) }
                else if *var == EvtVariable::AUTONOTES_BITS { write!(f, "Set(Autonote[{}] = true)", value) }
                else { write!(f, "Set({} = {})", var, value) }
            }
            Self::GiveItem { strength, item_type, item_id } =>
                write!(f, "GiveItem(str={} type={} id={})", strength, item_type, item_id),
            Self::SetNPCTopic { npc_id, topic_index, event_id } =>
                write!(f, "SetNPCTopic(npc={} topic={} event={})", npc_id, topic_index, event_id),
            Self::MoveNPC { npc_id, map_id } =>
                write!(f, "MoveNPC(npc={} map={})", npc_id, map_id),
            Self::SpeakNPC { npc_id } => write!(f, "SpeakNPC(npc={})", npc_id),
            Self::ChangeEvent { target, new_event_id } =>
                write!(f, "ChangeEvent(target={} event={})", target, new_event_id),
            Self::SetNPCGreeting { npc_id, greeting_id } =>
                write!(f, "SetNPCGreeting(npc={} greeting={})", npc_id, greeting_id),
            Self::SetFacesBit { face_id, bit, on } =>
                write!(f, "SetFacesBit(face={} bit=0x{:x} on={})", face_id, bit, on),
            Self::ToggleActorFlag { actor_id, flag, on } =>
                write!(f, "ToggleActorFlag(actor={} flag=0x{:x} on={})", actor_id, flag, on),
            Self::SetTexture { face_id, texture_name } =>
                write!(f, "SetTexture(face={} tex='{}')", face_id, texture_name),
            Self::SetSprite { decoration_id, sprite_name } =>
                write!(f, "SetSprite(deco={} sprite='{}')", decoration_id, sprite_name),
            Self::ToggleIndoorLight { light_id, on } =>
                write!(f, "ToggleIndoorLight(light={} on={})", light_id, on),
            Self::SetSnow { on } => write!(f, "SetSnow(on={})", on),
            Self::SummonMonsters { monster_id, count, x, y, z } =>
                write!(f, "SummonMonsters(id={} count={} pos=({},{},{}))", monster_id, count, x, y, z),
            Self::CastSpell { spell_id, skill_level, skill_mastery, .. } =>
                write!(f, "CastSpell(spell={} level={} mastery={})", spell_id, skill_level, skill_mastery),
            Self::ReceiveDamage { damage_type, amount } =>
                write!(f, "ReceiveDamage(type={} amount={})", damage_type, amount),
            Self::ShowFace { player, expression } =>
                write!(f, "ShowFace(player={} expr={})", player, expression),
            Self::OnTimer { year, month, week, day, hour, minute } =>
                write!(f, "OnTimer(y={} m={} w={} d={} h={} min={})", year, month, week, day, hour, minute),
            Self::OnLongTimer { timer_data } =>
                write!(f, "OnLongTimer(data={:02x?})", timer_data),
            Self::OnMapReload => write!(f, "OnMapReload"),
            Self::OnMapLeave => write!(f, "OnMapLeave"),
            Self::OnCanShowDialogItemCmp { var, value } =>
                write!(f, "OnCanShowDialogItemCmp({} == {}?)", var, value),
            Self::EndCanShowDialogItem => write!(f, "EndCanShowDialogItem"),
            Self::SetCanShowDialogItem { on } =>
                write!(f, "SetCanShowDialogItem(on={})", on),
            Self::IsActorKilled { actor_group, count, jump_step } =>
                write!(f, "IsActorKilled(group={} count={} else step {})", actor_group, count, jump_step),
            Self::CheckSkill { skill_id, skill_level, jump_step } =>
                write!(f, "CheckSkill(skill={} level={} else step {})", skill_id, skill_level, jump_step),
            Self::RandomGoTo { steps } =>
                write!(f, "RandomGoTo(steps={:?})", steps),
            Self::SummonItem { item_id, x, y, z } =>
                write!(f, "SummonItem(id={} pos=({},{},{}))", item_id, x, y, z),
            Self::CharacterAnimation { player, anim_id } =>
                write!(f, "CharacterAnimation(player={} anim={})", player, anim_id),
            Self::PressAnyKey => write!(f, "PressAnyKey"),
            Self::ShowMovie { movie_name } =>
                write!(f, "ShowMovie('{}')", movie_name),
            Self::CheckItemsCount { item_id, count, jump_step } =>
                write!(f, "CheckItemsCount(item={} count={} else step {})", item_id, count, jump_step),
            Self::RemoveItems { item_id, count } =>
                write!(f, "RemoveItems(item={} count={})", item_id, count),
            Self::InputString { params } =>
                write!(f, "InputString(params={:02x?})", params),
            Self::SetNPCGroupNews { npc_group, news_id } =>
                write!(f, "SetNPCGroupNews(group={} news={})", npc_group, news_id),
            Self::SetActorGroup { actor_id, group_id } =>
                write!(f, "SetActorGroup(actor={} group={})", actor_id, group_id),
            Self::NPCSetItem { npc_id, item_id, on } =>
                write!(f, "NPCSetItem(npc={} item={} on={})", npc_id, item_id, on),
            Self::CanShowTopicIsActorKilled { actor_group, count } =>
                write!(f, "CanShowTopicIsActorKilled(group={} count={})", actor_group, count),
            Self::ChangeGroup { old_group, new_group } =>
                write!(f, "ChangeGroup(old={} new={})", old_group, new_group),
            Self::ChangeGroupAlly { group_id, ally_group } =>
                write!(f, "ChangeGroupAlly(group={} ally={})", group_id, ally_group),
            Self::CheckSeason { season, jump_step } => {
                let name = match season {
                    0 => "Winter",
                    1 => "Spring",
                    2 => "Summer",
                    3 => "Autumn",
                    _ => "Unknown",
                };
                write!(f, "CheckSeason({}={} else step {})", name, season, jump_step)
            }
            Self::ToggleActorGroupFlag { group_id, flag, on } =>
                write!(f, "ToggleActorGroupFlag(group={} flag=0x{:x} on={})", group_id, flag, on),
            Self::ToggleChestFlag { chest_id, flag, on } =>
                write!(f, "ToggleChestFlag(chest={} flag=0x{:x} on={})", chest_id, flag, on),
            Self::SetActorItem { actor_id, item_id, on } =>
                write!(f, "SetActorItem(actor={} item={} on={})", actor_id, item_id, on),
            Self::OnDateTimer { timer_data } =>
                write!(f, "OnDateTimer(data={:02x?})", timer_data),
            Self::EnableDateTimer { timer_id, on } =>
                write!(f, "EnableDateTimer(timer={} on={})", timer_id, on),
            Self::StopAnimation { decoration_id } =>
                write!(f, "StopAnimation(deco={})", decoration_id),
            Self::SpecialJump { jump_value } =>
                write!(f, "SpecialJump(value={})", jump_value),
            Self::IsTotalBountyHuntingAwardInRange { min, max, jump_step } =>
                write!(f, "IsTotalBountyHuntingAwardInRange(min={} max={} else step {})", min, max, jump_step),
            Self::IsNPCInParty { npc_id, jump_step } =>
                write!(f, "IsNPCInParty(npc={} else step {})", npc_id, jump_step),
            Self::Unhandled { opcode, opcode_name, params } =>
                write!(f, "Unhandled(0x{:02x} {} params={:02x?})", opcode, opcode_name, params),
        }
    }
}

/// A single step in an event script: step number + action.
#[derive(Debug, Clone)]
pub struct EvtStep {
    pub step: u8,
    pub event: GameEvent,
}

/// Parsed events from a .evt file, keyed by event_id.
pub struct EvtFile {
    /// For each event_id, the list of steps (step number + action).
    pub events: HashMap<u16, Vec<EvtStep>>,
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
    if params.len() >= 4 { Some(i32_at(params, 0)) } else { None }
}

/// Read a null-terminated string from params.
fn read_string(params: &[u8]) -> String {
    let end = params.iter().position(|&b| b == 0).unwrap_or(params.len());
    String::from_utf8_lossy(&params[..end]).to_string()
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
                Some(EvtOpcode::SpeakInHouse) => read_i32(params).map(|v| GameEvent::SpeakInHouse { house_id: v as u32 }),
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
                        Some(GameEvent::MoveToMap { x, y, z, direction, map_name })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::OpenChest) => {
                    Some(GameEvent::OpenChest { id: params.first().copied().unwrap_or(0) })
                }
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
                Some(EvtOpcode::Jmp) => {
                    Some(GameEvent::Jmp { target_step: params.first().copied().unwrap_or(0) })
                }
                Some(EvtOpcode::ForPartyMember) => {
                    Some(GameEvent::ForPartyMember { player: params.first().copied().unwrap_or(0) })
                }
                // ── Variable operations ─────────────────────────────────
                // MM6: Add/Subtract/Set = var_id(u8) + value(i32 LE) = 5 bytes
                Some(EvtOpcode::Add) => {
                    if params.len() >= 5 {
                        Some(GameEvent::Add { var: EvtVariable(params[0]), value: i32_at(params, 1) })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::Subtract) => {
                    if params.len() >= 5 {
                        Some(GameEvent::Subtract { var: EvtVariable(params[0]), value: i32_at(params, 1) })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::Set) => {
                    if params.len() >= 5 {
                        Some(GameEvent::Set { var: EvtVariable(params[0]), value: i32_at(params, 1) })
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
                        Some(GameEvent::MoveNPC { npc_id: i32_at(params, 0), map_id: i32_at(params, 4) })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::SpeakNPC) => {
                    read_i32(params).map(|v| GameEvent::SpeakNPC { npc_id: v })
                }
                Some(EvtOpcode::ChangeEvent) => {
                    if params.len() >= 8 {
                        Some(GameEvent::ChangeEvent { target: i32_at(params, 0), new_event_id: i32_at(params, 4) })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::SetNPCGreeting) => {
                    if params.len() >= 8 {
                        Some(GameEvent::SetNPCGreeting { npc_id: i32_at(params, 0), greeting_id: i32_at(params, 4) })
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
                        Some(GameEvent::SetTexture { face_id, texture_name: name })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::SetSprite) => {
                    // Format: cog(u32) + hide(u8) + name(null-terminated string)
                    if params.len() >= 6 {
                        let decoration_id = i32_at(params, 0);
                        let name = read_string(&params[5..]);
                        Some(GameEvent::SetSprite { decoration_id, sprite_name: name })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::ToggleIndoorLight) => {
                    if params.len() >= 5 {
                        Some(GameEvent::ToggleIndoorLight { light_id: i32_at(params, 0), on: params[4] })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::SetSnow) => {
                    Some(GameEvent::SetSnow { on: params.first().copied().unwrap_or(0) })
                }
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
                        Some(GameEvent::ReceiveDamage { damage_type: i32_at(params, 0), amount: i32_at(params, 4) })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::ShowFace) => {
                    if params.len() >= 5 {
                        Some(GameEvent::ShowFace { player: params[0], expression: i32_at(params, 1) })
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
                Some(EvtOpcode::OnLongTimer) => {
                    Some(GameEvent::OnLongTimer { timer_data: params.to_vec() })
                }
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
                Some(EvtOpcode::SetCanShowDialogItem) => {
                    Some(GameEvent::SetCanShowDialogItem { on: params.first().copied().unwrap_or(0) })
                }
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
                Some(EvtOpcode::RandomGoTo) => {
                    Some(GameEvent::RandomGoTo { steps: params.to_vec() })
                }
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
                        Some(GameEvent::CharacterAnimation { player: params[0], anim_id: params[1] })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::PressAnyKey) => Some(GameEvent::PressAnyKey),
                Some(EvtOpcode::ShowMovie) => {
                    Some(GameEvent::ShowMovie { movie_name: read_string(params) })
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
                        Some(GameEvent::RemoveItems { item_id: i32_at(params, 0), count: i32_at(params, 4) })
                    } else {
                        None
                    }
                }
                // ── Remaining opcodes with param parsing ─────────
                Some(EvtOpcode::InputString) => {
                    Some(GameEvent::InputString { params: params.to_vec() })
                }
                Some(EvtOpcode::SetNPCGroupNews) => {
                    if params.len() >= 8 {
                        Some(GameEvent::SetNPCGroupNews { npc_group: i32_at(params, 0), news_id: i32_at(params, 4) })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::SetActorGroup) => {
                    if params.len() >= 8 {
                        Some(GameEvent::SetActorGroup { actor_id: i32_at(params, 0), group_id: i32_at(params, 4) })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::NPCSetItem) => {
                    if params.len() >= 9 {
                        Some(GameEvent::NPCSetItem { npc_id: i32_at(params, 0), item_id: i32_at(params, 4), on: params[8] })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::CanShowTopicIsActorKilled) => {
                    if params.len() >= 8 {
                        Some(GameEvent::CanShowTopicIsActorKilled { actor_group: i32_at(params, 0), count: i32_at(params, 4) })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::ChangeGroup) => {
                    if params.len() >= 8 {
                        Some(GameEvent::ChangeGroup { old_group: i32_at(params, 0), new_group: i32_at(params, 4) })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::ChangeGroupAlly) => {
                    if params.len() >= 8 {
                        Some(GameEvent::ChangeGroupAlly { group_id: i32_at(params, 0), ally_group: i32_at(params, 4) })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::CheckSeason) => {
                    if params.len() >= 5 {
                        Some(GameEvent::CheckSeason { season: i32_at(params, 0), jump_step: params[4] })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::ToggleActorGroupFlag) => {
                    if params.len() >= 9 {
                        Some(GameEvent::ToggleActorGroupFlag { group_id: i32_at(params, 0), flag: i32_at(params, 4), on: params[8] })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::ToggleChestFlag) => {
                    if params.len() >= 9 {
                        Some(GameEvent::ToggleChestFlag { chest_id: i32_at(params, 0), flag: i32_at(params, 4), on: params[8] })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::SetActorItem) => {
                    if params.len() >= 9 {
                        Some(GameEvent::SetActorItem { actor_id: i32_at(params, 0), item_id: i32_at(params, 4), on: params[8] })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::OnDateTimer) => {
                    Some(GameEvent::OnDateTimer { timer_data: params.to_vec() })
                }
                Some(EvtOpcode::EnableDateTimer) => {
                    if params.len() >= 5 {
                        Some(GameEvent::EnableDateTimer { timer_id: i32_at(params, 0), on: params[4] })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::StopAnimation) => {
                    read_i32(params).map(|v| GameEvent::StopAnimation { decoration_id: v })
                }
                Some(EvtOpcode::SpecialJump) => {
                    read_i32(params).map(|v| GameEvent::SpecialJump { jump_value: v })
                }
                Some(EvtOpcode::IsTotalBountyHuntingAwardInRange) => {
                    if params.len() >= 9 {
                        Some(GameEvent::IsTotalBountyHuntingAwardInRange {
                            min: i32_at(params, 0), max: i32_at(params, 4), jump_step: params[8],
                        })
                    } else {
                        None
                    }
                }
                Some(EvtOpcode::IsNPCInParty) => {
                    if params.len() >= 5 {
                        Some(GameEvent::IsNPCInParty { npc_id: i32_at(params, 0), jump_step: params[4] })
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
