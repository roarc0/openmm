//! Party creation UI state — controls the "makeme" character creation screen.
//!
//! The actual character data lives in [`Party`] — this module only holds
//! transient UI state: which member is selected, which stat is highlighted,
//! and how many bonus points remain to allocate.
//!
//! Exposes property bindings `member0`–`char3` via [`PropertySource`] so RON
//! screens can display portrait, class, stats, etc.

use bevy::prelude::*;

use crate::game::player::party::Party;
use crate::game::player::party::creation;
use crate::game::player::party::member::{CharacterClass, PartyMember};
use crate::game::player::party::portrait::PortraitId;
use crate::screens::PropertySource;
use crate::screens::runtime::RuntimeElement;

/// All selectable primary stats for point distribution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CharStat {
    #[default]
    Might,
    Intellect,
    Personality,
    Endurance,
    Accuracy,
    Speed,
    Luck,
}

impl CharStat {
    pub fn name(self) -> &'static str {
        match self {
            Self::Might => "might",
            Self::Intellect => "intellect",
            Self::Personality => "personality",
            Self::Endurance => "endurance",
            Self::Accuracy => "accuracy",
            Self::Speed => "speed",
            Self::Luck => "luck",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "might" => Some(Self::Might),
            "intellect" => Some(Self::Intellect),
            "personality" => Some(Self::Personality),
            "endurance" => Some(Self::Endurance),
            "accuracy" => Some(Self::Accuracy),
            "speed" => Some(Self::Speed),
            "luck" => Some(Self::Luck),
            _ => None,
        }
    }

    /// Index into PartyMember.base_attrs array.
    pub fn attr_index(self) -> usize {
        match self {
            Self::Might => 0,
            Self::Intellect => 1,
            Self::Personality => 2,
            Self::Endurance => 3,
            Self::Speed => 4,
            Self::Accuracy => 5,
            Self::Luck => 6,
        }
    }
}

/// Transient UI state for party creation. Does NOT hold character data — that's in `Party`.
#[derive(Resource)]
pub struct PartyCreationState {
    /// Which party member (0–3) is currently focused for stat allocation.
    pub active_member: usize,
    /// Currently highlighted stat for the active member.
    pub selected_stat: [CharStat; 4],
    /// Remaining bonus points per member.
    pub bonus_points: [u8; 4],
}

impl Default for PartyCreationState {
    fn default() -> Self {
        Self {
            active_member: 0,
            selected_stat: [CharStat::default(); 4],
            bonus_points: [25; 4], // MM6 gives 25 bonus points per character
        }
    }
}

// ── PropertySource: expose Party data to RON screens ────────────────────────

/// Bridges a single party member's data to the screen property system.
struct PartyMemberSource {
    index: usize,
    member: PartyMember,
    selected_stat: CharStat,
    class_base_attrs: [i16; 7],
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
            _ => "member3",
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
        }));
    }
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

    for (elem, mut node) in &mut query {
        if elem.element_id == "icons/ARROWL0" || elem.element_id == "icons/ARROWR0" {
            node.top = Val::Percent(y / crate::screens::REF_H * 100.0);
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
