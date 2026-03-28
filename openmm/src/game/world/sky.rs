use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageSamplerDescriptor},
    prelude::*,
};

use crate::{assets::GameAssets, GameState};
use crate::game::InGame;
use crate::game::player::{Player, PlayerCamera};
use crate::states::loading::PreparedWorld;

pub struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Game), sky_setup)
            .add_systems(Update, update_sky.run_if(in_state(GameState::Game)));
    }
}

#[derive(Component)]
struct SkyPlane {
    scroll_offset: f32,
}

fn sky_setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    game_assets: Res<GameAssets>,
    prepared: Option<Res<PreparedWorld>>,
) {
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
        address_mode_v: ImageAddressMode::Repeat,
        ..default()
    });
    let image_handle = images.add(image);

    // Large flat quad above the player, tilted slightly to fill the upper view.
    // The quad is big enough to always cover the sky area.
    let size = 120_000.0;
    let quad = meshes.add(Rectangle::new(size, size));

    commands.spawn((
        Name::new("sky"),
        Mesh3d(quad),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color_texture: Some(image_handle),
            alpha_mode: AlphaMode::Opaque,
            unlit: true,
            cull_mode: None,
            ..default()
        })),
        // Position high above, facing down, with UV tiling
        Transform::from_xyz(0.0, 15000.0, 0.0)
            .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
        InGame,
        SkyPlane { scroll_offset: 0.0 },
    ));
}

/// Follow the player horizontally and scroll the sky UVs.
fn update_sky(
    time: Res<Time>,
    player_query: Query<&GlobalTransform, With<Player>>,
    mut sky_query: Query<(&mut Transform, &mut SkyPlane, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    clock_query: Query<&super::sun::DayClock>,
) {
    let Ok(player_gt) = player_query.single() else {
        return;
    };
    let player_pos = player_gt.translation();

    for (mut transform, mut sky, mat_handle) in sky_query.iter_mut() {
        // Follow player XZ
        transform.translation.x = player_pos.x;
        transform.translation.z = player_pos.z;

        // Scroll UVs by adjusting texture offset via base_color tint
        // (actual UV scrolling needs a shader, so we rotate the plane slowly instead)
        sky.scroll_offset += time.delta_secs() * 0.002;
        transform.rotation = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)
            * Quat::from_rotation_z(sky.scroll_offset);

        // Tint sky based on time of day
        if let Ok(clock) = clock_query.single() {
            if let Some(mat) = materials.get_mut(&mat_handle.0) {
                let tod = clock.time_of_day;
                let day_amount = 1.0 - (tod * 2.0 - 1.0).abs();
                let dawn_dusk: f32 = {
                    let d1 = (tod - 0.25).abs();
                    let d2 = (tod - 0.75).abs();
                    (1.0 - (d1.min(d2) * 10.0).min(1.0)).max(0.0)
                };

                let r: f32 = 0.15 + 0.85 * day_amount + 0.3 * dawn_dusk;
                let g: f32 = 0.15 + 0.75 * day_amount + 0.1 * dawn_dusk;
                let b: f32 = 0.25 + 0.75 * day_amount - 0.15 * dawn_dusk;
                mat.base_color = Color::srgb(
                    r.clamp(0.08, 1.0),
                    g.clamp(0.08, 1.0),
                    b.clamp(0.1, 1.0),
                );
            }
        }
    }
}
