use crate::system::config::GameConfig;

/// Returns the bevy log level from config.
pub fn log_level(cfg: &GameConfig) -> bevy::log::Level {
    match cfg.log_level.to_lowercase().as_str() {
        "trace" => bevy::log::Level::TRACE,
        "debug" => bevy::log::Level::DEBUG,
        "info" => bevy::log::Level::INFO,
        "warn" => bevy::log::Level::WARN,
        "error" => bevy::log::Level::ERROR,
        _ => bevy::log::Level::INFO,
    }
}

/// Lowercase string form of a log level — for tracing filter directives.
pub fn log_level_name(level: bevy::log::Level) -> &'static str {
    match level {
        bevy::log::Level::TRACE => "trace",
        bevy::log::Level::DEBUG => "debug",
        bevy::log::Level::INFO => "info",
        bevy::log::Level::WARN => "warn",
        bevy::log::Level::ERROR => "error",
    }
}
