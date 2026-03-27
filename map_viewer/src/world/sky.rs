use bevy::{asset::RenderAssetUsages, prelude::*};

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
    let image = Image::from_dynamic(
        settings.lod_manager.bitmap("sky01").unwrap(),
        true,
        RenderAssetUsages::RENDER_WORLD,
    );
    let image_handle = images.add(image);

    commands.spawn((
        Mesh3d(meshes.add(Mesh::from(Cylinder::default()))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color_texture: Some(image_handle),
            alpha_mode: AlphaMode::Opaque,
            unlit: true,
            flip_normal_map_y: true,
            cull_mode: None,
            ..default()
        })),
        Transform::from_scale(Vec3::splat(100_000_000.0)),
    ));
}
