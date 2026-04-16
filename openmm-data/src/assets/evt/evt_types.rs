//! Type definitions for EVT event scripts: `GameEvent` enum and `EvtStep`.

use crate::assets::enums::{DoorAction, EvtTargetCharacter, EvtVariable};

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
    /// Play a Smacker video by name. Transitions to GameState::Video; returns to Game when done.
    PlayVideo { name: String, skippable: bool },
    /// Exit/stop processing this event sequence.
    Exit,

    // ── Control flow ────────────────────────────────────────────────────
    /// Compare variable against value; if condition IS MET (true), jump to `jump_step`.
    /// Used to skip an action when already done (e.g. already picked, already visited).
    Compare {
        var: EvtVariable,
        value: i32,
        jump_step: u8,
    },
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
    SetNPCTopic {
        npc_id: i32,
        topic_index: u8,
        event_id: i32,
    },
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
    SummonMonsters {
        monster_id: i32,
        count: i32,
        x: i32,
        y: i32,
        z: i32,
    },
    /// Cast a spell.
    CastSpell {
        spell_id: i32,
        skill_level: i32,
        skill_mastery: i32,
        from_x: i32,
        from_y: i32,
        from_z: i32,
        to_x: i32,
        to_y: i32,
        to_z: i32,
    },
    /// Receive damage.
    ReceiveDamage { damage_type: i32, amount: i32 },
    /// Show a character face animation.
    ShowFace { player: u8, expression: i32 },

    // ── Timer / conditional ─────────────────────────────────────────────
    /// Timer-based event (fires after delay).
    OnTimer {
        year: u16,
        month: u8,
        week: u8,
        day: u16,
        hour: u8,
        minute: u8,
    },
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
    IsActorKilled {
        actor_group: i32,
        count: i32,
        jump_step: u8,
    },
    /// Check skill level.
    CheckSkill {
        skill_id: u8,
        skill_level: u8,
        jump_step: u8,
    },
    /// Random goto — jump to one of several steps randomly.
    RandomGoTo { steps: Vec<u8> },
    /// Summon an item at a location.
    SummonItem { item_id: i32, x: i32, y: i32, z: i32 },
    /// Character animation.
    CharacterAnimation { player: u8, anim_id: u8 },
    /// Wait for key press.
    PressAnyKey,
    /// Set texture on an outdoor BSP model face (MM6 opcode 0x0C).
    /// model = BSP model index, facet = face index within model, texture_name = new texture.
    SetTextureOutdoors {
        model: u32,
        facet: u32,
        texture_name: String,
    },
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
    Unhandled {
        opcode: u8,
        opcode_name: &'static str,
        params: Vec<u8>,
    },
}

