use std::borrow::Cow;

use bevy::{
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    pbr::wireframe::{Wireframe, WireframeConfig},
    prelude::{shape::Quad, *},
};
use bevy_mod_billboard::{
    prelude::{BillboardMeshHandle, BillboardPlugin, BillboardTexture},
    BillboardTextureBundle,
};
use lod::LodManager;
use random_color::{Luminosity, RandomColor};

use crate::{
    despawn_screen,
    odm::{bsp_model_bounding_box, bsp_model_generate_mesh, OdmAsset},
    player::{self, FlyCam},
    GameState,
};

use self::sun::SunPlugin;

pub(crate) mod sun;

#[derive(Component)]
pub struct OnGameScreen;

#[derive(Resource)]
pub(super) struct WorldSettings {
    pub lod_manager: LodManager,
    pub map_name: String,
}

impl Default for WorldSettings {
    fn default() -> Self {
        let mut default = Self {
            lod_manager: LodManager::new(lod::get_lod_path()).unwrap(),
            map_name: "outc2.odm".into(),
        };

        default
    }
}

impl WorldSettings {
    fn get_odm(&self) -> Option<OdmAsset> {
        OdmAsset::new(&self.lod_manager, self.map_name.as_str()).ok()
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
                SunPlugin,
                player::PlayerPlugin,
                BillboardPlugin,
            ))
            .add_systems(OnEnter(GameState::Game), world_setup)
            .add_systems(OnExit(GameState::Game), despawn_screen::<OnGameScreen>);
    }
}

fn random_color() -> Color {
    let color = RandomColor::new()
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
    mut settings: Res<WorldSettings>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut billboard_textures: ResMut<Assets<BillboardTexture>>,
    mut wireframe_config: ResMut<WireframeConfig>,
) {
    let odm = settings.get_odm();
    if !odm.is_some() {
        return;
    }
    let odm = odm.unwrap();

    wireframe_config.global = false;
    let image_handle = images.add(odm.image.clone());
    let material = odm.material(image_handle);

    commands.spawn(PbrBundle {
        mesh: meshes.add(odm.mesh.clone()),
        material: materials.add(material),
        ..default()
    });

    for b in odm.map.bsp_models {
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

    for e in odm.map.entities {
        let color = random_color();

        let name = if e.declist_name == "rock01" {
            "rok1"
        } else {
            &e.declist_name
        }
        .to_string();

        if let Ok(image) = settings.lod_manager.sprite(name.as_str()) {
            let image = bevy::render::texture::Image::from_dynamic(image, true);
            let image_handle = images.add(image);

            commands.spawn(BillboardTextureBundle {
                texture: billboard_textures.add(BillboardTexture::Single(image_handle)),
                transform: Transform::from_xyz(
                    e.data.origin[0] as f32,
                    e.data.origin[2] as f32 + 128.,
                    -e.data.origin[1] as f32,
                ),
                mesh: BillboardMeshHandle(meshes.add(Quad::new(Vec2::new(256., 256.)).into())),
                ..default()
            });
        } else {
            println!("failed to read {}", e.declist_name);
            commands.spawn(PbrBundle {
                mesh: meshes.add(Mesh::from(shape::Cube { size: 120.0 })),
                material: materials.add(color.into()),
                transform: Transform::from_xyz(
                    e.data.origin[0] as f32,
                    e.data.origin[2] as f32,
                    -e.data.origin[1] as f32,
                ),
                ..default()
            });
        }
    }

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
        // settings.map_name = if settings.map_name == "oute3.odm" {
        //     "oute2.odm"
        // } else {
        //     "oute3.odm"
        // }
        // .into();
        // info!("Changing map: {}", &settings.map_name);
    }
}
