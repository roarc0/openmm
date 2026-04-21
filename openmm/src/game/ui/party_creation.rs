//! Party creation UI state — controls the "makeme" character creation screen.
//!
//! The actual character data lives in [`Party`] — this module only holds
//! transient UI state: which member is selected, which attribute is highlighted,
//! and how many bonus points remain to allocate.
//!
//! Exposes property bindings `member0`–`char3` via [`PropertySource`] so RON
//! screens can display portrait, class, attributes, etc.

use bevy::ecs::message::MessageReader;
use bevy::prelude::*;

use crate::game::player::party::Party;
use crate::game::player::party::creation;
use crate::game::player::party::member::{Attribute, Class, PartyMember};
use crate::game::player::party::portrait::PortraitId;
use crate::screens::PropertySource;
use crate::screens::runtime::RuntimeElement;
use crate::screens::runtime::ScreenActionEvent;

/// Transient UI state for party creation. Does NOT hold character data — that's in `Party`.
#[derive(Resource)]
pub struct PartyCreationState {
    /// Which party member (0–3) is currently focused for attribute allocation.
    pub active_member: usize,
    /// Currently highlighted attribute for the active member.
    pub selected_attribute: [Attribute; 4],
    /// Remaining bonus points shared across all members.
    pub bonus_points: u8,
    /// Chosen optional skills per member (index into class_available_skills, or None).
    pub chosen_skills: [[Option<usize>; 2]; 4],
}

impl Default for PartyCreationState {
    fn default() -> Self {
        Self {
            active_member: 0,
            selected_attribute: [Attribute::default(); 4],
            bonus_points: 52,
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
    selected_attribute: Attribute,
    class_base_attrs: [i16; 7],
    /// Which optional skill slots have been filled (indices into class_available_skills).
    chosen_skills: [Option<usize>; 2],
}

impl PartyMemberSource {
    /// Color for a attribute label: green if above base, red if below, white if equal.
    fn attribute_label_color(&self, attribute: Attribute) -> &'static str {
        let idx = attribute.attr_index();
        attribute_value_color(self.member.base_attrs[idx], self.class_base_attrs[idx])
    }

    /// Color for a attribute value: green if above base, red if below, white if equal.
    fn attribute_value_color(&self, attribute: Attribute) -> &'static str {
        let idx = attribute.attr_index();
        attribute_value_color(self.member.base_attrs[idx], self.class_base_attrs[idx])
    }

    fn selected_class_color(&self, class: Class) -> &'static str {
        if self.member.class.base_class() == class.base_class() {
            "cyan"
        } else {
            "white"
        }
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
            "class_color_knight" => Some(self.selected_class_color(Class::Knight(0)).to_string()),
            "class_color_paladin" => Some(self.selected_class_color(Class::Paladin(0)).to_string()),
            "class_color_archer" => Some(self.selected_class_color(Class::Archer(0)).to_string()),
            "class_color_cleric" => Some(self.selected_class_color(Class::Cleric(0)).to_string()),
            "class_color_druid" => Some(self.selected_class_color(Class::Druid(0)).to_string()),
            "class_color_sorcerer" => Some(self.selected_class_color(Class::Sorcerer(0)).to_string()),
            "gender" => Some(if m.portrait.is_male() { "male" } else { "female" }.to_string()),
            "selected_attribute" => Some(self.selected_attribute.name().to_string()),
            "attribute_color_might" => Some(self.attribute_label_color(Attribute::Might).to_string()),
            "attribute_color_intellect" => Some(self.attribute_label_color(Attribute::Intellect).to_string()),
            "attribute_color_personality" => Some(self.attribute_label_color(Attribute::Personality).to_string()),
            "attribute_color_endurance" => Some(self.attribute_label_color(Attribute::Endurance).to_string()),
            "attribute_color_accuracy" => Some(self.attribute_label_color(Attribute::Accuracy).to_string()),
            "attribute_color_speed" => Some(self.attribute_label_color(Attribute::Speed).to_string()),
            "attribute_color_luck" => Some(self.attribute_label_color(Attribute::Luck).to_string()),
            "val_color_might" => Some(self.attribute_value_color(Attribute::Might).to_string()),
            "val_color_intellect" => Some(self.attribute_value_color(Attribute::Intellect).to_string()),
            "val_color_personality" => Some(self.attribute_value_color(Attribute::Personality).to_string()),
            "val_color_endurance" => Some(self.attribute_value_color(Attribute::Endurance).to_string()),
            "val_color_accuracy" => Some(self.attribute_value_color(Attribute::Accuracy).to_string()),
            "val_color_speed" => Some(self.attribute_value_color(Attribute::Speed).to_string()),
            "val_color_luck" => Some(self.attribute_value_color(Attribute::Luck).to_string()),
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
                Some(
                    skills
                        .get(idx)
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "None".to_string()),
                )
            }
            // Chosen optional skills (slots 2-3)
            "skill_2" | "skill_3" => {
                let slot: usize = path[6..].parse::<usize>().ok()? - 2;
                let avail = creation::class_available_skills(m.class);
                match self.chosen_skills[slot] {
                    Some(ai) => Some(
                        avail
                            .get(ai)
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| "None".to_string()),
                    ),
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
                Some(avail.get(idx).map(|s| s.to_string()).unwrap_or_default())
            }
            // Color for available skill: cyan if already chosen, white otherwise
            "av_skill_0_color" | "av_skill_1_color" | "av_skill_2_color" | "av_skill_3_color" | "av_skill_4_color"
            | "av_skill_5_color" | "av_skill_6_color" | "av_skill_7_color" | "av_skill_8_color" => {
                let idx: usize = path[9..path.len() - 6].parse().ok()?;
                let is_chosen = self.chosen_skills.contains(&Some(idx));
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
            selected_attribute: creation_state.selected_attribute[i],
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
        selected_attribute: creation_state.selected_attribute[active],
        chosen_skills: creation_state.chosen_skills[active],
    }));
    // Register global creation state.
    registry.register(Box::new(CreationSource {
        bonus_points: creation_state.bonus_points,
    }));
}

