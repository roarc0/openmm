//! Party creation UI state — controls the "makeme" character creation screen.
//!
//! The actual character data lives in [`Party`] — this module only holds
//! transient UI state: which member is selected, which stat is highlighted,
//! and how many bonus points remain to allocate.
//!
//! Exposes property bindings `member0`–`char3` via [`PropertySource`] so RON
//! screens can display portrait, class, stats, etc.

use bevy::ecs::message::MessageReader;
use bevy::prelude::*;

use crate::game::player::party::Party;
use crate::game::player::party::creation;
use crate::game::player::party::member::{CharStat, CharacterClass, PartyMember};
use crate::game::player::party::portrait::PortraitId;
use crate::screens::PropertySource;
use crate::screens::runtime::RuntimeElement;
use crate::screens::runtime::ScreenActionEvent;

/// Transient UI state for party creation. Does NOT hold character data — that's in `Party`.
#[derive(Resource)]
pub struct PartyCreationState {
    /// Which party member (0–3) is currently focused for stat allocation.
    pub active_member: usize,
    /// Currently highlighted stat for the active member.
    pub selected_stat: [CharStat; 4],
    /// Remaining bonus points shared across all members.
    pub bonus_points: u8,
    /// Chosen optional skills per member (index into class_available_skills, or None).
    pub chosen_skills: [[Option<usize>; 2]; 4],
}

impl Default for PartyCreationState {
    fn default() -> Self {
        Self {
            active_member: 0,
            selected_stat: [CharStat::default(); 4],
            bonus_points: 50,
            chosen_skills: [[None; 2]; 4],
        }
    }
}

/// Sentinel index for the "active" member property source alias.
const ACTIVE_SOURCE_INDEX: usize = 99;

// ── PropertySource: expose Party data to RON screens ────────────────────────

/// Bridges a single party member's data to the screen property system.
struct PartyMemberSource {
    index: usize,
    member: PartyMember,
    selected_stat: CharStat,
    class_base_attrs: [i16; 7],
    /// Which optional skill slots have been filled (indices into class_available_skills).
    chosen_skills: [Option<usize>; 2],
}

impl PartyMemberSource {
    /// Color for a stat label: green if above base, red if below, white if equal.
    fn stat_label_color(&self, stat: CharStat) -> &'static str {
        let idx = stat.attr_index();
        stat_value_color(self.member.base_attrs[idx], self.class_base_attrs[idx])
    }

    /// Color for a stat value: green if above base, red if below, white if equal.
    fn stat_value_color(&self, stat: CharStat) -> &'static str {
        let idx = stat.attr_index();
        stat_value_color(self.member.base_attrs[idx], self.class_base_attrs[idx])
    }

    fn selected_class_color(&self, class: CharacterClass) -> &'static str {
        if self.member.class == class { "cyan" } else { "white" }
    }
}

impl PropertySource for PartyMemberSource {
    fn source_name(&self) -> &str {
        match self.index {
            0 => "member0",
            1 => "member1",
            2 => "member2",
            3 => "member3",
            ACTIVE_SOURCE_INDEX => "active",
            _ => "member0",
        }
    }

