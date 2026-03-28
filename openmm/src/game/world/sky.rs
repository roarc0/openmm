use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageSamplerDescriptor},
    prelude::*,
};

use crate::{assets::GameAssets, GameState};
use crate::game::InGame;
use crate::states::loading::PreparedWorld;

pub struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Game), sky_setup)
            .add_systems(Update, scroll_sky.run_if(in_state(GameState::Game)));
    }
}

#[derive(Component)]
struct SkyDome;

fn sky_setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    game_assets: Res<GameAssets>,
    prepared: Option<Res<PreparedWorld>>,
) {
    // Use the sky texture from the ODM
    let sky_name = prepared
        .as_ref()
        .map(|p| p.map.sky_texture.as_str())
        .unwrap_or("sky01");

    let sky_bitmap = game_assets.lod_manager().bitmap(sky_name)
        .or_else(|| game_assets.lod_manager().bitmap("sky01"));

    let Some(sky_img) = sky_bitmap else {
        return;
    };

    let mut image = Image::from_dynamic(sky_img, true, RenderAssetUsages::RENDER_WORLD);
    image.sampler = bevy::image::ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::ClampToEdge,
        ..default()
    });
    let image_handle = images.add(image);

    // Use a large sphere, rendered from inside (cull_mode: None)
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color_texture: Some(image_handle),
            alpha_mode: AlphaMode::Opaque,
            unlit: true,
            cull_mode: None,
            ..default()
        })),
        Transform::from_scale(Vec3::splat(50_000.0)),
        InGame,
        SkyDome,
    ));
}

/// Slowly rotate the sky dome to simulate cloud movement.
fn scroll_sky(time: Res<Time>, mut query: Query<&mut Transform, With<SkyDome>>) {
    for mut transform in query.iter_mut() {
        transform.rotate_y(time.delta_secs() * 0.005);
    }
}
