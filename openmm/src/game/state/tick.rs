//! Configurable game-logic tick rate with distance-tiered actor updates.
//!
//! Gameplay systems (AI, combat, physics, sounds) run in `FixedUpdate` at a
//! configurable base rate (default 30 Hz). Actors are further throttled by
//! distance to the player:
//!
//! - **Close** (< `close_range`): every tick
//! - **Medium** (< `medium_range`): every 2nd tick
//! - **Far** (< draw distance): every 4th tick
//! - **Beyond draw distance**: skipped entirely
//!
//! Each actor gets a `tick_phase` derived from its entity index so not all
//! distant actors update on the same frame — prevents stutter spikes.

use bevy::prelude::*;

use crate::system::config::GameConfig;

// ── Distance tier constants ──────────────────────────────────────────────────

/// Default base tick rate for gameplay systems (Hz).
pub const DEFAULT_GAME_TICK_RATE: f64 = 30.0;

/// Default distance threshold for "close" actors (Bevy units).
pub const DEFAULT_CLOSE_RANGE: f32 = 2000.0;

/// Default distance threshold for "medium" actors (Bevy units).
pub const DEFAULT_MEDIUM_RANGE: f32 = 5000.0;

/// Close actors update every tick.
const CLOSE_DIVISOR: u32 = 1;

/// Medium-range actors update every 2nd tick.
const MEDIUM_DIVISOR: u32 = 2;

/// Far actors update every 4th tick.
const FAR_DIVISOR: u32 = 4;

// ── Resources ────────────────────────────────────────────────────────────────

/// Runtime tick configuration. Inserted as a `Resource` and synced from
/// `GameConfig` fields so console commands can tweak it live.
#[derive(Resource)]
pub struct GameTickConfig {
    /// Squared close range (precomputed for fast comparison).
    pub close_range_sq: f32,
    /// Squared medium range.
    pub medium_range_sq: f32,
    /// Monotonic counter incremented every `FixedUpdate`.
    pub tick: u32,
}

impl GameTickConfig {
    pub fn new(close_range: f32, medium_range: f32) -> Self {
        Self {
            close_range_sq: close_range * close_range,
            medium_range_sq: medium_range * medium_range,
            tick: 0,
        }
    }
}

impl Default for GameTickConfig {
    fn default() -> Self {
        Self::new(DEFAULT_CLOSE_RANGE, DEFAULT_MEDIUM_RANGE)
    }
}

// ── Tick helper ──────────────────────────────────────────────────────────────

/// Returns `true` if an actor at `distance_sq` from the player should be
/// updated this tick. `phase` spreads actors across frames so distant ones
/// don't all fire on the same tick.
///
/// Actors beyond `draw_distance_sq` are always skipped.
#[inline]
pub fn should_tick_actor(
    tick: u32,
    phase: u32,
    distance_sq: f32,
    close_sq: f32,
    medium_sq: f32,
    draw_distance_sq: f32,
) -> bool {
    if distance_sq > draw_distance_sq {
        return false;
    }
    let divisor = if distance_sq < close_sq {
        CLOSE_DIVISOR
    } else if distance_sq < medium_sq {
        MEDIUM_DIVISOR
    } else {
        FAR_DIVISOR
    };
    (tick.wrapping_add(phase)).is_multiple_of(divisor)
}

/// Derive a stable phase from an actor's position so each actor lands on a
/// different tick slot. Uses XZ coordinates for spatial spread.
#[inline]
pub fn position_phase(x: f32, z: f32) -> u32 {
    // Mix position bits for a cheap, stable hash.
    let ix = (x * 0.1) as i32;
    let iz = (z * 0.1) as i32;
    (ix.wrapping_mul(2654435761_u32 as i32) ^ iz) as u32
}

// ── Plugin ───────────────────────────────────────────────────────────────────

pub struct GameTickPlugin;

impl Plugin for GameTickPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameTickConfig>()
            .add_systems(FixedUpdate, increment_tick.in_set(GameTickSet));
    }
}

/// System set that runs before all gameplay systems in `FixedUpdate`.
/// Actor systems should run `.after(GameTickSet)`.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct GameTickSet;

