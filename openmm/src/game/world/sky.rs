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

/// Slowly rotate the sky dome and tint based on time of day.
fn scroll_sky(
    time: Res<Time>,
    mut sky_query: Query<(&mut Transform, &MeshMaterial3d<StandardMaterial>), With<SkyDome>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    clock_query: Query<&super::sun::DayClock>,
) {
    for (mut transform, mat_handle) in sky_query.iter_mut() {
        transform.rotate_y(time.delta_secs() * 0.005);

        // Tint sky based on time of day
        if let Ok(clock) = clock_query.single() {
            if let Some(mat) = materials.get_mut(&mat_handle.0) {
                let tod = clock.time_of_day;
                let day_amount = 1.0 - (tod * 2.0 - 1.0).abs();
                let dawn_dusk = {
                    let d1 = (tod - 0.25).abs();
                    let d2 = (tod - 0.75).abs();
                    (1.0 - (d1.min(d2) * 10.0).min(1.0)).max(0.0)
                };

                let r: f32 = 0.2 + 0.8 * day_amount + 0.3 * dawn_dusk;
                let g: f32 = 0.2 + 0.7 * day_amount + 0.1 * dawn_dusk;
                let b: f32 = 0.3 + 0.7 * day_amount - 0.15 * dawn_dusk;
                mat.base_color = Color::srgb(
                    r.clamp(0.1, 1.0),
                    g.clamp(0.1, 1.0),
                    b.clamp(0.15, 1.0),
                );
            }
        }
    }
}
