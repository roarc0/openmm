pub mod creation;
pub mod member;
pub mod portrait;

use bevy::prelude::*;
use openmm_data::enums::{EvtTargetCharacter, EvtVariable};

use member::PartyMember;

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
            EvtTargetCharacter::Active => vec![0], // first member as stand-in
            EvtTargetCharacter::Party => vec![0, 1, 2, 3],
            EvtTargetCharacter::Random => vec![0], // deterministic fallback
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

    /// Read a per-character EvtVariable from the targeted member(s).
    /// For multi-target (Party), returns the value for member 0 (representative).
    pub fn get_member_var(&self, target: EvtTargetCharacter, var: EvtVariable) -> i32 {
        let idx = self.target_indices(target);
        idx.first().map(|&i| self.members[i].get_var(var)).unwrap_or(0)
    }

    /// Write a per-character EvtVariable to the targeted member(s).
    pub fn set_member_var(&mut self, target: EvtTargetCharacter, var: EvtVariable, value: i32) {
        for i in self.target_indices(target) {
            self.members[i].set_var(var, value);
        }
    }

    /// Add delta to a per-character EvtVariable for the targeted member(s).
    pub fn add_member_var(&mut self, target: EvtTargetCharacter, var: EvtVariable, delta: i32) {
        for i in self.target_indices(target) {
            self.members[i].add_var(var, delta);
        }
    }
}

impl Default for Party {
    fn default() -> Self {
        // Randomized starting party for character creation screen.
        let seeds = creation::random_unique_party_creation_seeds();
        let members = std::array::from_fn(|i| {
            let mut m = PartyMember::new(seeds[i].name, seeds[i].class, seeds[i].portrait, 1);
            m.base_attrs = seeds[i].base_attrs;
            m
        });

        Self {
            members,
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
