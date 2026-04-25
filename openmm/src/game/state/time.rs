use bevy::prelude::*;

use crate::GameState;
use openmm_data::utils::time;

// ── Time scale ────────────────────────────────────────────────────────────────

/// 1 real second = 1 in-game minute, so a full day passes in 24 real minutes.
const SECS_PER_GAME_MINUTE: f64 = 1.0;

// ── Resource ──────────────────────────────────────────────────────────────────

/// Authoritative in-game clock.
///
/// Runs at 1 real second = 1 in-game minute. A full 24-hour game day passes
/// in 24 real minutes. The clock pauses while `UiMode` is not `World`
/// (inventory, dialogue, chests, etc.) or when explicitly stopped via the
/// console commands `time stop` / `time start`.
///
/// **Epoch:** midnight, 1 January, Year 1000 (Monday).
/// **Default start:** 9:00am, 1 January, Year 1000.
#[derive(Resource)]
pub struct GameTime {
    /// Real seconds accumulated while the game clock is running.
    elapsed_secs: f64,
    /// Starting offset in game minutes from the epoch (midnight, Jan 1, Year 1000).
    start_minute: u64,
    /// Explicit pause flag — set by console `time stop` / `time start`.
    paused: bool,
}

impl GameTime {
    /// Explicitly pause or resume the clock (`time stop` / `time start`).
    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    /// Whether the clock is explicitly paused.
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Skip forward by `hours` in-game hours (console `time advance <hours>`).
    pub fn advance_hours(&mut self, hours: f32) {
        // 1 hour = 60 game minutes = 60 real seconds (since 1 sec = 1 game minute)
        self.elapsed_secs += hours as f64 * 60.0 * SECS_PER_GAME_MINUTE;
    }

    /// Total in-game minutes since the epoch.
    pub fn total_minutes(&self) -> u64 {
        self.start_minute + (self.elapsed_secs / SECS_PER_GAME_MINUTE) as u64
    }

    /// Minute within the current hour (0–59).
    pub fn minute(&self) -> u32 {
        time::minute(self.total_minutes())
    }

    /// Hour of the day in 24-hour time (0–23).
    pub fn hour(&self) -> u32 {
        time::hour(self.total_minutes())
    }

    /// Day of week index — 0 = Monday, 6 = Sunday.
    pub fn day_of_week(&self) -> u32 {
        time::day_of_week(self.total_minutes())
    }

    /// Calendar date as `(year, month, day)`, all 1-indexed.
    pub fn calendar_date(&self) -> (u32, u32, u32) {
        time::date(self.total_minutes())
    }

    /// Time of day in [0, 1]: 0.0 = midnight, 0.25 = 6am, 0.5 = noon, 0.75 = 6pm.
    /// Used by the lighting and sky systems.
    ///
    /// Computed from fractional seconds (not integer minutes) so the sun and
    /// shadows move smoothly between game-minute ticks.
    pub fn time_of_day(&self) -> f32 {
        let total_mins_f = self.start_minute as f64 + self.elapsed_secs / SECS_PER_GAME_MINUTE;
        let mins_per_day = time::MINS_PER_DAY as f64;
        ((total_mins_f % mins_per_day) / mins_per_day) as f32
    }

    /// Format the current date and time, e.g. `"Monday Jan 1 1000 9:00am"`.
    pub fn format_datetime(&self) -> String {
        time::format(self.total_minutes())
    }

    /// Create GameTime from MM6 calendar fields.
    /// `month_0` and `day_0` are 0-indexed (from party.bin).
    /// Epoch: midnight Jan 1, Year 1000.
    pub fn from_calendar(year: u32, month_0: u32, day_0: u32, hour: u32, minute: u32) -> Self {
        let years_since_epoch = year.saturating_sub(1000);
        let total_days = years_since_epoch * 336 + month_0 * 28 + day_0;
        let total_minutes = (total_days as u64) * 1440 + (hour as u64) * 60 + minute as u64;
        Self {
            start_minute: total_minutes,
            elapsed_secs: 0.0,
            paused: false,
        }
    }

    /// Export calendar fields (0-indexed month/day) for saving back to party.bin.
    /// Returns `(year, month_0, day_0, hour, minute)`.
    pub fn to_calendar(&self) -> (u32, u32, u32, u32, u32) {
        let total = self.total_minutes();
        let total_days = (total / 1440) as u32;
        let day_minutes = (total % 1440) as u32;
        let hour = day_minutes / 60;
        let minute = day_minutes % 60;
        let year = 1000 + total_days / 336;
        let year_day = total_days % 336;
        let month_0 = year_day / 28;
        let day_0 = year_day % 28;
        (year, month_0, day_0, hour, minute)
    }
}

impl Default for GameTime {
    fn default() -> Self {
        Self {
            elapsed_secs: 0.0,
            start_minute: 9 * time::MINS_PER_HOUR, // 9:00am
            paused: false,
        }
    }
}

// ── Plugin ────────────────────────────────────────────────────────────────────

pub struct GameTimePlugin;

impl Plugin for GameTimePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameTime>().add_systems(
            Update,
            advance_game_time
                .run_if(in_state(GameState::Game))
                .run_if(crate::game::ui::is_world_mode),
        );
    }
}

/// Advance the game clock. 1 real second = 1 in-game minute.
/// Does not run when `UiMode` is not `World` (menus, dialogue, etc.) or when
/// the clock is explicitly paused via `GameTime::set_paused`.
fn advance_game_time(time: Res<Time>, mut game_time: ResMut<GameTime>) {
    if !game_time.paused {
        game_time.elapsed_secs += time.delta_secs_f64();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_calendar_default_matches() {
        // existing Default starts at 9:00am Jan 1 Year 1000
        let default = GameTime::default();
        let from_cal = GameTime::from_calendar(1000, 0, 0, 9, 0);
        assert_eq!(default.total_minutes(), from_cal.total_minutes());
    }

    #[test]
    fn to_calendar_roundtrip() {
        let gt = GameTime::from_calendar(1168, 5, 14, 15, 30);
        let (year, month_0, day_0, hour, minute) = gt.to_calendar();
        assert_eq!(year, 1168);
        assert_eq!(month_0, 5);
        assert_eq!(day_0, 14);
        assert_eq!(hour, 15);
        assert_eq!(minute, 30);
    }

    #[test]
    fn from_calendar_epoch() {
        let gt = GameTime::from_calendar(1000, 0, 0, 0, 0);
        assert_eq!(gt.total_minutes(), 0);
        assert_eq!(gt.hour(), 0);
        assert_eq!(gt.minute(), 0);
    }

    #[test]
    fn to_calendar_epoch() {
        let gt = GameTime::from_calendar(1000, 0, 0, 0, 0);
        let (year, month_0, day_0, hour, minute) = gt.to_calendar();
        assert_eq!((year, month_0, day_0, hour, minute), (1000, 0, 0, 0, 0));
    }

    #[test]
    fn roundtrip_many_values() {
        // test several dates round-trip through from_calendar -> to_calendar
        for &(y, m, d, h, min) in &[
            (1000, 0, 0, 0, 0),
            (1000, 0, 0, 9, 0),
            (1000, 11, 27, 23, 59),
            (1165, 0, 0, 9, 0),
            (1200, 6, 15, 12, 30),
        ] {
            let gt = GameTime::from_calendar(y, m, d, h, min);
            let (ry, rm, rd, rh, rmin) = gt.to_calendar();
            assert_eq!(
                (ry, rm, rd, rh, rmin),
                (y, m, d, h, min),
                "roundtrip failed for ({y}, {m}, {d}, {h}, {min})"
            );
        }
    }
}