    fn resolve(&self, path: &str) -> Option<String> {
        let m = &self.member;
        match path {
            "portrait" => Some(m.portrait.creation_texture().to_string()),
            "class" => Some(m.class.name().to_string()),
            "class_icon" => Some(m.class.icon().to_string()),
            "class_color_knight" => Some(self.selected_class_color(CharacterClass::Knight).to_string()),
            "class_color_paladin" => Some(self.selected_class_color(CharacterClass::Paladin).to_string()),
            "class_color_archer" => Some(self.selected_class_color(CharacterClass::Archer).to_string()),
            "class_color_cleric" => Some(self.selected_class_color(CharacterClass::Cleric).to_string()),
            "class_color_druid" => Some(self.selected_class_color(CharacterClass::Druid).to_string()),
            "class_color_sorcerer" => Some(self.selected_class_color(CharacterClass::Sorcerer).to_string()),
            "gender" => Some(if m.portrait.is_male() { "male" } else { "female" }.to_string()),
            "selected_stat" => Some(self.selected_stat.name().to_string()),
            "stat_color_might" => Some(self.stat_label_color(CharStat::Might).to_string()),
            "stat_color_intellect" => Some(self.stat_label_color(CharStat::Intellect).to_string()),
            "stat_color_personality" => Some(self.stat_label_color(CharStat::Personality).to_string()),
            "stat_color_endurance" => Some(self.stat_label_color(CharStat::Endurance).to_string()),
            "stat_color_accuracy" => Some(self.stat_label_color(CharStat::Accuracy).to_string()),
            "stat_color_speed" => Some(self.stat_label_color(CharStat::Speed).to_string()),
            "stat_color_luck" => Some(self.stat_label_color(CharStat::Luck).to_string()),
            "val_color_might" => Some(self.stat_value_color(CharStat::Might).to_string()),
            "val_color_intellect" => Some(self.stat_value_color(CharStat::Intellect).to_string()),
            "val_color_personality" => Some(self.stat_value_color(CharStat::Personality).to_string()),
            "val_color_endurance" => Some(self.stat_value_color(CharStat::Endurance).to_string()),
            "val_color_accuracy" => Some(self.stat_value_color(CharStat::Accuracy).to_string()),
            "val_color_speed" => Some(self.stat_value_color(CharStat::Speed).to_string()),
            "val_color_luck" => Some(self.stat_value_color(CharStat::Luck).to_string()),
            "name" => Some(m.name.clone()),
            "might" => Some(m.base_attrs[0].to_string()),
            "intellect" => Some(m.base_attrs[1].to_string()),
            "personality" => Some(m.base_attrs[2].to_string()),
            "endurance" => Some(m.base_attrs[3].to_string()),
            "speed" => Some(m.base_attrs[4].to_string()),
            "accuracy" => Some(m.base_attrs[5].to_string()),
            "luck" => Some(m.base_attrs[6].to_string()),
            // Fixed starting skills (slots 0-1)
            "skill_0" | "skill_1" => {
                let idx: usize = path[6..].parse().ok()?;
                let skills = creation::class_starting_skills(m.class);
                Some(skills.get(idx).unwrap_or(&"None").to_string())
            }
            // Chosen optional skills (slots 2-3)
            "skill_2" | "skill_3" => {
                let slot: usize = path[6..].parse::<usize>().ok()? - 2;
                let avail = creation::class_available_skills(m.class);
                match self.chosen_skills[slot] {
                    Some(ai) => Some(avail.get(ai).unwrap_or(&"None").to_string()),
                    None => Some("None".to_string()),
                }
            }
            // Colors for skill slots: fixed=white, chosen=green, empty=white
            "skill_0_color" | "skill_1_color" => Some("white".to_string()),
            "skill_2_color" => Some(
                if self.chosen_skills[0].is_some() {
                    "green"
                } else {
                    "cyan"
                }
                .to_string(),
            ),
            "skill_3_color" => Some(
                if self.chosen_skills[1].is_some() {
                    "green"
                } else {
                    "cyan"
                }
                .to_string(),
            ),
            // Available skills for the active member's class (9 slots)
            "av_skill_0" | "av_skill_1" | "av_skill_2" | "av_skill_3" | "av_skill_4" | "av_skill_5" | "av_skill_6"
            | "av_skill_7" | "av_skill_8" => {
                let idx: usize = path[9..].parse().ok()?;
                let avail = creation::class_available_skills(m.class);
                Some(avail.get(idx).unwrap_or(&"").to_string())
            }
            // Color for available skill: cyan if already chosen, white otherwise
            "av_skill_0_color" | "av_skill_1_color" | "av_skill_2_color" | "av_skill_3_color" | "av_skill_4_color"
            | "av_skill_5_color" | "av_skill_6_color" | "av_skill_7_color" | "av_skill_8_color" => {
                let idx: usize = path[9..path.len() - 6].parse().ok()?;
                let is_chosen = self.chosen_skills.iter().any(|c| *c == Some(idx));
                Some(if is_chosen { "cyan" } else { "white" }.to_string())
            }
            _ => None,
        }
    }
}

