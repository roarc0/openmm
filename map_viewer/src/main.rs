use bevy::pbr::wireframe::{WireframeConfig, WireframePlugin};
use bevy::prelude::*;
use player::MovementSettings;

use lod::LodManager;

use crate::odm::OdmAsset;

mod debug_area;
mod lod_asset;
mod odm;
mod player;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            WireframePlugin,
            player::PlayerPlugin,
            debug_area::DebugAreaPlugin,
        ))
        .insert_resource(Msaa::Sample8)
        .insert_resource(MovementSettings {
            sensitivity: 0.00012, // default: 0.00012
            speed: 12.0 * 1024.0, // default: 12.0
        })
        .add_systems(Startup, setup)
        .add_systems(Update, wireframe_debug)
        .run();
}

fn setup(
    mut commands: Commands,
    images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut wireframe_config: ResMut<WireframeConfig>,
) {
    wireframe_config.global = false;

    let lod_manager = LodManager::new(lod::get_lod_path()).unwrap();
    let odm_asset = OdmAsset::new(images, &lod_manager, "oute3.odm").unwrap();

    commands.spawn(PbrBundle {
        mesh: meshes.add(odm_asset.mesh),
        material: materials.add(odm_asset.material),
        ..default()
    });

    commands.insert_resource(AmbientLight {
        brightness: 0.3,
        ..default()
    });

    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            shadows_enabled: true,
            illuminance: 15000.,
            ..default()
        },
        transform: Transform::from_xyz(1000.0, 1500.0, 0.0),
        ..default()
    });

    info!("Running...");
}

fn wireframe_debug(
    //mut commands: Commands,
    keys: Res<Input<KeyCode>>,
    mut wireframe_config: ResMut<WireframeConfig>,
) {
    if keys.just_pressed(KeyCode::BracketLeft) {
        info!("Changed wireframe");
        wireframe_config.global = !wireframe_config.global;
    }
}
