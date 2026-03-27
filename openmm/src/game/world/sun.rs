use bevy::prelude::*;

use crate::GameState;
use crate::game::InGame;

pub struct SunPlugin;

impl Plugin for SunPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Game), sun_setup);
    }
}

fn sun_setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Strong ambient to match MM6's flat-lit look — avoids harsh dark triangles
    commands.spawn((
        AmbientLight {
            color: Color::WHITE,
            brightness: 800.0,
            ..default()
        },
        InGame,
    ));

    // Visual sun sphere
    commands.spawn((
        Name::new("fake_sun"),
        Mesh3d(meshes.add(Mesh::from(Sphere { radius: 200.0 }))),
        MeshMaterial3d(materials.add(Color::srgb(0.9, 0.9, 0.2))),
        Transform::from_xyz(10000.0, 30000.0, 10000.0),
        InGame,
    ));

    // Directional light from a high angle
    commands.spawn((
        Name::new("sun"),
        DirectionalLight {
            shadows_enabled: true,
            illuminance: 5000.,
            ..default()
        },
        Transform::from_xyz(10000.0, 30000.0, 10000.0).looking_at(Vec3::ZERO, Vec3::Y),
        InGame,
    ));
}
