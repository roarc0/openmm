use bevy::{prelude::*, render::render_asset::RenderAssetUsages};

use crate::{despawn_all, GameState};

use super::{InWorld, WorldSettings};

pub struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Game), sky_setup)
            .add_systems(OnExit(GameState::Game), despawn_all::<InWorld>);
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
        RenderAssetUsages::RENDER_WORLD,
    );
    let image_handle = images.add(image);

    commands.spawn(PbrBundle {
        mesh: meshes.add(Mesh::from(Cylinder::default())),
        material: materials.add(StandardMaterial {
            base_color_texture: Some(image_handle),
            alpha_mode: AlphaMode::Opaque,
            unlit: true,
            flip_normal_map_y: true,
            fog_enabled: true,
            cull_mode: None,
            ..default()
        }),
        transform: Transform::from_scale(Vec3::splat(100_000_000.0)),
        ..default()
    });
}
