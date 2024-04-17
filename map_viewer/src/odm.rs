use bevy::{
    prelude::{shape::Quad, *},
    render::{
        render_asset::RenderAssetUsages,
        render_resource::{Face, PrimitiveTopology, Texture},
    },
};

// use bevy_mod_billboard::{
//     prelude::{BillboardMeshHandle, BillboardPlugin, BillboardTextureBundle},
//     BillboardLockAxis, BillboardLockAxisBundle, BillboardTextureHandle,
// };

use std::error::Error;

use crate::{despawn_all, utils::random_color, world::WorldSettings, GameState};
use lod::{
    dtile::TileTable,
    odm::{Odm, OdmData},
    LodManager,
};

// TODO make it a real bundle
pub(super) struct OdmBundle {
    pub map: Odm,
    pub mesh: Mesh,
    pub texture: Image,
    pub models: Vec<ModelBundle>,
}

// TODO make it a real bundle
pub(super) struct ModelBundle {
    pub mesh: Mesh,
    pub bounding_box_mesh: Mesh,
    pub material: StandardMaterial,
}

impl OdmBundle {
    pub(super) fn new(lod_manager: &LodManager, map_name: &str) -> Result<Self, Box<dyn Error>> {
        let map = Odm::new(lod_manager, map_name)?;
        let tile_table = map.tile_table(lod_manager)?;
        let mesh = Self::generate_terrain_mesh(&map, &tile_table);
        let image = bevy::render::texture::Image::from_dynamic(
            tile_table.atlas_image(lod_manager)?,
            true,
            RenderAssetUsages::RENDER_WORLD,
        );
        let models = process_models(&map);

        Ok(OdmBundle {
            map,
            mesh,
            texture: image,
            models,
        })
    }

    pub fn terrain_material(&self, image_handle: Handle<Image>) -> StandardMaterial {
        StandardMaterial {
            base_color_texture: Some(image_handle),
            unlit: false,
            alpha_mode: AlphaMode::Opaque,
            fog_enabled: true,
            perceptual_roughness: 1.0,
            reflectance: 0.2,
            flip_normal_map_y: true,
            cull_mode: Some(Face::Back),
            ..default()
        }
    }

    fn generate_terrain_mesh(odm: &Odm, tile_table: &TileTable) -> Mesh {
        let odm_data = OdmData::new(odm, tile_table);
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        );
        mesh.insert_indices(bevy::render::mesh::Indices::U32(odm_data.indices.clone()));
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, odm_data.positions);
        mesh.duplicate_vertices();
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, odm_data.uvs);

        // let normals = generate_terrain_normals(
        //     mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        //         .unwrap()
        //         .as_float3()
        //         .unwrap(),
        //     &odm_data.indices,
        // );
        // mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);

        mesh.compute_flat_normals();
        mesh.compute_aabb();
        _ = mesh.generate_tangents();

        mesh
    }
}

fn process_models(map: &Odm) -> Vec<ModelBundle> {
    let mut models = Vec::new();
    for b in &map.bsp_models {
        let bounding_box_mesh: Mesh = generate_bsp_model_bounding_box(b).into();
        let mesh = generate_bsp_model_mesh(b);
        let material = StandardMaterial {
            base_color: random_color(),
            alpha_mode: AlphaMode::Opaque,
            cull_mode: None,
            ..default()
        };
        models.push(ModelBundle {
            bounding_box_mesh,
            mesh,
            material,
        });
    }
    models
}

fn generate_normals(vertices: &[[f32; 3]], indices: &[u32]) -> Vec<[f32; 3]> {
    let mut normals = vec![[0.0, 0.0, 0.0]; vertices.len()];

    for face_indices in indices.chunks(3) {
        let v0 = vertices[face_indices[0] as usize];
        let v1 = vertices[face_indices[1] as usize];
        let v2 = vertices[face_indices[2] as usize];

        let edge1 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
        let edge2 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];

        let cross_product = [
            edge1[1] * edge2[2] - edge1[2] * edge2[1],
            edge1[2] * edge2[0] - edge1[0] * edge2[2],
            edge1[0] * edge2[1] - edge1[1] * edge2[0],
        ];

        for index in face_indices {
            normals[*index as usize][0] += cross_product[0];
            normals[*index as usize][1] += cross_product[1];
            normals[*index as usize][2] += cross_product[2];
        }
    }

    for normal in normals.iter_mut() {
        let length = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
        normal[0] /= length;
        normal[1] /= length;
        normal[2] /= length;
    }
    normals
}

fn generate_bsp_model_mesh(model: &lod::bsp_model::BSPModel) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_indices(bevy::render::mesh::Indices::U32(model.indices.clone()));
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, model.vertices.clone());
    mesh.duplicate_vertices();

    mesh.compute_flat_normals();
    mesh.compute_aabb();
    _ = mesh.generate_tangents();

    mesh
}

fn generate_bsp_model_bounding_box(model: &lod::bsp_model::BSPModel) -> Cuboid {
    Cuboid::from_corners(
        Vec3::new(
            model.header.bounding_box.min_x as f32,
            model.header.bounding_box.min_z as f32,
            -model.header.bounding_box.min_y as f32,
        ),
        Vec3::new(
            model.header.bounding_box.max_x as f32,
            model.header.bounding_box.max_z as f32,
            -model.header.bounding_box.max_y as f32,
        ),
    )
}

