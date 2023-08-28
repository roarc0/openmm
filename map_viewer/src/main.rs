use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::pbr::wireframe::{Wireframe, WireframeConfig, WireframePlugin};
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use player::{FlyCam, MovementSettings};

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
            FrameTimeDiagnosticsPlugin,
            player::PlayerPlugin,
            debug_area::DebugAreaPlugin,
        ))
        .insert_resource(Msaa::Sample8)
        .insert_resource(MovementSettings {
            sensitivity: 0.00012, // default: 0.00012
            speed: 7.0 * 1024.0,  // default: 12.0
        })
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (wireframe_debug, update_fps_text, update_position_text),
        )
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

    let mut c = 0;
    for b in odm_asset.map.bsp_models {
        let color = if c % 2 == 0 {
            Color::rgba(1.0, 0.1, 0.0, 0.9)
        } else {
            Color::rgba(1.0, 0.1, 1.0, 0.9)
        };
        c += 1;

        commands.spawn((
            PbrBundle {
                mesh: meshes.add(
                    shape::Box::from_corners(
                        [
                            b.header.bounding_box.min_x as f32,
                            b.header.bounding_box.min_z as f32,
                            -b.header.bounding_box.min_y as f32,
                        ]
                        .into(),
                        [
                            b.header.bounding_box.max_x as f32,
                            b.header.bounding_box.max_z as f32,
                            -b.header.bounding_box.max_y as f32,
                        ]
                        .into(),
                    )
                    .into(),
                ),
                material: materials.add(Color::rgba(0.0, 0.0, 0.0, 0.1).into()),
                //visibility: Visibility::Hidden,
                ..default()
            },
            Wireframe,
        ));

        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);
        mesh.set_indices(Some(bevy::render::mesh::Indices::U32(b.indices)));
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, b.vertices);
        mesh.duplicate_vertices();
        mesh.compute_flat_normals();

        commands.spawn((
            PbrBundle {
                mesh: meshes.add(mesh.clone()),
                material: materials.add(StandardMaterial {
                    base_color: color,
                    unlit: false,
                    alpha_mode: AlphaMode::Opaque,
                    fog_enabled: true,
                    perceptual_roughness: 0.5,
                    reflectance: 0.1,
                    //double_sided: true,
                    cull_mode: None,
                    ..default()
                }),
                ..default()
            },
            Wireframe,
        ));
    }

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

    commands.spawn((
        TextBundle::from_sections([
            TextSection::new(
                "FPS: ",
                TextStyle {
                    font_size: 15.0,
                    color: Color::WHITE,
                    ..default()
                },
            ),
            TextSection::from_style(TextStyle {
                font_size: 15.0,
                color: Color::GOLD,
                ..default()
            }),
        ]),
        FpsText,
    ));

    commands.spawn((
        TextBundle::from_sections([
            TextSection::new(
                " POS: ",
                TextStyle {
                    font_size: 15.0,
                    color: Color::WHITE,
                    ..default()
                },
            ),
            TextSection::from_style(TextStyle {
                font_size: 15.0,
                color: Color::GOLD,
                ..default()
            }),
        ]),
        PositionText,
    ));

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

#[derive(Component)]
struct FpsText;

fn update_fps_text(diagnostics: Res<DiagnosticsStore>, mut query: Query<&mut Text, With<FpsText>>) {
    for mut text in &mut query {
        if let Some(fps) = diagnostics.get(FrameTimeDiagnosticsPlugin::FPS) {
            if let Some(value) = fps.smoothed() {
                text.sections[1].value = format!("{value:.2}");
            }
        }
    }
}

#[derive(Component)]
struct PositionText;

fn update_position_text(
    mut query: Query<&mut Text, With<PositionText>>,
    query2: Query<&Transform, With<FlyCam>>,
) {
    let transform = query2.get_single().unwrap();
    for mut text in &mut query {
        text.sections[1].value = format!("{:?}", transform.translation);
    }
}
