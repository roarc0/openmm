//! Pure game variable operations — read/write `GameVariables` and `Party`
//! without touching Bevy `Commands`, assets, or rendering state.
//
// These functions read/write GameVariables and Party without touching
// Bevy Commands, Assets, or any rendering state.

use bevy::prelude::*;
use openmm_data::enums::EvtVariable;

use crate::game::player::party::Party;

use super::state::GameVariables;

/// Read a game variable's current value.
pub(in crate::game) fn get_variable(
    vars: &GameVariables,
    party: &Party,
    game_time: Option<&super::time::GameTime>,
    var: EvtVariable,
) -> i32 {
    if var.is_map_var() {
        return vars.map_vars[var.map_var_index().unwrap() as usize];
    }
    match var {
        EvtVariable::GOLD => vars.gold,
        EvtVariable::FOOD => vars.food,
        EvtVariable::REPUTATION_IS => vars.reputation,
        EvtVariable::QBITS => 0,          // compare uses contains check, handled separately
        EvtVariable::AUTONOTES_BITS => 0, // compare uses contains check
        EvtVariable::FLYING => vars.flying as i32,
        EvtVariable::NPCS => vars.npcs_in_party,
        EvtVariable::TOTAL_CIRCUS_PRIZE => vars.total_circus_prize,
        EvtVariable::SKILL_POINTS => party.get_member_var(party.active_target, var),
        EvtVariable::DAYS_COUNTER1 => vars.days_counters[0],
        EvtVariable::DAYS_COUNTER2 => vars.days_counters[1],
        EvtVariable::DAYS_COUNTER3 => vars.days_counters[2],
        EvtVariable::DAYS_COUNTER4 => vars.days_counters[3],
        EvtVariable::DAYS_COUNTER5 => vars.days_counters[4],
        EvtVariable::DAYS_COUNTER6 => vars.days_counters[5],
        EvtVariable::MONTH_IS => game_time.map(|t| t.calendar_date().1 as i32).unwrap_or(1),
        EvtVariable::HOUR_IS => game_time.map(|t| t.hour() as i32).unwrap_or(9),
        EvtVariable::DAY_OF_WEEK_IS => game_time.map(|t| t.day_of_week() as i32).unwrap_or(0),
        EvtVariable::DAY_OF_YEAR_IS => {
            game_time
                .map(|t| {
                    let (_, m, d) = t.calendar_date();
                    ((m - 1) * 28 + d) as i32 // MM6 uses 28-day months
                })
                .unwrap_or(1)
        }
        _ => {
            // Per-character variables (attrs, skills, conditions, etc.)
            let pv = party.get_member_var(party.active_target, var);
            if pv != 0 || is_character_var(var) {
                return pv;
            }
            debug!(
                "get_variable: unhandled variable {} (0x{:02x}), returning 0",
                var, var.0
            );
            0
        }
    }
}

/// Returns true if this variable is per-character (not global).
pub(in crate::game) fn is_character_var(var: EvtVariable) -> bool {
    var.is_character_scoped()
}

/// Write a value to a game variable.
pub(in crate::game) fn set_variable(vars: &mut GameVariables, party: &mut Party, var: EvtVariable, value: i32) {
    if var.is_map_var() {
        let idx = var.map_var_index().unwrap() as usize;
        info!("  {} = {} (was {})", var, value, vars.map_vars[idx]);
        vars.map_vars[idx] = value;
        return;
    }
    match var {
        EvtVariable::GOLD => {
            info!("  Gold = {} (was {})", value, vars.gold);
            vars.gold = value;
        }
        EvtVariable::FOOD => {
            info!("  Food = {} (was {})", value, vars.food);
            vars.food = value;
        }
        EvtVariable::REPUTATION_IS => {
            info!("  Reputation = {} (was {})", value, vars.reputation);
            vars.reputation = value;
        }
        EvtVariable::QBITS => {
            if value != 0 {
                vars.set_qbit(value);
            }
        }
        EvtVariable::AUTONOTES_BITS => {
            if value != 0 {
                vars.add_autonote(value);
            }
        }
        EvtVariable::FLYING => vars.flying = value != 0,
        EvtVariable::NPCS => vars.npcs_in_party = value,
        EvtVariable::TOTAL_CIRCUS_PRIZE => vars.total_circus_prize = value,
        EvtVariable::DAYS_COUNTER1 => vars.days_counters[0] = value,
        EvtVariable::DAYS_COUNTER2 => vars.days_counters[1] = value,
        EvtVariable::DAYS_COUNTER3 => vars.days_counters[2] = value,
        EvtVariable::DAYS_COUNTER4 => vars.days_counters[3] = value,
        EvtVariable::DAYS_COUNTER5 => vars.days_counters[4] = value,
        EvtVariable::DAYS_COUNTER6 => vars.days_counters[5] = value,
        _ => {
            if is_character_var(var) {
                let target = party.active_target;
                party.set_member_var(target, var, value);
            } else {
                warn!(
                    "  set_variable: unhandled variable {} (0x{:02x}) = {}",
                    var, var.0, value
                );
            }
        }
    }
}

