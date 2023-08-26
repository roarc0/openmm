use bevy::pbr::wireframe::WireframePlugin;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use bevy_prototype_debug_lines::DebugLinesPlugin;
use lod::lod_data::LodData;
use player::MovementSettings;

use lod::dtile::Dtile;
use lod::odm::Odm;
use lod::LodManager;

mod debug_area;
mod lod_asset;
mod odm_mesh;
mod player;

fn main() {
    App::new()
        .insert_resource(Msaa::Sample8)
        .add_plugins(DefaultPlugins)
        .add_plugins(DebugLinesPlugin::default())
        .add_plugins(WireframePlugin)
        .add_plugins(player::PlayerPlugin)
        .add_plugins(debug_area::DebugAreaPlugin)
        .insert_resource(MovementSettings {
            sensitivity: 0.0002,  // default: 0.00012
            speed: 12.0 * 1024.0, // default: 12.0
        })
        .add_systems(Startup, setup)
        .run();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let lod_path = lod::get_lod_path();
    let lod_manager = LodManager::new(lod_path).unwrap();

    //load map
    let map_name = "games/oute3";
    let map = LodData::try_from(
        lod_manager
            .try_get_bytes(&format!("{}.odm", map_name))
            .unwrap(),
    )
    .unwrap();
    let map = Odm::try_from(map.data.as_slice()).unwrap();

    //load dtile.bin
    let dtile_data: LodData<'_> =
        LodData::try_from(lod_manager.try_get_bytes("icons/dtile.bin").unwrap()).unwrap();
    let tile_table = Dtile::new(&dtile_data.data).table(map.tile_data);
    tile_table
        .atlas_image(lod_manager)
        .save("map_viewer/assets/terrain_atlas.png")
        .unwrap();

    let image = asset_server.load("terrain_atlas.png");
    let material_handle = materials.add(StandardMaterial {
        base_color_texture: Some(image.clone()),
        unlit: false,
        alpha_mode: AlphaMode::Opaque,
        fog_enabled: true,
        perceptual_roughness: 1.0,
        reflectance: 0.1,
        ..default()
    });

    let mesh = odm_mesh::generate_mesh(&map, &tile_table, PrimitiveTopology::TriangleList);
    commands.spawn(PbrBundle {
        mesh: meshes.add(mesh),
        material: material_handle.clone(),
        ..default()
    });

    // let mesh = odm_mesh::odm_to_mesh(&map, &tile_table, PrimitiveTopology::LineList);
    // commands.spawn(PbrBundle {
    //     mesh: meshes.add(mesh),
    //     material: material_handle,
    //     ..default()
    // });

    commands.insert_resource(AmbientLight {
        brightness: 0.4,
        ..default()
    });

    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            shadows_enabled: true,
            illuminance: 10000.,
            ..default()
        },
        transform: Transform::from_xyz(0.0, 1000.0, 0.0),
        ..default()
    });

    info!("Running...");
}
