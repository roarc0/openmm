pub mod member;

use bevy::prelude::*;
use lod::enums::{EvtTargetCharacter, EvtVariable};

use member::{CharacterClass, PartyMember};

/// The active party: exactly 4 members (indices 0–3 match EvtTargetCharacter::Player1–4).
#[derive(Resource)]
pub struct Party {
    pub members: [PartyMember; 4],
    /// The character target set by the most recent ForPartyMember EVT opcode.
    pub active_target: EvtTargetCharacter,
}

impl Party {
    /// Resolve a target to a list of member indices.
    fn target_indices(&self, target: EvtTargetCharacter) -> Vec<usize> {
        match target {
            EvtTargetCharacter::Player1 => vec![0],
            EvtTargetCharacter::Player2 => vec![1],
            EvtTargetCharacter::Player3 => vec![2],
            EvtTargetCharacter::Player4 => vec![3],
            EvtTargetCharacter::Active  => vec![0], // first member as stand-in
            EvtTargetCharacter::Party   => vec![0, 1, 2, 3],
            EvtTargetCharacter::Random  => vec![0], // deterministic fallback
        }
    }

    /// Returns the highest skill level for `var` across all members matching `target`.
    /// Returns 0 if the variable is not a skill or no members match.
    pub fn max_skill(&self, target: EvtTargetCharacter, var: EvtVariable) -> u8 {
        self.target_indices(target)
            .iter()
            .map(|&i| self.members[i].get_skill(var))
            .max()
            .unwrap_or(0)
    }
}

impl Default for Party {
    fn default() -> Self {
        // Mock MM6 starting party
        let mut zoltan   = PartyMember::new("Zoltan",   CharacterClass::Knight,  1);
        let mut roderick = PartyMember::new("Roderick", CharacterClass::Paladin, 1);
        let mut alexei   = PartyMember::new("Alexei",   CharacterClass::Archer,  1);
        let mut serena   = PartyMember::new("Serena",   CharacterClass::Cleric,  1);

        zoltan.set_skill(EvtVariable::SKILL_SWORD,         1);
        zoltan.set_skill(EvtVariable::SKILL_SHIELD,        1);
        zoltan.set_skill(EvtVariable::SKILL_LEATHER,       1);

        roderick.set_skill(EvtVariable::SKILL_SWORD,       1);
        roderick.set_skill(EvtVariable::SKILL_CHAIN,       1);
        roderick.set_skill(EvtVariable::SKILL_SPIRIT_MAGIC, 1);

        alexei.set_skill(EvtVariable::SKILL_BOW,           1);
        alexei.set_skill(EvtVariable::SKILL_LEATHER,       1);
        alexei.set_skill(EvtVariable::SKILL_AIR_MAGIC,     1);

        serena.set_skill(EvtVariable::SKILL_MACE,          1);
        serena.set_skill(EvtVariable::SKILL_CHAIN,         1);
        serena.set_skill(EvtVariable::SKILL_SPIRIT_MAGIC,  1);

        Self {
            members: [zoltan, roderick, alexei, serena],
            active_target: EvtTargetCharacter::Player1,
        }
    }
}

pub struct PartyPlugin;

impl Plugin for PartyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Party>();
    }
}
