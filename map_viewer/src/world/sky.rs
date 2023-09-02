use bevy::prelude::*;

use crate::{despawn_screen, GameState};

use super::{InWorld, WorldSettings};

pub struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        app
            //.add_systems(Update, (update_sky).run_if(in_state(GameState::Game)))
            .add_systems(OnEnter(GameState::Game), sky_setup)
            .add_systems(OnExit(GameState::Game), despawn_screen::<InWorld>);
    }
}

fn sky_setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    settings: Res<WorldSettings>,
) {
    let image = bevy::render::texture::Image::from_dynamic(
        settings.lod_manager.bitmap("sky01").unwrap(),
        true,
    );
    let image_handle = images.add(image);

    commands.spawn(PbrBundle {
        mesh: meshes.add(Mesh::from(shape::Box::default())),
        material: materials.add(StandardMaterial {
            //base_color: Color::hex("4488dd").unwrap(),
            base_color_texture: Some(image_handle),
            unlit: true,
            flip_normal_map_y:true,
            alpha_mode: AlphaMode::Opaque,
            fog_enabled: false,
            cull_mode: None,
            ..default()
        }),
        transform: Transform::from_scale(Vec3::splat(10_000_000.0)),
        ..default()
    });
}
