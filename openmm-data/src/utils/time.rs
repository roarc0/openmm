// ── Calendar constants ─────────────────────────────────────────────────────────

pub const MINS_PER_HOUR: u64 = 60;
pub const HOURS_PER_DAY: u64 = 24;
pub const MINS_PER_DAY: u64 = MINS_PER_HOUR * HOURS_PER_DAY;

pub const DAYS_IN_MONTH: [u32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
pub const MONTH_NAMES: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
pub const MONTH_NAMES_FULL: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
// Jan 1, Year 1000 is defined as Monday.
pub const DAY_NAMES: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];

pub fn is_leap_year(year: u32) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}

pub fn to_12_hour(hour: u32) -> (u32, &'static str) {
    match hour {
        0 => (12, "am"),
        1..=11 => (hour, "am"),
        12 => (12, "pm"),
        _ => (hour - 12, "pm"),
    }
}

/// Helper for calendar math given total minutes since epoch (midnight, Jan 1, 1000).
pub fn minute(total_minutes: u64) -> u32 {
    (total_minutes % MINS_PER_HOUR) as u32
}

pub fn hour(total_minutes: u64) -> u32 {
    ((total_minutes / MINS_PER_HOUR) % HOURS_PER_DAY) as u32
}

pub fn day_of_week(total_minutes: u64) -> u32 {
    let total_days = total_minutes / MINS_PER_DAY;
    (total_days % 7) as u32
}

pub fn date(total_minutes: u64) -> (u32, u32, u32) {
    let mut remaining = total_minutes / MINS_PER_DAY;

    let mut year = 1000u32;
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        year += 1;
    }

    let mut month = 1u32;
    for (i, &days_in_month) in DAYS_IN_MONTH.iter().enumerate() {
        let mut days = days_in_month as u64;
        if i == 1 && is_leap_year(year) {
            days = 29;
        }
        if remaining < days {
            break;
        }
        remaining -= days;
        month += 1;
    }

    let day = remaining as u32 + 1;
    (year, month, day)
}

/// Time-of-day greeting word for the given hour (0–23).
///
/// - 5–11  → "morning"
/// - 12–17 → "day"
/// - 18–21 → "evening"
/// - 22–4  → "night"
pub fn time_of_day_word(hour: u32) -> &'static str {
    match hour {
        5..=11 => "morning",
        12..=17 => "day",
        18..=21 => "evening",
        _ => "night",
    }
}

pub fn format(total_minutes: u64) -> String {
    let (y, m, d) = date(total_minutes);
    let dow = DAY_NAMES[day_of_week(total_minutes) as usize];
    let month_name = MONTH_NAMES[(m - 1) as usize];
    let (h12, ampm) = to_12_hour(hour(total_minutes));
    format!(
        "{} {} {} {} {}:{:02}{}",
        dow,
        month_name,
        d,
        y,
        h12,
        minute(total_minutes),
        ampm
    )
}

pub fn format_full(total_minutes: u64) -> String {
    let (y, m, d) = date(total_minutes);
    let dow = DAY_NAMES[day_of_week(total_minutes) as usize];
    let month_name = MONTH_NAMES_FULL[(m - 1) as usize];
    let (h12, ampm) = to_12_hour(hour(total_minutes));
    format!(
        "{}:{:02} {} {} {} {} {}",
        h12,
        minute(total_minutes),
        ampm,
        dow,
        d,
        month_name,
        y
    )
}
