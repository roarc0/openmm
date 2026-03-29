use bevy::prelude::*;

use crate::GameState;
use crate::config::GameConfig;
use crate::game::InGame;

/// Full day/night cycle duration in seconds.
const DAY_CYCLE_SECS: f32 = 1800.0; // 30 minutes

pub struct SunPlugin;

impl Plugin for SunPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Game), sun_setup)
            .add_systems(Update, animate_day_cycle.run_if(in_state(GameState::Game)));
    }
}

/// Tracks the time of day as 0.0 (midnight) → 0.5 (noon) → 1.0 (midnight).
#[derive(Component)]
pub struct DayClock {
    /// 0.0 = midnight, 0.25 = sunrise, 0.5 = noon, 0.75 = sunset
    pub time_of_day: f32,
}

#[derive(Component)]
struct AmbientMarker;

fn sun_setup(mut commands: Commands, cfg: Res<GameConfig>) {
    commands.spawn((
        AmbientLight {
            color: Color::srgb(0.75, 0.75, 0.82),
            brightness: 280.0,
            ..default()
        },
        AmbientMarker,
        InGame,
    ));

    // Start at 9am (0.375)
    let tod = 0.375;
    let (dir_transform, color, illuminance) = sun_from_time(tod);

    commands.spawn((
        Name::new("sun"),
        DirectionalLight {
            shadows_enabled: cfg.shadows,
            illuminance,
            color,
            ..default()
        },
        dir_transform,
        DayClock { time_of_day: tod },
        InGame,
    ));
}

/// Compute sun transform, color, and brightness from time of day.
fn sun_from_time(tod: f32) -> (Transform, Color, f32) {
    // Sun angle: 0 at sunrise (0.25), PI at sunset (0.75)
    let sun_progress = ((tod - 0.25) / 0.5).clamp(0.0, 1.0);
    let angle = sun_progress * std::f32::consts::PI;

    let radius = 50000.0;
    let x = angle.cos() * radius;
    let y = angle.sin() * radius;
    let transform = Transform::from_xyz(x, y, 0.0).looking_at(Vec3::ZERO, Vec3::Y);

    // Elevation: 0 at horizon, 1 at zenith
    let elevation = angle.sin().max(0.0);

    // Sun color: warm at horizon, white at noon
    let r = 1.0;
    let g = 0.75 + 0.25 * elevation;
    let b = 0.55 + 0.45 * elevation;
    let color = Color::srgb(r, g, b);

    // Illuminance based on whether sun is up
    let is_day = tod > 0.22 && tod < 0.78;
    let illuminance = if is_day {
        300.0 + 600.0 * elevation
    } else {
        0.0
    };

    (transform, color, illuminance)
}

/// Compute ambient light from time of day.
fn ambient_from_time(tod: f32) -> (Color, f32) {
    // Night: dark blue, low brightness
    // Dawn/dusk: warm orange tint
    // Day: bright, slightly blue

    // How much "day" is it (0=midnight, 1=noon)
    let day_amount = 1.0 - (tod * 2.0 - 1.0).abs(); // 0 at midnight, 1 at noon

    // Smooth transitions
    let dawn_dusk = {
        let dist_to_sunrise = (tod - 0.25).abs();
        let dist_to_sunset = (tod - 0.75).abs();
        let nearest = dist_to_sunrise.min(dist_to_sunset);
        (1.0 - (nearest * 10.0).min(1.0)).max(0.0) // peak at sunrise/sunset
    };

    let r = 0.15 + 0.65 * day_amount + 0.2 * dawn_dusk;
    let g = 0.15 + 0.60 * day_amount + 0.1 * dawn_dusk;
    let b = 0.25 + 0.55 * day_amount - 0.1 * dawn_dusk;

    let brightness = 30.0 + 280.0 * day_amount;

    (Color::srgb(r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0)), brightness)
}

fn animate_day_cycle(
    time: Res<Time>,
    mut sun_query: Query<(&mut Transform, &mut DayClock, &mut DirectionalLight)>,
    mut ambient_query: Query<&mut AmbientLight, With<AmbientMarker>>,
) {
    for (mut transform, mut clock, mut light) in sun_query.iter_mut() {
        clock.time_of_day += time.delta_secs() / DAY_CYCLE_SECS;
        if clock.time_of_day > 1.0 {
            clock.time_of_day -= 1.0;
        }

        let (new_transform, color, illuminance) = sun_from_time(clock.time_of_day);
        *transform = new_transform;
        light.color = color;
        light.illuminance = illuminance;

        // Update ambient
        let (ambient_color, ambient_brightness) = ambient_from_time(clock.time_of_day);
        for mut ambient in ambient_query.iter_mut() {
            ambient.color = ambient_color;
            ambient.brightness = ambient_brightness;
        }
    }
}
