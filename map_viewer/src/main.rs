//#![feature(exclusive_range_pattern)]

use std::path::Path;

use bevy::pbr::wireframe::WireframePlugin;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use bevy_prototype_debug_lines::DebugLinesPlugin;
use player::MovementSettings;

use lod::dtile::DtileBin;
use lod::odm::Odm;
use lod::{raw, Lod};

mod debug_area;
mod odm_mesh;
mod player;
//mod shader;

fn main() {
    App::new()
        //.insert_resource(Msaa::Sample4)
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
    let lod_path = Path::new(&lod_path);
    let games_lod = Lod::open(lod_path.join("games.lod")).unwrap();
    let icons_lod = Lod::open(lod_path.join("icons.lod")).unwrap();
    let bitmaps_lod = Lod::open(lod_path.join("BITMAPS.LOD")).unwrap();

    //load map
    let map_name = "outc1";
    let map = raw::Raw::try_from(
        games_lod
            .try_get_bytes(&format!("{}.odm", map_name))
            .unwrap(),
    )
    .unwrap();
    let map = Odm::try_from(map.data.as_slice()).unwrap();

    //load dtile.bin
    let dtile_data = raw::Raw::try_from(icons_lod.try_get_bytes("dtile.bin").unwrap()).unwrap();
    let tile_table = DtileBin::new(&dtile_data.data).table(map.tile_data);
    tile_table
        .atlas_image(bitmaps_lod)
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

    let mesh = odm_mesh::odm_to_mesh(&map, &tile_table, PrimitiveTopology::TriangleList);
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