/// Property source for global creation state (bonus points, etc.).
struct CreationSource {
    bonus_points: u8,
}

impl PropertySource for CreationSource {
    fn source_name(&self) -> &str {
        "creation"
    }

    fn resolve(&self, path: &str) -> Option<String> {
        match path {
            "bonus_points" => Some(self.bonus_points.to_string()),
            _ => None,
        }
    }
}

/// System: sync party member data into the PropertyRegistry when Party or creation state changes.
pub fn update_party_creation_registry(
    party: Res<Party>,
    creation_state: Res<PartyCreationState>,
    mut registry: ResMut<crate::screens::PropertyRegistry>,
) {
    if !party.is_changed() && !creation_state.is_changed() {
        return;
    }
    for (i, member) in party.members.iter().enumerate() {
        registry.register(Box::new(PartyMemberSource {
            index: i,
            class_base_attrs: creation::class_base_attrs(member.class),
            member: member.clone(),
            selected_stat: creation_state.selected_stat[i],
            chosen_skills: creation_state.chosen_skills[i],
        }));
    }
    // Register "active" as alias for the currently selected member.
    let active = creation_state.active_member;
    let active_member = &party.members[active];
    registry.register(Box::new(PartyMemberSource {
        index: ACTIVE_SOURCE_INDEX,
        class_base_attrs: creation::class_base_attrs(active_member.class),
        member: active_member.clone(),
        selected_stat: creation_state.selected_stat[active],
        chosen_skills: creation_state.chosen_skills[active],
    }));
    // Register global creation state.
    registry.register(Box::new(CreationSource {
        bonus_points: creation_state.bonus_points,
    }));
}

/// Move the stat selector arrows to the selected stat row for the active member.
pub fn sync_creation_arrows(creation_state: Res<PartyCreationState>, mut query: Query<(&RuntimeElement, &mut Node)>) {
    if !creation_state.is_changed() {
        return;
    }
    let active = creation_state.active_member;
    let y = match creation_state.selected_stat[active] {
        CharStat::Might => 154.0,
        CharStat::Intellect => 171.0,
        CharStat::Personality => 188.0,
        CharStat::Endurance => 205.0,
        CharStat::Accuracy => 222.0,
        CharStat::Speed => 239.0,
        CharStat::Luck => 256.0,
    };

    const CHAR_COL_WIDTH: f32 = 158.0;
    for (elem, mut node) in &mut query {
        if elem.element_id == "icons/ARROWL0" {
            let xorig = 11.0;
            node.top = Val::Percent(y / crate::screens::REF_H * 100.0);
            node.left = Val::Percent((xorig + active as f32 * CHAR_COL_WIDTH) / crate::screens::REF_W * 100.0);
        } else if elem.element_id == "icons/ARROWR0" {
            let xorig = 135.0;
            node.top = Val::Percent(y / crate::screens::REF_H * 100.0);
            node.left = Val::Percent((xorig + active as f32 * CHAR_COL_WIDTH) / crate::screens::REF_W * 100.0);
        }
    }
}

// ── Party mutation helpers (called from interaction.rs action handlers) ──────

/// Cycle portrait for a party member. Updates name to match new gender.
pub fn cycle_member_portrait(party: &mut Party, member_idx: usize, delta: i32) {
    let all = PortraitId::ALL;
    let current = all
        .iter()
        .position(|&p| p == party.members[member_idx].portrait)
        .unwrap_or(0);
    let next = (current as i32 + delta).rem_euclid(all.len() as i32) as usize;
    party.members[member_idx].portrait = all[next];
    party.members[member_idx].name = creation::random_name(all[next].is_male()).to_string();
}

/// Set the class for a party member (resets base attrs to class defaults).
/// Returns the bonus points that should be refunded (caller updates PartyCreationState).
pub fn set_member_class(party: &mut Party, member_idx: usize, class: CharacterClass) {
    party.members[member_idx].class = class;
    party.members[member_idx].base_attrs = creation::class_base_attrs(class);
}

