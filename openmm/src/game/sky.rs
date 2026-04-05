use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};

use crate::GameState;
use crate::assets::GameAssets;
use crate::game::InGame;
use crate::game::player::PlayerCamera;
use crate::states::loading::PreparedWorld;

/// Marker for the sky dome entity.
#[derive(Component)]
struct SkyDome;

/// Custom unlit material for the sky with time-scrolling UVs.
#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct SkyMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub sky_texture: Handle<Image>,
}

impl Material for SkyMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/sky.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/sky.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}

pub struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        // ClearColor = fog/sky target. Updated each frame by update_sky_color.
        app.insert_resource(ClearColor(Color::srgb(0.38, 0.43, 0.52)))
            .add_plugins(MaterialPlugin::<SkyMaterial>::default())
            .add_systems(OnEnter(GameState::Game), spawn_sky)
            .add_systems(
                Update,
                (follow_camera, update_sky_color, sync_fog_to_sky)
                    .chain()
                    .run_if(in_state(GameState::Game)),
            );
    }
}

/// Spawn the sky dome — a large inverted cylinder around the camera
/// textured with the map's sky texture from the LOD.
fn spawn_sky(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut sky_materials: ResMut<Assets<SkyMaterial>>,
    prepared: Option<Res<PreparedWorld>>,
    indoor: Option<Res<crate::states::loading::PreparedIndoorWorld>>,
    game_assets: Res<GameAssets>,
) {
    // Indoor maps: dark ceiling, no sky dome
    if indoor.is_some() {
        commands.insert_resource(ClearColor(Color::BLACK));
        return;
    }

    // Determine sky texture name from ODM, fallback to plansky1
    let sky_name = prepared
        .as_ref()
        .map(|p| p.map.sky_texture.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("plansky1");
    info!("Sky: loading texture '{}'", sky_name);

    // Load sky texture from LOD
    let sky_img = game_assets
        .game_lod()
        .bitmap(sky_name)
        .or_else(|| game_assets.game_lod().bitmap("plansky1"));
    info!("Sky: texture loaded = {}", sky_img.is_some());

    let sky_tex_handle = if let Some(img) = sky_img {
        let mut bevy_img = crate::assets::dynamic_to_bevy_image(img);
        bevy_img.sampler = crate::assets::repeat_linear_sampler();
        images.add(bevy_img)
    } else {
        images.add(Image::default())
    };

    // Sky plane — a large flat quad above the camera. Perspective naturally
    // creates the MM6-style foreshortening (clouds compressed at the horizon,
    // spread out overhead). No complex UV math needed.
    // Size must be large enough that edges are always beyond the fog end distance.
    let sky_size = 300000.0;
    let uv_repeat = 12.0;

    let mesh = build_sky_plane(sky_size, uv_repeat);

    let sky_mat = sky_materials.add(SkyMaterial {
        sky_texture: sky_tex_handle,
    });

    commands.spawn((
        Name::new("sky_plane"),
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(sky_mat),
        Transform::from_translation(Vec3::new(0.0, 3000.0, 0.0)),
        SkyDome,
        InGame,
    ));
}

/// Build a flat horizontal plane mesh for the sky.
/// The plane is centered at origin in XZ, at Y=0 (positioned via Transform).
fn build_sky_plane(size: f32, uv_repeat: f32) -> Mesh {
    let half = size / 2.0;
    let positions = vec![
        [-half, 0.0, -half],
        [half, 0.0, -half],
        [half, 0.0, half],
        [-half, 0.0, half],
    ];
    let normals = vec![[0.0, -1.0, 0.0], [0.0, -1.0, 0.0], [0.0, -1.0, 0.0], [0.0, -1.0, 0.0]];
    let uvs = vec![[0.0, 0.0], [uv_repeat, 0.0], [uv_repeat, uv_repeat], [0.0, uv_repeat]];
    // Both windings so the plane is visible from both sides
    let indices = vec![0u32, 1, 2, 0, 2, 3, 0, 2, 1, 0, 3, 2];

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Keep the sky plane centered on the camera, always a fixed height above.
const SKY_HEIGHT_ABOVE_CAMERA: f32 = 3000.0;

fn follow_camera(
    camera_query: Query<&GlobalTransform, With<PlayerCamera>>,
    mut sky_query: Query<&mut Transform, With<SkyDome>>,
) {
    let Ok(cam_gt) = camera_query.single() else { return };
    let cam_pos = cam_gt.translation();
    for mut transform in sky_query.iter_mut() {
        transform.translation.x = cam_pos.x;
        transform.translation.y = cam_pos.y + SKY_HEIGHT_ABOVE_CAMERA;
        transform.translation.z = cam_pos.z;
    }
}

/// Update the clear color (sky background) based on time of day.
/// Skipped indoors — black is set once in spawn_sky and must not be overwritten.
fn update_sky_color(
    mut clear_color: ResMut<ClearColor>,
    game_time: Res<crate::game::game_time::GameTime>,
    indoor: Option<Res<crate::states::loading::PreparedIndoorWorld>>,
) {
    if indoor.is_some() {
        return;
    }
    let tod = game_time.time_of_day();

    let day_amount = 1.0 - (tod * 2.0 - 1.0).abs();
    let dawn_dusk: f32 = {
        let d1 = (tod - 0.25).abs();
        let d2 = (tod - 0.75).abs();
        (1.0 - (d1.min(d2) * 8.0).min(1.0)).max(0.0)
    };

    // Fog/sky blend color matching plansky2 horizon + terrain tones.
    // Day: hazy blue-green (0.38, 0.43, 0.52). Night: dark. Dawn/dusk: warm.
    let r: f32 = 0.10 + 0.28 * day_amount + 0.40 * dawn_dusk;
    let g: f32 = 0.10 + 0.33 * day_amount + 0.18 * dawn_dusk;
    let b: f32 = 0.14 + 0.38 * day_amount - 0.08 * dawn_dusk;

    clear_color.0 = Color::srgb(r.clamp(0.06, 0.80), g.clamp(0.06, 0.70), b.clamp(0.08, 0.65));
}

/// Keep fog color in sync with the sky so distant objects fade into the horizon.
fn sync_fog_to_sky(clear_color: Res<ClearColor>, mut fog_query: Query<&mut DistanceFog>) {
    let sky = clear_color.0;
    for mut fog in fog_query.iter_mut() {
        fog.color = sky;
    }
}