/// Move the attribute selector arrows to the selected attribute row for the active member.
pub fn sync_creation_arrows(creation_state: Res<PartyCreationState>, mut query: Query<(&RuntimeElement, &mut Node)>) {
    if !creation_state.is_changed() {
        return;
    }
    let active = creation_state.active_member;
    let y = match creation_state.selected_attribute[active] {
        Attribute::Might => 154.0,
        Attribute::Intellect => 171.0,
        Attribute::Personality => 188.0,
        Attribute::Endurance => 205.0,
        Attribute::Accuracy => 222.0,
        Attribute::Speed => 239.0,
        Attribute::Luck => 256.0,
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
pub fn set_member_class(party: &mut Party, member_idx: usize, class: Class) {
    party.members[member_idx].class = class;
    party.members[member_idx].base_attrs = creation::class_base_attrs(class);
}

/// Maximum value a attribute can reach during character creation.
pub const STAT_MAX: i16 = 25;
/// Maximum points a attribute can be reduced below its class base.
pub const STAT_MIN_DEFICIT: i16 = 2;

/// Determine the color for a attribute value relative to its class base.
/// Green = above base, red = below base, white = at base.
pub fn attribute_value_color(current: i16, class_base: i16) -> &'static str {
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
    keys: Res<ButtonInput<KeyCode>>,
) {
    use crate::screens::scripting::{parse_string_arg, parse_string_int_args};

    let Some(ref mut cs) = creation_state else {
        return;
    };

    // Handle keyboard navigation
    if keys.just_pressed(KeyCode::ArrowLeft) {
        cs.active_member = (cs.active_member + 3) % 4;
    }
    if keys.just_pressed(KeyCode::ArrowRight) {
        cs.active_member = (cs.active_member + 1) % 4;
    }
    if keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::ArrowDown) {
        let delta = if keys.just_pressed(KeyCode::ArrowUp) { -1 } else { 1 };
        let order = [
            Attribute::Might,
            Attribute::Intellect,
            Attribute::Personality,
            Attribute::Endurance,
            Attribute::Accuracy,
            Attribute::Speed,
            Attribute::Luck,
        ];
        let active = cs.active_member;
        let p = order
            .iter()
            .position(|&s| s == cs.selected_attribute[active])
            .unwrap_or(0);
        let next_pos = (p as isize + delta).rem_euclid(order.len() as isize) as usize;
        cs.selected_attribute[active] = order[next_pos];
    }

    let do_increment_attribute = |cs: &mut PartyCreationState, p: &mut Party| {
        let index = cs.active_member;
        let attribute = cs.selected_attribute[index];
        if cs.bonus_points > 0 {
            let idx = attribute.attr_index();
            if p.members[index].base_attrs[idx] < STAT_MAX {
                p.members[index].base_attrs[idx] += 1;
                cs.bonus_points -= 1;
            }
        }
    };

    let do_decrement_attribute = |cs: &mut PartyCreationState, p: &mut Party| {
        let index = cs.active_member;
        let attribute = cs.selected_attribute[index];
        let idx = attribute.attr_index();
        let class_base = creation::class_base_attrs(p.members[index].class)[idx];
        if p.members[index].base_attrs[idx] > class_base - STAT_MIN_DEFICIT {
            p.members[index].base_attrs[idx] -= 1;
            cs.bonus_points += 1;
        }
    };

    if (keys.just_pressed(KeyCode::NumpadAdd) || keys.just_pressed(KeyCode::Equal))
        && let Some(ref mut p) = party
    {
        do_increment_attribute(cs, p);
    }
    if (keys.just_pressed(KeyCode::NumpadSubtract) || keys.just_pressed(KeyCode::Minus))
        && let Some(ref mut p) = party
    {
        do_decrement_attribute(cs, p);
    }

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
            if let Some(class) = Class::from_name(class_name) {
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

        // SelectAttribute("might", 0) -> specify member and attribute
        if let Some((attribute_name, member_idx)) = parse_string_int_args(s, "SelectAttribute") {
            if let Some(attribute) = Attribute::from_name(attribute_name) {
                let active = member_idx as usize;
                // Shift focus to the character clicked
                cs.active_member = active;
                cs.selected_attribute[active] = attribute;
            } else {
                warn!("SelectAttribute: unknown attribute '{}'", attribute_name);
            }
            continue;
        }

        // Backward compatibility: SelectAttribute("might")
        if let Some(attribute_name) = parse_string_arg(s, "SelectAttribute") {
            if let Some(attribute) = Attribute::from_name(attribute_name) {
                let active = cs.active_member;
                cs.selected_attribute[active] = attribute;
            } else {
                warn!("SelectAttribute: unknown attribute '{}'", attribute_name);
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

        // IncrementAttribute()
        if s == "IncrementAttribute()" {
            if let Some(ref mut p) = party {
                do_increment_attribute(cs, p);
            }
            continue;
        }

        // DecrementAttribute()
        if s == "DecrementAttribute()" {
            if let Some(ref mut p) = party {
                do_decrement_attribute(cs, p);
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
                if skill_idx < avail.len() && !cs.chosen_skills[member].contains(&Some(skill_idx)) {
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
                if (2..=3).contains(&slot) {
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