/// Maximum value a stat can reach during character creation.
pub const STAT_MAX: i16 = 25;
/// Maximum points a stat can be reduced below its class base.
pub const STAT_MIN_DEFICIT: i16 = 2;

/// Determine the color for a stat value relative to its class base.
/// Green = above base, red = below base, white = at base.
pub fn stat_value_color(current: i16, class_base: i16) -> &'static str {
    if current > class_base {
        "green"
    } else if current < class_base {
        "red"
    } else {
        "white"
    }
}

// ── Screen-specific action handler ───��──────────────────────────────────────

fn parse_member_index(object: &str) -> Option<usize> {
    match object {
        "member0" => Some(0),
        "member1" => Some(1),
        "member2" => Some(2),
        "member3" => Some(3),
        _ => None,
    }
}

/// Handle screen actions specific to the party creation screen.
/// Only active when `PartyCreationState` exists (i.e. during character creation).
pub fn handle_creation_actions(
    mut commands: Commands,
    mut events: MessageReader<ScreenActionEvent>,
    mut creation_state: Option<ResMut<PartyCreationState>>,
    mut party: Option<ResMut<Party>>,
    sound_manager: Option<Res<crate::game::sound::SoundManager>>,
    mut sound_writer: Option<bevy::ecs::message::MessageWriter<crate::game::sound::effects::PlayUiSoundEvent>>,
) {
    use crate::screens::scripting::{parse_string_arg, parse_string_int_args, parse_two_string_args};

    let Some(ref mut cs) = creation_state else {
        return;
    };

    for ScreenActionEvent(action) in events.read() {
        let s = action.trim();

        // CyclePortrait("member0", 1)
        if let Some((object, delta)) = parse_string_int_args(s, "CyclePortrait") {
            if let Some(index) = parse_member_index(object) {
                if let Some(ref mut p) = party {
                    cycle_member_portrait(p, index, delta);
                    play_portrait_sound(p, index, &sound_manager, &mut sound_writer);
                }
            } else {
                warn!("CyclePortrait: unknown object '{}'", object);
            }
            continue;
        }

        // SelectClass("Knight")
        if let Some(class_name) = parse_string_arg(s, "SelectClass") {
            if let Some(class) = CharacterClass::from_name(class_name) {
                let index = cs.active_member;
                if let Some(ref mut p) = party {
                    // Refund any points spent on the old class before switching.
                    let old_base = creation::class_base_attrs(p.members[index].class);
                    let spent: i16 = (0..7)
                        .map(|i| p.members[index].base_attrs[i] - old_base[i])
                        .filter(|d| *d > 0)
                        .sum();
                    cs.bonus_points = cs.bonus_points.saturating_add(spent as u8);
                    set_member_class(p, index, class);
                    // Clear chosen optional skills when switching class.
                    cs.chosen_skills[index] = [None; 2];
                }
            } else {
                warn!("SelectClass: unknown class '{}'", class_name);
            }
            continue;
        }

        // SelectStat("might", 0) -> specify member and stat
        if let Some((stat_name, member_idx)) = parse_string_int_args(s, "SelectStat") {
            if let Some(stat) = CharStat::from_name(stat_name) {
                let active = member_idx as usize;
                // Shift focus to the character clicked
                cs.active_member = active;
                cs.selected_stat[active] = stat;
            } else {
                warn!("SelectStat: unknown stat '{}'", stat_name);
            }
            continue;
        }

        // Backward compatibility: SelectStat("might")
        if let Some(stat_name) = parse_string_arg(s, "SelectStat") {
            if let Some(stat) = CharStat::from_name(stat_name) {
                let active = cs.active_member;
                cs.selected_stat[active] = stat;
            } else {
                warn!("SelectStat: unknown stat '{}'", stat_name);
            }
            continue;
        }

        // SelectMember("member0")
        if let Some(object) = parse_string_arg(s, "SelectMember") {
            if let Some(index) = parse_member_index(object) {
                cs.active_member = index;
            }
            continue;
        }

        // IncrementStat()
        if s == "IncrementStat()" {
            let index = cs.active_member;
            let stat = cs.selected_stat[index];
            if cs.bonus_points > 0 {
                let idx = stat.attr_index();
                if let Some(ref mut p) = party
                    && p.members[index].base_attrs[idx] < STAT_MAX
                {
                    p.members[index].base_attrs[idx] += 1;
                    cs.bonus_points -= 1;
                }
            }
            continue;
        }

        // DecrementStat()
        if s == "DecrementStat()" {
            let index = cs.active_member;
            let stat = cs.selected_stat[index];
            let idx = stat.attr_index();
            if let Some(ref mut p) = party {
                let current = p.members[index].base_attrs[idx];
                let class_base = creation::class_base_attrs(p.members[index].class)[idx];
                if current > class_base - STAT_MIN_DEFICIT {
                    p.members[index].base_attrs[idx] -= 1;
                    cs.bonus_points += 1;
                }
            }
            continue;
        }

        // SelectAvailableSkill("3") — pick from available pool
        if let Some(idx_str) = parse_string_arg(s, "SelectAvailableSkill") {
            if let Ok(skill_idx) = idx_str.parse::<usize>() {
                let member = cs.active_member;
                let avail = creation::class_available_skills(
                    party.as_ref().map(|p| p.members[member].class).unwrap_or_default(),
                );
                // Only allow if index is valid and not already chosen.
                if skill_idx < avail.len() && !cs.chosen_skills[member].iter().any(|c| *c == Some(skill_idx)) {
                    // Fill first empty slot.
                    if cs.chosen_skills[member][0].is_none() {
                        cs.chosen_skills[member][0] = Some(skill_idx);
                    } else if cs.chosen_skills[member][1].is_none() {
                        cs.chosen_skills[member][1] = Some(skill_idx);
                    }
                    // else: both slots full, ignore click
                }
            }
            continue;
        }

        // RemoveChosenSkill("member0", 2) — remove from slot and focus that member
        if let Some((member_str, slot)) = parse_string_int_args(s, "RemoveChosenSkill") {
            if let Some(member) = parse_member_index(member_str) {
                cs.active_member = member;
                // Slot 2 maps to chosen_skills[0], slot 3 to chosen_skills[1]
                if slot >= 2 && slot <= 3 {
                    cs.chosen_skills[member][slot as usize - 2] = None;
                }
            }
            continue;
        }

        // ConfirmCreation() — proceed to game only if all points & skills allocated
        if s == "ConfirmCreation()" {
            if cs.bonus_points > 0 {
                warn!("ConfirmCreation blocked: {} bonus points unspent", cs.bonus_points);
                continue;
            }
            let all_skills_picked = cs.chosen_skills.iter().all(|slots| slots.iter().all(|s| s.is_some()));
            if !all_skills_picked {
                warn!("ConfirmCreation blocked: not all members have 2 chosen skills");
                continue;
            }
            info!("ConfirmCreation: all points and skills allocated, starting game");
            commands.set_state(crate::GameState::Loading);
            continue;
        }

        // Unrecognized action for this screen — not an error, other screens may handle it.
        debug!("creation: unhandled action '{}'", s);
    }
}

/// Play a random PickMe voice line for the given party member's portrait.
fn play_portrait_sound(
    party: &Party,
    index: usize,
    sound_manager: &Option<Res<crate::game::sound::SoundManager>>,
    sound_writer: &mut Option<bevy::ecs::message::MessageWriter<crate::game::sound::effects::PlayUiSoundEvent>>,
) {
    use crate::game::player::party::portrait::Speech;

    let portrait = party.members[index].portrait;
    let Some(sm) = sound_manager.as_deref() else { return };
    let variants = portrait.available_variants(Speech::PickMe, sm);
    if variants.is_empty() {
        return;
    }
    let mut rng = creation::SplitMix64::seeded();
    let pick = rng.index(variants.len());
    let sound_name = portrait.voice_name(Speech::PickMe, variants[pick]);
    if let Some(entry) = sm.dsounds.get_by_name(&sound_name)
        && let Some(w) = sound_writer
    {
        w.write(crate::game::sound::effects::PlayUiSoundEvent {
            sound_id: entry.sound_id,
        });
    }
}
