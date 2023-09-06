use std::f32::consts::PI;

use bevy::prelude::*;

use crate::{despawn_all, GameState};

use super::InWorld;

pub struct SunPlugin;

impl Plugin for SunPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (update_sun).run_if(in_state(GameState::Game)))
            .add_systems(OnEnter(GameState::Game), sun_setup)
            .add_systems(OnExit(GameState::Game), despawn_all::<InWorld>);
    }
}

fn sun_setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(AmbientLight {
        brightness: 0.6,
        ..default()
    });

    let entity_spawn = Transform::from_xyz(0.0, 20000.0, 0.0)
        .with_rotation(Quat::from_rotation_x(-PI / 4.))
        .translation;

    commands.spawn((
        Name::new("fake_sun"),
        PbrBundle {
            mesh: meshes.add(Mesh::from(shape::UVSphere {
                radius: 500.0,
                ..default()
            })),
            material: materials.add(Color::rgb(0.9, 0.9, 0.2).into()),
            transform: Transform::from_translation(entity_spawn),
            ..default()
        },
        Movable::new(entity_spawn),
    ));

    commands.spawn((
        Name::new("sun"),
        DirectionalLightBundle {
            directional_light: DirectionalLight {
                shadows_enabled: false,
                illuminance: 32000.,
                ..default()
            },
            transform: Transform::from_translation(entity_spawn),
            // cascade_shadow_config: CascadeShadowConfigBuilder {
            //     first_cascade_far_bound: 4.0,
            //     maximum_distance: 100000.0,
            //     ..default()
            // }
            //.into(),
            ..default()
        },
        Movable::new(entity_spawn),
        InWorld,
    ));
}

fn update_sun(time: Res<Time>, mut sun: Query<(&mut Transform, &mut Movable)>) {
    for (mut transform, mut movable) in &mut sun {
        if (movable.spawn - transform.translation).length() > movable.max_distance {
            movable.speed *= -1.0;
        }
        let direction = transform.local_z();
        transform.translation += direction * movable.speed * time.delta_seconds();
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
            speed: 200.0,
        }
    }
}
