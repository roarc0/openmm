use bevy::prelude::*;
use openmm_data::dtile::Tileset;

// Synchronized 0.5s timer for all HUD systems
#[derive(Resource)]
pub struct HudThrottle(pub Timer);

impl Default for HudThrottle {
    fn default() -> Self {
        Self(Timer::from_seconds(0.5, TimerMode::Repeating))
    }
}

/// Color for an FPS value: green > 55, yellow > 30, red below.
pub fn fps_color(fps: f64) -> Color {
    if fps >= 55.0 {
        Color::srgb(0.2, 1.0, 0.2)
    } else if fps >= 30.0 {
        Color::srgb(1.0, 0.9, 0.1)
    } else {
        Color::srgb(1.0, 0.2, 0.2)
    }
}

/// Distinct color per terrain tileset.
pub fn tileset_color(ts: Tileset) -> Color {
    match ts {
        Tileset::Grass => Color::srgb(0.3, 0.9, 0.3),
        Tileset::Snow => Color::srgb(0.9, 0.9, 1.0),
        Tileset::Desert => Color::srgb(1.0, 0.85, 0.4),
        Tileset::Volcanic => Color::srgb(0.8, 0.3, 0.2),
        Tileset::Dirt => Color::srgb(0.7, 0.5, 0.3),
        Tileset::Water => Color::srgb(0.3, 0.5, 1.0),
        Tileset::CrackedSwamp => Color::srgb(0.5, 0.5, 0.2),
        Tileset::Swamp => Color::srgb(0.4, 0.6, 0.2),
        Tileset::Road => Color::srgb(0.7, 0.7, 0.7),
    }
}

#[derive(Component)]
pub struct FpsText;

#[derive(Component)]
pub struct CpuText;

#[derive(Component)]
pub struct MapNameSpan;

#[derive(Component)]
pub struct ModeSpan;

#[derive(Component)]
pub struct PosSpan;

#[derive(Component)]
pub struct TileSpan;

#[derive(Component)]
pub struct ChartMaxLabel;

#[derive(Component)]
pub struct ChartMinLabel;