fn increment_tick(mut tick_cfg: ResMut<GameTickConfig>) {
    tick_cfg.tick = tick_cfg.tick.wrapping_add(1);
}

/// Syncs `GameTickConfig` ranges and `Time<Fixed>` rate from `GameConfig`.
/// Runs in `Update` so console changes take effect next frame.
pub fn sync_tick_config(
    cfg: Res<GameConfig>,
    mut tick_cfg: ResMut<GameTickConfig>,
    mut fixed_time: ResMut<Time<Fixed>>,
) {
    let target_hz = cfg.game_tick_rate;
    let current_hz = 1.0 / fixed_time.timestep().as_secs_f64();
    if (target_hz - current_hz).abs() > 0.5 {
        fixed_time.set_timestep_hz(target_hz);
        info!("Game tick rate set to {target_hz} Hz");
    }

    let close_sq = cfg.close_actor_range * cfg.close_actor_range;
    let medium_sq = cfg.medium_actor_range * cfg.medium_actor_range;
    if (tick_cfg.close_range_sq - close_sq).abs() > 1.0 {
        tick_cfg.close_range_sq = close_sq;
    }
    if (tick_cfg.medium_range_sq - medium_sq).abs() > 1.0 {
        tick_cfg.medium_range_sq = medium_sq;
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_actors_tick_every_frame() {
        let close_sq = 2000.0 * 2000.0;
        let medium_sq = 5000.0 * 5000.0;
        let draw_sq = 16000.0 * 16000.0;
        let dist = 500.0 * 500.0; // well within close range

        for tick in 0..10 {
            assert!(should_tick_actor(tick, 0, dist, close_sq, medium_sq, draw_sq));
        }
    }

    #[test]
    fn medium_actors_tick_every_2nd_frame() {
        let close_sq = 2000.0 * 2000.0;
        let medium_sq = 5000.0 * 5000.0;
        let draw_sq = 16000.0 * 16000.0;
        let dist = 3000.0 * 3000.0; // between close and medium

        let ticks: Vec<bool> = (0..8)
            .map(|t| should_tick_actor(t, 0, dist, close_sq, medium_sq, draw_sq))
            .collect();
        // Phase 0: ticks on 0, 2, 4, 6
        assert_eq!(ticks, vec![true, false, true, false, true, false, true, false]);
    }

    #[test]
    fn far_actors_tick_every_4th_frame() {
        let close_sq = 2000.0 * 2000.0;
        let medium_sq = 5000.0 * 5000.0;
        let draw_sq = 16000.0 * 16000.0;
        let dist = 8000.0 * 8000.0; // beyond medium

        let ticks: Vec<bool> = (0..8)
            .map(|t| should_tick_actor(t, 0, dist, close_sq, medium_sq, draw_sq))
            .collect();
        assert_eq!(ticks, vec![true, false, false, false, true, false, false, false]);
    }

    #[test]
    fn beyond_draw_distance_never_ticks() {
        let close_sq = 2000.0 * 2000.0;
        let medium_sq = 5000.0 * 5000.0;
        let draw_sq = 16000.0 * 16000.0;
        let dist = 20000.0 * 20000.0; // beyond draw distance

        for tick in 0..10 {
            assert!(!should_tick_actor(tick, 0, dist, close_sq, medium_sq, draw_sq));
        }
    }

    #[test]
    fn phase_offsets_spread_actors() {
        let close_sq = 2000.0 * 2000.0;
        let medium_sq = 5000.0 * 5000.0;
        let draw_sq = 16000.0 * 16000.0;
        let dist = 3000.0 * 3000.0; // medium range, divisor=2

        // Phase 0 ticks on even frames, phase 1 on odd frames
        assert!(should_tick_actor(0, 0, dist, close_sq, medium_sq, draw_sq));
        assert!(!should_tick_actor(0, 1, dist, close_sq, medium_sq, draw_sq));
        assert!(!should_tick_actor(1, 0, dist, close_sq, medium_sq, draw_sq));
        assert!(should_tick_actor(1, 1, dist, close_sq, medium_sq, draw_sq));
    }
}