/// Display implementation for readable logging.
impl std::fmt::Display for GameEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SpeakInHouse { house_id } => write!(f, "SpeakInHouse(house={})", house_id),
            Self::MoveToMap {
                x,
                y,
                z,
                direction,
                map_name,
            } => write!(f, "MoveToMap('{}' pos=({},{},{}) dir={})", map_name, x, y, z, direction),
            Self::OpenChest { id } => write!(f, "OpenChest({})", id),
            Self::Hint { text, .. } => write!(f, "Hint('{}')", text),
            Self::ChangeDoorState { door_id, action } => {
                write!(f, "ChangeDoorState(door={} action={})", door_id, action)
            }
            Self::PlaySound { sound_id } => write!(f, "PlaySound({})", sound_id),
            Self::StatusText { text, .. } => write!(f, "StatusText('{}')", text),
            Self::LocationName { text, .. } => write!(f, "LocationName('{}')", text),
            Self::ShowMessage { text, .. } => write!(f, "ShowMessage('{}')", text),
            Self::PlayVideo { name, skippable } => write!(f, "PlayVideo('{}' skippable={})", name, skippable),
            Self::Exit => write!(f, "Exit"),
            Self::Compare { var, value, jump_step } => {
                if *var == EvtVariable::QBITS {
                    write!(f, "Compare(QBit[{}] set? skip step {})", value, jump_step)
                } else if *var == EvtVariable::AUTONOTES_BITS {
                    write!(f, "Compare(Autonote[{}] set? skip step {})", value, jump_step)
                } else if *var == EvtVariable::INVENTORY {
                    write!(f, "Compare(HasItem({})? skip step {})", value, jump_step)
                } else {
                    write!(f, "Compare({} >= {}? skip step {})", var, value, jump_step)
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
                if *var == EvtVariable::QBITS {
                    write!(f, "Add(QBit[{}] = true)", value)
                } else if *var == EvtVariable::AUTONOTES_BITS {
                    write!(f, "Add(Autonote[{}] = true)", value)
                } else {
                    write!(f, "Add({} += {})", var, value)
                }
            }
            Self::Subtract { var, value } => {
                if *var == EvtVariable::QBITS {
                    write!(f, "Subtract(QBit[{}] = false)", value)
                } else if *var == EvtVariable::AUTONOTES_BITS {
                    write!(f, "Subtract(Autonote[{}] = false)", value)
                } else {
                    write!(f, "Subtract({} -= {})", var, value)
                }
            }
            Self::Set { var, value } => {
                if *var == EvtVariable::QBITS {
                    write!(f, "Set(QBit[{}] = true)", value)
                } else if *var == EvtVariable::AUTONOTES_BITS {
                    write!(f, "Set(Autonote[{}] = true)", value)
                } else {
                    write!(f, "Set({} = {})", var, value)
                }
            }
            Self::GiveItem {
                strength,
                item_type,
                item_id,
            } => write!(f, "GiveItem(str={} type={} id={})", strength, item_type, item_id),
            Self::SetNPCTopic {
                npc_id,
                topic_index,
                event_id,
            } => write!(
                f,
                "SetNPCTopic(npc={} topic={} event={})",
                npc_id, topic_index, event_id
            ),
            Self::MoveNPC { npc_id, map_id } => write!(f, "MoveNPC(npc={} map={})", npc_id, map_id),
            Self::SpeakNPC { npc_id } => write!(f, "SpeakNPC(npc={})", npc_id),
            Self::ChangeEvent { target, new_event_id } => {
                write!(f, "ChangeEvent(target={} event={})", target, new_event_id)
            }
            Self::SetNPCGreeting { npc_id, greeting_id } => {
                write!(f, "SetNPCGreeting(npc={} greeting={})", npc_id, greeting_id)
            }
            Self::SetFacesBit { face_id, bit, on } => {
                write!(f, "SetFacesBit(face={} bit=0x{:x} on={})", face_id, bit, on)
            }
            Self::ToggleActorFlag { actor_id, flag, on } => {
                write!(f, "ToggleActorFlag(actor={} flag=0x{:x} on={})", actor_id, flag, on)
            }
            Self::SetTexture { face_id, texture_name } => {
                write!(f, "SetTexture(face={} tex='{}')", face_id, texture_name)
            }
            Self::SetSprite {
                decoration_id,
                sprite_name,
            } => write!(f, "SetSprite(deco={} sprite='{}')", decoration_id, sprite_name),
            Self::ToggleIndoorLight { light_id, on } => write!(f, "ToggleIndoorLight(light={} on={})", light_id, on),
            Self::SetSnow { on } => write!(f, "SetSnow(on={})", on),
            Self::SummonMonsters {
                monster_id,
                count,
                x,
                y,
                z,
            } => write!(
                f,
                "SummonMonsters(id={} count={} pos=({},{},{}))",
                monster_id, count, x, y, z
            ),
            Self::CastSpell {
                spell_id,
                skill_level,
                skill_mastery,
                ..
            } => write!(
                f,
                "CastSpell(spell={} level={} mastery={})",
                spell_id, skill_level, skill_mastery
            ),
            Self::ReceiveDamage { damage_type, amount } => {
                write!(f, "ReceiveDamage(type={} amount={})", damage_type, amount)
            }
            Self::ShowFace { player, expression } => write!(f, "ShowFace(player={} expr={})", player, expression),
            Self::OnTimer {
                year,
                month,
                week,
                day,
                hour,
                minute,
            } => write!(
                f,
                "OnTimer(y={} m={} w={} d={} h={} min={})",
                year, month, week, day, hour, minute
            ),
            Self::OnLongTimer { timer_data } => write!(f, "OnLongTimer(data={:02x?})", timer_data),
            Self::OnMapReload => write!(f, "OnMapReload"),
            Self::OnMapLeave => write!(f, "OnMapLeave"),
            Self::OnCanShowDialogItemCmp { var, value } => write!(f, "OnCanShowDialogItemCmp({} == {}?)", var, value),
            Self::EndCanShowDialogItem => write!(f, "EndCanShowDialogItem"),
            Self::SetCanShowDialogItem { on } => write!(f, "SetCanShowDialogItem(on={})", on),
            Self::IsActorKilled {
                actor_group,
                count,
                jump_step,
            } => write!(
                f,
                "IsActorKilled(group={} count={} else step {})",
                actor_group, count, jump_step
            ),
            Self::CheckSkill {
                skill_id,
                skill_level,
                jump_step,
            } => write!(
                f,
                "CheckSkill(skill={} level={} else step {})",
                skill_id, skill_level, jump_step
            ),
            Self::RandomGoTo { steps } => write!(f, "RandomGoTo(steps={:?})", steps),
            Self::SummonItem { item_id, x, y, z } => write!(f, "SummonItem(id={} pos=({},{},{}))", item_id, x, y, z),
            Self::CharacterAnimation { player, anim_id } => {
                write!(f, "CharacterAnimation(player={} anim={})", player, anim_id)
            }
            Self::PressAnyKey => write!(f, "PressAnyKey"),
            Self::SetTextureOutdoors {
                model,
                facet,
                texture_name,
            } => {
                write!(
                    f,
                    "SetTextureOutdoors(model={} facet={} tex='{}')",
                    model, facet, texture_name
                )
            }
            Self::CheckItemsCount {
                item_id,
                count,
                jump_step,
            } => write!(
                f,
                "CheckItemsCount(item={} count={} else step {})",
                item_id, count, jump_step
            ),
            Self::RemoveItems { item_id, count } => write!(f, "RemoveItems(item={} count={})", item_id, count),
            Self::InputString { params } => write!(f, "InputString(params={:02x?})", params),
            Self::SetNPCGroupNews { npc_group, news_id } => {
                write!(f, "SetNPCGroupNews(group={} news={})", npc_group, news_id)
            }
            Self::SetActorGroup { actor_id, group_id } => {
                write!(f, "SetActorGroup(actor={} group={})", actor_id, group_id)
            }
            Self::NPCSetItem { npc_id, item_id, on } => {
                write!(f, "NPCSetItem(npc={} item={} on={})", npc_id, item_id, on)
            }
            Self::CanShowTopicIsActorKilled { actor_group, count } => {
                write!(f, "CanShowTopicIsActorKilled(group={} count={})", actor_group, count)
            }
            Self::ChangeGroup { old_group, new_group } => write!(f, "ChangeGroup(old={} new={})", old_group, new_group),
            Self::ChangeGroupAlly { group_id, ally_group } => {
                write!(f, "ChangeGroupAlly(group={} ally={})", group_id, ally_group)
            }
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
            Self::ToggleActorGroupFlag { group_id, flag, on } => write!(
                f,
                "ToggleActorGroupFlag(group={} flag=0x{:x} on={})",
                group_id, flag, on
            ),
            Self::ToggleChestFlag { chest_id, flag, on } => {
                write!(f, "ToggleChestFlag(chest={} flag=0x{:x} on={})", chest_id, flag, on)
            }
            Self::SetActorItem { actor_id, item_id, on } => {
                write!(f, "SetActorItem(actor={} item={} on={})", actor_id, item_id, on)
            }
            Self::OnDateTimer { timer_data } => write!(f, "OnDateTimer(data={:02x?})", timer_data),
            Self::EnableDateTimer { timer_id, on } => write!(f, "EnableDateTimer(timer={} on={})", timer_id, on),
            Self::StopAnimation { decoration_id } => write!(f, "StopAnimation(deco={})", decoration_id),
            Self::SpecialJump { jump_value } => write!(f, "SpecialJump(value={})", jump_value),
            Self::IsTotalBountyHuntingAwardInRange { min, max, jump_step } => write!(
                f,
                "IsTotalBountyHuntingAwardInRange(min={} max={} else step {})",
                min, max, jump_step
            ),
            Self::IsNPCInParty { npc_id, jump_step } => {
                write!(f, "IsNPCInParty(npc={} else step {})", npc_id, jump_step)
            }
            Self::Unhandled {
                opcode,
                opcode_name,
                params,
            } => write!(f, "Unhandled(0x{:02x} {} params={:02x?})", opcode, opcode_name, params),
        }
    }
}

/// A single step in an event script: step number + action.
#[derive(Debug, Clone)]
pub struct EvtStep {
    pub step: u8,
    pub event: GameEvent,
}
