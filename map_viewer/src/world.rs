use bevy::{
    pbr::wireframe::Wireframe,
    prelude::{shape::Quad, *},
};
use bevy_mod_billboard::{
    prelude::{BillboardMeshHandle, BillboardPlugin, BillboardTexture},
    BillboardLockAxis, BillboardLockAxisBundle, BillboardTextureBundle,
};
use lod::{
    odm::{ODM_HEIGHT_SCALE, ODM_PLAY_SIZE, ODM_TILE_SCALE},
    LodManager,
};

use crate::{
    despawn_all,
    odm::{OdmBundle, OdmName},
    player::{self, MovementSettings, PlayerPlugin},
    GameState,
};

use self::{sky::SkyPlugin, sun::SunPlugin};

pub(crate) mod sky;
pub(crate) mod sun;

#[derive(Component)]
pub struct InWorld;

#[derive(Resource)]
pub(super) struct WorldSettings {
    pub lod_manager: LodManager,
    pub current_odm: OdmName,
    pub swap_odm: bool,
}

impl Default for WorldSettings {
    fn default() -> Self {
        Self {
            lod_manager: LodManager::new(lod::get_lod_path()).expect("unable to load lod files"),
            current_odm: OdmName::default(),
            swap_odm: false,
        }
    }
}

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldSettings>()
            .insert_resource(MovementSettings {
                max_xz: ODM_TILE_SCALE * ODM_PLAY_SIZE as f32 / 2.0,
                max_y: ODM_TILE_SCALE * ODM_HEIGHT_SCALE / 2.0,
                ..Default::default()
            })
            .add_plugins((PlayerPlugin, SunPlugin, SkyPlugin, BillboardPlugin))
            .add_systems(
                Update,
                (change_map_input, change_map).run_if(in_state(GameState::Game)),
            )
            .add_systems(OnEnter(GameState::Game), world_setup)
            .add_systems(OnExit(GameState::Game), despawn_all::<InWorld>);
    }
}

#[derive(Component)]
struct CurrentMap;

fn world_setup(mut commands: Commands) {}

fn change_map(
    mut commands: Commands,
    mut settings: ResMut<WorldSettings>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut billboard_textures: ResMut<Assets<BillboardTexture>>,
    query: Query<Entity, With<CurrentMap>>,
) {
    if settings.swap_odm {
        return;
    }

    for entity in &query {
        commands.entity(entity).despawn_recursive();
    }

    let odm = OdmBundle::new(
        &settings.lod_manager,
        settings.current_odm.to_string().as_str(),
    );

    if odm.is_err() {
        return;
    }
    let odm = odm.unwrap();

    let image_handle = images.add(odm.texture.clone());
    let material = odm.terrain_material(image_handle);

    commands
        .spawn((
            Name::new("odm"),
            PbrBundle {
                mesh: meshes.add(odm.mesh.clone()),
                material: materials.add(material),
                ..default()
            },
            InWorld,
            CurrentMap,
        ))
        .with_children(|parent| {
            for m in odm.models {
                parent.spawn((
                    Name::new("model"),
                    PbrBundle {
                        mesh: meshes.add(m.mesh.clone()),
                        material: materials.add(m.material.clone()),

                        ..default()
                    },
                ));
            }

            let sprite_manager =
                lod::billboard::BillboardManager::new(&settings.lod_manager).unwrap();

            for b in odm.map.billboards {
                let billboard_sprite = sprite_manager
                    .get(&settings.lod_manager, &b.declist_name, b.data.declist_id)
                    .unwrap();
                let (width, height) = billboard_sprite.dimensions();

                let image =
                    bevy::render::texture::Image::from_dynamic(billboard_sprite.image, true);
                let image_handle = images.add(image);

                parent.spawn((
                    Name::new("billboard"),
                    BillboardLockAxisBundle {
                        billboard_bundle: BillboardTextureBundle {
                            transform: Transform::from_xyz(
                                b.data.position[0] as f32,
                                b.data.position[2] as f32 + height / 2.,
                                -b.data.position[1] as f32,
                            ),
                            texture: billboard_textures
                                .add(BillboardTexture::Single(image_handle.clone())),
                            mesh: BillboardMeshHandle(
                                meshes.add(Quad::new(Vec2::new(width, height)).into()),
                            ),
                            ..default()
                        },
                        lock_axis: BillboardLockAxis {
                            y_axis: true,
                            rotation: false,
                        },
                    },
                ));
            }
        });

    settings.swap_odm = true;
}

fn change_map_input(keys: Res<Input<KeyCode>>, mut settings: ResMut<WorldSettings>) {
    let new_map = if keys.just_pressed(KeyCode::J) {
        settings.current_odm.go_north()
    } else if keys.just_pressed(KeyCode::H) {
        settings.current_odm.go_west()
    } else if keys.just_pressed(KeyCode::K) {
        settings.current_odm.go_south()
    } else if keys.just_pressed(KeyCode::L) {
        settings.current_odm.go_east()
    } else {
        None
    };

    if let Some(new_map) = new_map {
        settings.current_odm = new_map;
        settings.swap_odm = false;
        info!("Changing map: {}", &settings.current_odm);
    }
}
