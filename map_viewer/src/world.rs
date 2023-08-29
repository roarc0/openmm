use bevy::{
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    pbr::wireframe::{Wireframe, WireframeConfig},
    prelude::*,
    render::{color, render_resource::PrimitiveTopology},
};
use lod::LodManager;
use random_color::{Luminosity, RandomColor};

use crate::{
    despawn_screen,
    odm::{bsp_model_bounding_box, bsp_model_generate_mesh, OdmAsset},
    player::{self, FlyCam, MovementSettings},
    GameState,
};

#[derive(Component)]
pub struct OnGameScreen;

#[derive(Resource)]
pub(super) struct WorldSettings {
    pub lod_manager: LodManager,
    pub map_name: String,
    pub show_wireframe: bool,
}

impl Default for WorldSettings {
    fn default() -> Self {
        Self {
            lod_manager: LodManager::new(lod::get_lod_path()).unwrap(),
            map_name: "oute3.odm".into(),
            show_wireframe: false,
        }
    }
}

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldSettings>()
            .add_systems(
                Update,
                (
                    update_wireframe_input,
                    update_fps_text,
                    update_position_text,
                    update_map_input,
                )
                    .run_if(in_state(GameState::Game)),
            )
            .add_plugins((
                //debug_area::DebugAreaPlugin,
                player::PlayerPlugin,
            ))
            .add_systems(OnEnter(GameState::Game), world_setup)
            .add_systems(OnExit(GameState::Game), despawn_screen::<OnGameScreen>);
    }
}

// #[derive(Resource, Deref, DerefMut)]
// struct GameTimer(Timer);

// fn game(
//     time: Res<Time>,
//     mut game_state: ResMut<NextState<GameState>>,
//     mut timer: ResMut<GameTimer>,
// ) {
//     if timer.tick(time.delta()).finished() {
//         game_state.set(GameState::Menu);
//     }
// }

fn random_color() -> Color {
    let color = RandomColor::new()
        .hue(random_color::Color::Red)
        .luminosity(Luminosity::Dark)
        .to_rgb_array();

    Color::rgba(
        color[0] as f32 / 255.,
        color[1] as f32 / 255.,
        color[2] as f32 / 255.,
        1.0,
    )
}

fn world_setup(
    mut commands: Commands,
    settings: Res<WorldSettings>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut wireframe_config: ResMut<WireframeConfig>,
) {
    // if current_level.is_some() {
    //     return;
    // }

    wireframe_config.global = false;
    let odm_asset = OdmAsset::new(&settings).unwrap();

    let image_handle = images.add(odm_asset.image.clone());
    let material = odm_asset.material(image_handle);

    commands.spawn(PbrBundle {
        mesh: meshes.add(odm_asset.mesh),
        material: materials.add(material),
        ..default()
    });

    for b in odm_asset.map.bsp_models {
        let color = random_color();
        commands.spawn((
            PbrBundle {
                mesh: meshes.add(bsp_model_bounding_box(&b).into()),
                material: materials.add(Color::rgba(0.0, 0.0, 0.0, 0.1).into()),
                //visibility: Visibility::Hidden,
                ..default()
            },
            Wireframe,
        ));

        let mesh = bsp_model_generate_mesh(b);

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

fn update_wireframe_input(
    keys: Res<Input<KeyCode>>,
    mut wireframe_config: ResMut<WireframeConfig>,
) {
    if keys.just_pressed(KeyCode::BracketLeft) {
        info!("Changed wireframe");
        wireframe_config.global = !wireframe_config.global;
    }
}

#[derive(Component)]
pub struct FpsText;

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
pub struct PositionText;

fn update_position_text(
    mut query: Query<&mut Text, With<PositionText>>,
    query2: Query<&Transform, With<FlyCam>>,
) {
    let transform = query2.get_single().unwrap();
    for mut text in &mut query {
        text.sections[1].value = format!("{:?}", transform.translation);
    }
}

fn update_map_input(keys: Res<Input<KeyCode>>, mut settings: ResMut<WorldSettings>) {
    if keys.just_pressed(KeyCode::Key1) {
        // let pattern = r"out([a-e][1-3]).odm";
        // let re = Regex::new(pattern).unwrap();
        settings.map_name = if settings.map_name == "oute3.odm" {
            "oute2.odm"
        } else {
            "oute3.odm"
        }
        .into();
        info!("Changing map: {}", &settings.map_name);
    }
}
