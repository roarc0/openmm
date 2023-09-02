use bevy::{
    pbr::wireframe::Wireframe,
    prelude::{shape::Quad, *},
};
use bevy_mod_billboard::{
    prelude::{BillboardMeshHandle, BillboardPlugin, BillboardTexture},
    BillboardLockAxis, BillboardLockAxisBundle, BillboardTextureBundle,
};
use lod::LodManager;

use crate::{
    despawn_screen,
    odm::OdmBundle,
    player::{self},
    GameState,
};

use self::{sky::SkyPlugin, sun::SunPlugin};

pub(crate) mod dev;
pub(crate) mod sky;
pub(crate) mod sun;

#[derive(Component)]
pub struct InWorld;

#[derive(Resource)]
pub(super) struct WorldSettings {
    pub lod_manager: LodManager,
    pub map_name: String,
}

impl Default for WorldSettings {
    fn default() -> Self {
        Self {
            lod_manager: LodManager::new(lod::get_lod_path()).unwrap(),
            map_name: "oute3.odm".into(),
        }
    }
}

impl WorldSettings {
    fn get_odm(&self) -> Option<OdmBundle> {
        OdmBundle::new(&self.lod_manager, self.map_name.as_str()).ok()
    }
}

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldSettings>()
            .add_plugins((
                dev::DevPlugin,
                player::PlayerPlugin,
                SunPlugin,
                SkyPlugin,
                BillboardPlugin,
            ))
            .add_systems(
                Update,
                (update_map_input,).run_if(in_state(GameState::Game)),
            )
            .add_systems(OnEnter(GameState::Game), world_setup)
            .add_systems(OnExit(GameState::Game), despawn_screen::<InWorld>);
    }
}

fn world_setup(
    mut commands: Commands,
    settings: Res<WorldSettings>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut billboard_textures: ResMut<Assets<BillboardTexture>>,
) {
    let odm = settings.get_odm();
    if odm.is_none() {
        return;
    }
    let odm = odm.unwrap();

    let image_handle = images.add(odm.texture.clone());
    let material = odm.terrain_material(image_handle);
    commands.spawn(PbrBundle {
        mesh: meshes.add(odm.mesh.clone()),
        material: materials.add(material),
        ..default()
    });

    for m in odm.models {
        commands.spawn((
            PbrBundle {
                mesh: meshes.add(m.mesh.clone()),
                material: materials.add(m.material.clone()),
                ..default()
            },
            Wireframe,
        ));
    }

    let sprite_manager = lod::billboard::BillboardManager::new(&settings.lod_manager).unwrap();

    for b in odm.map.billboards {
        let billboard_sprite = sprite_manager
            .get(&settings.lod_manager, &b.declist_name, b.data.declist_id)
            .unwrap();
        let (width, height) = billboard_sprite.dimensions();

        let image = bevy::render::texture::Image::from_dynamic(billboard_sprite.image, true);
        let image_handle = images.add(image);

        commands.spawn(BillboardLockAxisBundle {
            billboard_bundle: BillboardTextureBundle {
                transform: Transform::from_xyz(
                    b.data.position[0] as f32,
                    b.data.position[2] as f32 + height / 2.,
                    -b.data.position[1] as f32,
                ),
                texture: billboard_textures.add(BillboardTexture::Single(image_handle.clone())),
                mesh: BillboardMeshHandle(meshes.add(Quad::new(Vec2::new(width, height)).into())),
                ..default()
            },
            lock_axis: BillboardLockAxis {
                y_axis: true,
                rotation: false,
            },
        });
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
