#![feature(exclusive_range_pattern)]

use std::f32::consts::PI;
use std::path::Path;

use bevy::pbr::wireframe::WireframePlugin;
use bevy::pbr::CascadeShadowConfigBuilder;
use bevy::prelude::*;
use bevy::render::render_resource::{AddressMode, PrimitiveTopology, SamplerDescriptor};

use bevy_prototype_debug_lines::DebugLinesPlugin;
use lod::dtile::DtileBin;
use lod::image::get_atlas;
use lod::odm::Odm;
use lod::{raw, Lod};
use player::MovementSettings;

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
            speed: 10.0 * 1024.0, // default: 12.0
        })
        .add_systems(Startup, setup)
        .run();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let lod_path = lod::get_lod_path();
    let lod_path = Path::new(&lod_path);
    let games_lod = Lod::open(lod_path.join("games.lod")).unwrap();
    let icons_lod = Lod::open(lod_path.join("icons.lod")).unwrap();
    let bitmaps_lod = Lod::open(lod_path.join("BITMAPS.LOD")).unwrap();

    //load map
    let map_name = "oute3";
    let map = raw::Raw::try_from(
        games_lod
            .try_get_bytes(&format!("{}.odm", map_name))
            .unwrap(),
    )
    .unwrap();
    let map = Odm::try_from(map.data.as_slice()).unwrap();

    //load dtile.bin
    let dtile_data = raw::Raw::try_from(icons_lod.try_get_bytes("dtile.bin").unwrap()).unwrap();
    let dtile_table = DtileBin::new(&dtile_data.data).table(map.tile_data);
    print!("{:?}", &dtile_table);
    let tile_set = dtile_table.names();
    print!("{:?}, ", &tile_set);

    let ts: Vec<&str> = tile_set.iter().map(|s| s.as_str()).collect();
    get_atlas(&bitmaps_lod, ts.as_slice())
        .unwrap()
        .save("map_viewer/assets/terrain_atlas.png")
        .unwrap();

    let image = asset_server.load("terrain_atlas.png");
    let material_handle = materials.add(StandardMaterial {
        base_color_texture: Some(image.clone()),
        unlit: true,
        alpha_mode: AlphaMode::Opaque,
        ..default()
    });

    let mesh = odm_mesh::odm_to_mesh(&map, PrimitiveTopology::TriangleList, &dtile_table);
    commands.spawn(PbrBundle {
        mesh: meshes.add(mesh),
        material: material_handle.clone(),
        ..default()
    });

    let mesh = odm_mesh::odm_to_mesh(&map, PrimitiveTopology::LineList, &dtile_table);
    commands.spawn(PbrBundle {
        mesh: meshes.add(mesh),
        material: material_handle,
        ..default()
    });

    commands.insert_resource(AmbientLight {
        brightness: 1.,
        ..default()
    });

    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            shadows_enabled: true,
            ..default()
        },
        transform: Transform {
            translation: Vec3::new(0.0, 2000.0, 0.0),
            rotation: Quat::from_rotation_x(-PI / 4.),
            ..default()
        },
        cascade_shadow_config: CascadeShadowConfigBuilder {
            first_cascade_far_bound: 4.0,
            maximum_distance: 1000.0,
            ..default()
        }
        .into(),
        ..default()
    });

    // commands.spawn(DirectionalLightBundle {
    //     directional_light: DirectionalLight {
    //         shadows_enabled: true,
    //         illuminance: 10000.,
    //         ..default()
    //     },
    //     transform: Transform::from_xyz(-18.0, 12.0, 6.0),
    //     ..default()
    // });

    info!("Running...");
}
