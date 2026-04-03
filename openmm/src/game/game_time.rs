use bevy::prelude::*;

use crate::GameState;

// ── Time scale ────────────────────────────────────────────────────────────────

/// 1 real second = 1 in-game minute, so a full day passes in 24 real minutes.
const SECS_PER_GAME_MINUTE: f64 = 1.0;

// ── Calendar constants ─────────────────────────────────────────────────────────

const MINS_PER_HOUR: u64 = 60;
const HOURS_PER_DAY: u64 = 24;
const MINS_PER_DAY: u64 = MINS_PER_HOUR * HOURS_PER_DAY;
const DAYS_PER_YEAR: u64 = 365; // no leap years

const DAYS_IN_MONTH: [u32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
const MONTH_NAMES: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
// Jan 1, Year 1000 is defined as Monday.
const DAY_NAMES: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];

// ── Resource ──────────────────────────────────────────────────────────────────

/// Authoritative in-game clock.
///
/// Runs at 1 real second = 1 in-game minute. A full 24-hour game day passes
/// in 24 real minutes. The clock pauses while `HudView` is not `World`
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
}

impl Default for GameTime {
    fn default() -> Self {
        Self {
            elapsed_secs: 0.0,
            start_minute: 9 * MINS_PER_HOUR, // 9:00am
            paused: false,
        }
    }
}

impl GameTime {
    /// Total in-game minutes since the epoch.
    fn total_minutes(&self) -> u64 {
        self.start_minute + (self.elapsed_secs / SECS_PER_GAME_MINUTE) as u64
    }

    /// Minute within the current hour (0–59).
    pub fn minute(&self) -> u32 {
        (self.total_minutes() % MINS_PER_HOUR) as u32
    }

    /// Hour of the day in 24-hour time (0–23).
    pub fn hour(&self) -> u32 {
        ((self.total_minutes() / MINS_PER_HOUR) % HOURS_PER_DAY) as u32
    }

    fn total_days(&self) -> u64 {
        self.total_minutes() / MINS_PER_DAY
    }

    /// Day of week index — 0 = Monday, 6 = Sunday.
    pub fn day_of_week(&self) -> u32 {
        (self.total_days() % 7) as u32
    }

    /// Calendar date as `(year, month, day)`, all 1-indexed.
    /// Months use a fixed-365-day year (no leap years).
    pub fn calendar_date(&self) -> (u32, u32, u32) {
        let mut remaining = self.total_days();

        let mut year = 1000u32;
        loop {
            if remaining < DAYS_PER_YEAR {
                break;
            }
            remaining -= DAYS_PER_YEAR;
            year += 1;
        }

        let mut month = 1u32;
        for &days_in_month in &DAYS_IN_MONTH {
            if remaining < days_in_month as u64 {
                break;
            }
            remaining -= days_in_month as u64;
            month += 1;
        }

        let day = remaining as u32 + 1;
        (year, month, day)
    }

    /// Time of day in [0, 1]: 0.0 = midnight, 0.25 = 6am, 0.5 = noon, 0.75 = 6pm.
    /// Used by the lighting and sky systems.
    pub fn time_of_day(&self) -> f32 {
        (self.total_minutes() % MINS_PER_DAY) as f32 / MINS_PER_DAY as f32
    }

    /// Format the current date and time, e.g. `"Monday Jan 1 1000 9:00am"`.
    pub fn format_datetime(&self) -> String {
        let (year, month, day) = self.calendar_date();
        let dow = DAY_NAMES[self.day_of_week() as usize];
        let month_name = MONTH_NAMES[(month - 1) as usize];
        let (hour12, ampm) = to_12_hour(self.hour());
        format!("{} {} {} {} {}:{:02}{}", dow, month_name, day, year, hour12, self.minute(), ampm)
    }
}

fn to_12_hour(hour: u32) -> (u32, &'static str) {
    match hour {
        0 => (12, "am"),
        1..=11 => (hour, "am"),
        12 => (12, "pm"),
        _ => (hour - 12, "pm"),
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
                .run_if(resource_equals(crate::game::hud::HudView::World)),
        );
    }
}

/// Advance the game clock. 1 real second = 1 in-game minute.
/// Does not run when `HudView` is not `World` (menus, dialogue, etc.) or when
/// the clock is explicitly paused via `GameTime::set_paused`.
fn advance_game_time(time: Res<Time>, mut game_time: ResMut<GameTime>) {
    if !game_time.paused {
        game_time.elapsed_secs += time.delta_secs_f64();
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_9am_jan_1_year_1000() {
        let gt = GameTime::default();
        assert_eq!(gt.hour(), 9);
        assert_eq!(gt.minute(), 0);
        let (year, month, day) = gt.calendar_date();
        assert_eq!((year, month, day), (1000, 1, 1));
        assert_eq!(gt.day_of_week(), 0); // Monday
        assert_eq!(gt.format_datetime(), "Monday Jan 1 1000 9:00am");
    }

    #[test]
    fn one_real_minute_advances_one_game_hour() {
        let mut gt = GameTime::default();
        gt.elapsed_secs = 60.0 * SECS_PER_GAME_MINUTE; // 60 real seconds = 60 game minutes
        assert_eq!(gt.hour(), 10);
        assert_eq!(gt.minute(), 0);
        assert_eq!(gt.format_datetime(), "Monday Jan 1 1000 10:00am");
    }

    #[test]
    fn afternoon_formats_correctly() {
        let mut gt = GameTime::default();
        gt.elapsed_secs = 4.0 * 60.0 * SECS_PER_GAME_MINUTE; // +4 hours → 1:00pm
        assert_eq!(gt.hour(), 13);
        assert_eq!(gt.format_datetime(), "Monday Jan 1 1000 1:00pm");
    }

    #[test]
    fn midnight_is_12am() {
        let mut gt = GameTime::default();
        // Advance to midnight: 15 hours from 9am = 900 game minutes = 900 real seconds
        gt.elapsed_secs = 15.0 * 60.0 * SECS_PER_GAME_MINUTE;
        assert_eq!(gt.hour(), 0);
        assert_eq!(gt.format_datetime(), "Tuesday Jan 2 1000 12:00am");
    }

    #[test]
    fn day_rollover_advances_date_and_weekday() {
        let mut gt = GameTime::default();
        // Advance exactly 24 hours → same clock time (9am) but next calendar day.
        gt.elapsed_secs = 24.0 * 60.0 * SECS_PER_GAME_MINUTE; // 1440 real seconds
        let (year, month, day) = gt.calendar_date();
        assert_eq!((year, month, day), (1000, 1, 2));
        assert_eq!(gt.hour(), 9);
        assert_eq!(gt.day_of_week(), 1); // Tuesday
    }

    #[test]
    fn time_of_day_at_noon_is_half() {
        let mut gt = GameTime::default();
        gt.elapsed_secs = 3.0 * 60.0 * SECS_PER_GAME_MINUTE; // 9am + 3h = noon
        assert!((gt.time_of_day() - 0.5).abs() < 0.001);
    }

    #[test]
    fn year_advances_after_365_days() {
        let mut gt = GameTime::default();
        // Advance exactly 365 game days past the start of year 1000
        // From 9am Jan 1 to 9am Jan 1 next year = 365 * 24 * 60 minutes
        gt.elapsed_secs = (365 * 24 * 60) as f64 * SECS_PER_GAME_MINUTE;
        let (year, month, day) = gt.calendar_date();
        assert_eq!(year, 1001);
        assert_eq!((month, day), (1, 1));
    }
}
