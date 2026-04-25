//! Centralized state population from a loaded save file.

use super::ActiveSave;
use crate::game::player::party::Party;
use crate::game::player::party::member::{ATTR_COUNT, Class, PartyMember, RESIST_COUNT, SKILL_COUNT};
use crate::game::player::party::portrait::PortraitId;
use crate::game::state::state::WorldState;
use crate::game::state::time::GameTime;

/// Populate all live game state from an ActiveSave.
///
/// Syncs position, map, gold/food, quest bits, autonotes, calendar, and
/// party members. Called once after loading a save file.
pub fn populate_state_from_save(
    save: &ActiveSave,
    world_state: &mut WorldState,
    party: &mut Party,
    game_time: &mut GameTime,
) {
    let sp = &save.party;

    // ── Position ────────────────────────────────────────────────────────
    world_state.player.position = save.spawn_position;
    world_state.player.yaw = save.spawn_yaw;

    // ── Map ─────────────────────────────────────────────────────────────
    world_state.map.name = save.map_name.clone();
    if let openmm_data::utils::MapName::Outdoor(ref odm) = save.map_name {
        let s = odm.to_string();
        // ODM names are like "oute3" — last two chars are column + row.
        let chars: Vec<char> = s.chars().collect();
        if chars.len() >= 2 {
            world_state.map.map_x = chars[chars.len() - 2];
            world_state.map.map_y = chars[chars.len() - 1];
        }
    }

    // ── Gold, food, reputation ──────────────────────────────────────────
    world_state.game_vars.gold = sp.gold;
    world_state.game_vars.food = sp.food;
    world_state.game_vars.reputation = sp.reputation;

    // ── Quest bits ──────────────────────────────────────────────────────
    world_state.game_vars.quest_bits.clear();
    for &bit in &sp.quest_bits {
        world_state.game_vars.quest_bits.insert(bit);
    }

    // ── Autonotes ───────────────────────────────────────────────────────
    world_state.game_vars.autonotes.clear();
    for &note in &sp.autonote_bits {
        world_state.game_vars.autonotes.insert(note);
    }

    // ── Calendar -> GameTime ────────────────────────────────────────────
    *game_time = GameTime::from_calendar(
        sp.year as u32,
        sp.month as u32,
        sp.day as u32,
        sp.hour as u32,
        sp.minute as u32,
    );

    // ── Characters -> Party members ─────────────────────────────────────
    for (i, sc) in sp.characters.iter().enumerate() {
        let class = Class::from_id(sc.class).unwrap_or_default();

        // Map face index to PortraitId (face 0-11 maps to ALL_ARR order).
        let portrait = PortraitId::ALL
            .get(sc.face as usize)
            .copied()
            .unwrap_or(PortraitId::MaleA);

        let mut member = PartyMember::new(sc.name.clone(), class, portrait, sc.level as u8);

        member.hp = sc.hp as i16;
        member.sp = sc.sp as i16;
        member.experience = sc.experience;
        member.skill_points = sc.skill_points;

        // Base attributes (both have 7 entries: Might..Luck).
        let attr_len = sc.base_stats.len().min(ATTR_COUNT);
        member.base_attrs[..attr_len].copy_from_slice(&sc.base_stats[..attr_len]);
        member.attr_bonuses[..attr_len].copy_from_slice(&sc.stat_bonuses[..attr_len]);

        // Skills (both have 31 entries).
        let skill_len = sc.skills.len().min(SKILL_COUNT);
        member.skills[..skill_len].copy_from_slice(&sc.skills[..skill_len]);

        // Resistances (both have 5 entries: Fire, Elec, Cold, Poison, Magic).
        let res_len = sc.resistances.len().min(RESIST_COUNT);
        member.resistances[..res_len].copy_from_slice(&sc.resistances[..res_len]);
        member.resistance_bonuses[..res_len].copy_from_slice(&sc.resistance_bonuses[..res_len]);

        party.members[i] = member;
    }
}
