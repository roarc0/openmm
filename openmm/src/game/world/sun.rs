use bevy::prelude::*;

use crate::GameState;
use crate::game::InGame;

pub struct SunPlugin;

impl Plugin for SunPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Game), sun_setup)
            .add_systems(Update, animate_sun.run_if(in_state(GameState::Game)));
    }
}

/// Marks the directional light entity so we can rotate it.
#[derive(Component)]
struct Sun {
    /// Current angle in radians (0 = sunrise east, PI/2 = noon overhead, PI = sunset west)
    angle: f32,
    /// Radians per second — full day cycle speed
    speed: f32,
}

fn sun_setup(mut commands: Commands) {
    commands.spawn((
        AmbientLight {
            color: Color::srgb(0.75, 0.75, 0.82),
            brightness: 280.0,
            ..default()
        },
        InGame,
    ));

    // Directional light — no mesh, just light with shadows
    // Starts at mid-morning angle
    let initial_angle: f32 = 1.0; // ~57 degrees, morning
    let (dir_transform, _) = sun_transform_and_color(initial_angle);

    commands.spawn((
        Name::new("sun"),
        DirectionalLight {
            shadows_enabled: false,
            illuminance: 600.,
            color: Color::srgb(1.0, 0.95, 0.85),
            ..default()
        },
        dir_transform,
        Sun {
            angle: initial_angle,
            // Full cycle (0 to PI) in ~5 minutes for visible movement
            speed: std::f32::consts::PI / 300.0,
        },
        InGame,
    ));
}

/// Compute the sun's transform and color tint from its angle.
/// angle 0 = east horizon, PI/2 = directly overhead, PI = west horizon
fn sun_transform_and_color(angle: f32) -> (Transform, Color) {
    // Sun orbits in the XY plane (east-west arc)
    let radius = 50000.0;
    let x = angle.cos() * radius;
    let y = angle.sin() * radius;

    let transform = Transform::from_xyz(x, y, 0.0).looking_at(Vec3::ZERO, Vec3::Y);

    // Warm color near horizon, white at noon
    let elevation = angle.sin().max(0.0); // 0 at horizon, 1 at zenith
    let r = 1.0;
    let g = 0.85 + 0.15 * elevation;
    let b = 0.7 + 0.3 * elevation;
    let color = Color::srgb(r, g, b);

    (transform, color)
}

fn animate_sun(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &mut Sun, &mut DirectionalLight)>,
) {
    for (mut transform, mut sun, mut light) in query.iter_mut() {
        sun.angle += sun.speed * time.delta_secs();

        // Wrap around: keep between 0.1 and PI-0.1 (never fully below horizon)
        if sun.angle > std::f32::consts::PI - 0.1 {
            sun.angle = 0.1;
        }

        let (new_transform, color) = sun_transform_and_color(sun.angle);
        *transform = new_transform;
        light.color = color;

        // Dim light near horizon, bright at noon
        let elevation = sun.angle.sin().max(0.0);
        light.illuminance = 300.0 + 600.0 * elevation;
    }
}
