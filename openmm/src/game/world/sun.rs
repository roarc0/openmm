use bevy::prelude::*;

use crate::GameState;
use crate::game::InGame;

pub struct SunPlugin;

impl Plugin for SunPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, update_sun.run_if(in_state(GameState::Game)))
            .add_systems(OnEnter(GameState::Game), sun_setup);
    }
}

fn sun_setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((AmbientLight {
        color: Color::WHITE,
        brightness: 250.0,
        ..default()
    }, InGame));

    let entity_spawn = Transform::from_xyz(0.0, 2000.0, 0.0).translation;
    let entity_spawn2 = Transform::from_xyz(0.0, 5000.0, 0.0).translation;

    commands.spawn((
        Name::new("fake_sun"),
        Mesh3d(meshes.add(Mesh::from(Sphere { radius: 200.0 }))),
        MeshMaterial3d(materials.add(Color::srgb(0.9, 0.9, 0.2))),
        Transform::from_translation(entity_spawn),
        Movable::new(entity_spawn),
        InGame,
    ));

    commands.spawn((
        Name::new("sun"),
        DirectionalLight {
            shadows_enabled: true,
            illuminance: 2000.,
            ..default()
        },
        Transform::from_translation(entity_spawn2),
        Movable::new(entity_spawn2),
        InGame,
    ));
}

fn update_sun(time: Res<Time>, mut sun: Query<(&mut Transform, &mut Movable)>) {
    for (mut transform, mut movable) in &mut sun {
        if (movable.spawn - transform.translation).length() > movable.max_distance {
            movable.speed *= -1.0;
        }
        let direction = transform.local_x();
        transform.translation += direction * movable.speed * time.delta_secs();
    }
}

#[derive(Component)]
struct Movable {
    spawn: Vec3,
    max_distance: f32,
    speed: f32,
}

impl Movable {
    fn new(spawn: Vec3) -> Self {
        Movable {
            spawn,
            max_distance: 32000.0,
            speed: 4000.0,
        }
    }
}