/// Add to a game variable.
pub(in crate::game) fn add_variable(vars: &mut GameVariables, party: &mut Party, var: EvtVariable, value: i32) {
    if var.is_map_var() {
        let idx = var.map_var_index().unwrap() as usize;
        let old = vars.map_vars[idx];
        vars.map_vars[idx] = old.wrapping_add(value);
        info!("  {} += {} ({} -> {})", var, value, old, vars.map_vars[idx]);
        return;
    }
    match var {
        EvtVariable::GOLD => {
            let old = vars.gold;
            vars.gold += value;
            info!("  Gold += {} ({} -> {})", value, old, vars.gold);
        }
        EvtVariable::FOOD => {
            let old = vars.food;
            vars.food += value;
            info!("  Food += {} ({} -> {})", value, old, vars.food);
        }
        EvtVariable::QBITS => {
            vars.set_qbit(value);
        }
        EvtVariable::AUTONOTES_BITS => {
            vars.add_autonote(value);
        }
        EvtVariable::DAYS_COUNTER1 => vars.days_counters[0] += value,
        EvtVariable::DAYS_COUNTER2 => vars.days_counters[1] += value,
        EvtVariable::DAYS_COUNTER3 => vars.days_counters[2] += value,
        EvtVariable::DAYS_COUNTER4 => vars.days_counters[3] += value,
        EvtVariable::DAYS_COUNTER5 => vars.days_counters[4] += value,
        EvtVariable::DAYS_COUNTER6 => vars.days_counters[5] += value,
        EvtVariable::TOTAL_CIRCUS_PRIZE => vars.total_circus_prize += value,
        _ => {
            if is_character_var(var) {
                let target = party.active_target;
                party.add_member_var(target, var, value);
            } else {
                warn!(
                    "  add_variable: unhandled variable {} (0x{:02x}) += {}",
                    var, var.0, value
                );
            }
        }
    }
}

/// Subtract from a game variable.
pub(in crate::game) fn subtract_variable(vars: &mut GameVariables, party: &mut Party, var: EvtVariable, value: i32) {
    if var.is_map_var() {
        let idx = var.map_var_index().unwrap() as usize;
        let old = vars.map_vars[idx];
        vars.map_vars[idx] = old.wrapping_sub(value);
        info!("  {} -= {} ({} -> {})", var, value, old, vars.map_vars[idx]);
        return;
    }
    match var {
        EvtVariable::GOLD => {
            let old = vars.gold;
            vars.gold -= value;
            info!("  Gold -= {} ({} -> {})", value, old, vars.gold);
        }
        EvtVariable::FOOD => {
            let old = vars.food;
            vars.food -= value;
            info!("  Food -= {} ({} -> {})", value, old, vars.food);
        }
        EvtVariable::QBITS => {
            vars.clear_qbit(value);
        }
        EvtVariable::AUTONOTES_BITS => {
            vars.remove_autonote(value);
        }
        EvtVariable::TOTAL_CIRCUS_PRIZE => vars.total_circus_prize -= value,
        _ => {
            if is_character_var(var) {
                let target = party.active_target;
                party.add_member_var(target, var, -value);
            } else {
                warn!(
                    "  subtract_variable: unhandled variable {} (0x{:02x}) -= {}",
                    var, var.0, value
                );
            }
        }
    }
}

/// Evaluate a Compare condition. Returns true if condition is met (JUMP to jump_step).
/// MM6 Compare semantics: jump when condition is TRUE (e.g. "already done" -> skip).
pub(in crate::game) fn evaluate_compare(
    vars: &GameVariables,
    party: &Party,
    game_time: Option<&super::time::GameTime>,
    var: EvtVariable,
    value: i32,
) -> bool {
    // Special cases: QBits and Autonotes check set membership
    if var == EvtVariable::QBITS {
        let result = vars.has_qbit(value);
        debug!("  Compare: QBit {} present? -> {}", value, result);
        return result;
    }
    if var == EvtVariable::AUTONOTES_BITS {
        let result = vars.has_autonote(value);
        debug!("  Compare: Autonote {} present? -> {}", value, result);
        return result;
    }
    if var == EvtVariable::INVENTORY {
        let result = vars.item_count(value) >= 1;
        debug!("  Compare: HasItem({}) -> {}", value, result);
        return result;
    }

    // MM6 Compare semantics: numeric variables use >= (not ==)
    let current = get_variable(vars, party, game_time, var);
    let result = current >= value;
    debug!("  Compare: {} = {} >= {}? -> {}", var, current, value, result);
    result
}