pub(super) struct OdmName {
    pub x: char,
    pub y: char,
}

impl Default for OdmName {
    fn default() -> Self {
        Self { x: 'e', y: '3' }
    }
}

use std::fmt::Display;

impl Display for OdmName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(Self::map_name(self.x, self.y).as_str())
    }
}

impl TryFrom<&str> for OdmName {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let x = value.as_bytes().get(3).copied().ok_or("invalid map name")? as char;
        let y = value.as_bytes().get(4).copied().ok_or("invalid map name")? as char;

        let x = Self::validate_x(x).ok_or("invalid map x coordinate")?;
        let y = Self::validate_y(y).ok_or("invalid map y coordinate")?;

        Ok(Self { x, y })
    }
}

impl OdmName {
    pub fn go_north(&self) -> Option<OdmName> {
        let y = Self::validate_y((self.y as u8 - 1) as char)?;
        Some(Self { x: self.x, y })
    }

    pub fn go_west(&self) -> Option<OdmName> {
        let x = Self::validate_x((self.x as u8 - 1) as char)?;
        Some(Self { x, y: self.y })
    }

    pub fn go_south(&self) -> Option<OdmName> {
        let y = Self::validate_y((self.y as u8 + 1) as char)?;
        Some(Self { x: self.x, y })
    }

    pub fn go_east(&self) -> Option<OdmName> {
        let x = Self::validate_x((self.x as u8 + 1) as char)?;
        Some(Self { x, y: self.y })
    }

    fn map_name(x: char, y: char) -> String {
        format!("out{}{}.odm", x, y)
    }

    fn validate_x(c: char) -> Option<char> {
        match c {
            'a'..='e' => Some(c),
            _ => None,
        }
    }
    fn validate_y(c: char) -> Option<char> {
        match c {
            '1'..='3' => Some(c),
            _ => None,
        }
    }
}

#[derive(Component)]
struct CurrentMap;

fn odm_setup(mut commands: Commands) {}

fn change_odm(
    mut commands: Commands,
    mut settings: ResMut<WorldSettings>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    //mut billboard_textures: ResMut<Assets<BillboardTextureBundle>>,
    query: Query<Entity, With<CurrentMap>>,
) {
    if !settings.odm_changed {
        return;
    }

    for e in &query {
        commands.entity(e).despawn_recursive();
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

            // let sprite_manager =
            //     lod::billboard::BillboardManager::new(&settings.lod_manager).unwrap();

            // for b in odm.map.billboards {
            //     let billboard_sprite = sprite_manager
            //         .get(&settings.lod_manager, &b.declist_name, b.data.declist_id)
            //         .unwrap();
            //     let (width, height) = billboard_sprite.dimensions();

            //     let image = bevy::render::texture::Image::from_dynamic(
            //         billboard_sprite.image,
            //         true,
            //         RenderAssetUsages::RENDER_WORLD,
            //     );
            //     let image_handle = images.add(image);

            //     parent.spawn((
            //         Name::new("billboard"),
            //         BillboardLockAxisBundle {
            //             billboard_bundle: BillboardTextureBundle {
            //                 transform: Transform::from_xyz(
            //                     b.data.position[0] as f32,
            //                     b.data.position[2] as f32 + height / 2.,
            //                     -b.data.position[1] as f32,
            //                 ),
            //                 texture: bevy_mod_billboard::BillboardTextureHandle(
            //                     image_handle.clone(),
            //                 ),
            //                 mesh: BillboardMeshHandle(meshes.add(Rectangle::new(width, height))),
            //                 ..default()
            //             },
            //             lock_axis: BillboardLockAxis {
            //                 y_axis: true,
            //                 rotation: false,
            //             },
            //         },
            //     ));
            // }
        });

    settings.odm_changed = false;
}

fn change_map_input(keys: Res<ButtonInput<KeyCode>>, mut settings: ResMut<WorldSettings>) {
    let new_map = if keys.just_pressed(KeyCode::KeyJ) {
        settings.current_odm.go_north()
    } else if keys.just_pressed(KeyCode::KeyH) {
        settings.current_odm.go_west()
    } else if keys.just_pressed(KeyCode::KeyK) {
        settings.current_odm.go_south()
    } else if keys.just_pressed(KeyCode::KeyL) {
        settings.current_odm.go_east()
    } else {
        None
    };

    if let Some(new_map) = new_map {
        settings.current_odm = new_map;
        settings.odm_changed = true;
        info!("Changing map: {}", &settings.current_odm);
    }
}

pub struct OdmPlugin;

impl Plugin for OdmPlugin {
    fn build(&self, app: &mut App) {
        app
            //.add_plugins(BillboardPlugin)
            .add_systems(
                Update,
                (change_map_input, change_odm).run_if(in_state(GameState::Game)),
            )
            .add_systems(OnEnter(GameState::Game), odm_setup)
            .add_systems(OnExit(GameState::Game), despawn_all::<CurrentMap>);
    }
}
