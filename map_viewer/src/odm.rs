use std::error::Error;

use bevy::{
    prelude::*,
    render::render_resource::{Face, PrimitiveTopology},
};
use lod::{
    dtile::TileTable,
    lod_data::LodData,
    odm::{Odm, OdmData},
    LodManager,
};
use random_color::{Luminosity, RandomColor};

pub(super) struct OdmBundle {
    pub map: Odm,
    pub mesh: Mesh,
    pub texture: Image,
    pub models: Vec<ModelBundle>,
    //pub billboards: Vec<BillboardBundle>,
}

pub(super) struct ModelBundle {
    // pub model: Model,
    pub mesh: Mesh,
    pub bounding_box_mesh: Mesh,
    pub material: StandardMaterial,
}

pub(super) struct BillboardBundle {
    // pub model: Billboard,
    pub mesh: Mesh,
    pub texture: Image,
}

impl OdmBundle {
    pub(super) fn new(lod_manager: &LodManager, map_name: &str) -> Result<Self, Box<dyn Error>> {
        let map = LodData::try_from(lod_manager.try_get_bytes(format!("games/{}", &map_name))?)?;
        let map = Odm::try_from(map.data.as_slice())?;

        let tile_table = map.tile_table(lod_manager)?;
        let mesh = Self::generate_terrain_mesh(&map, &tile_table);

        let image =
            bevy::render::texture::Image::from_dynamic(tile_table.atlas_image(lod_manager)?, true);

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
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);
        mesh.set_indices(Some(bevy::render::mesh::Indices::U32(
            odm_data.indices.clone(),
        )));
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, odm_data.positions);
        mesh.duplicate_vertices();
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, odm_data.uvs);

        // let normals = calculate_normals(
        //     mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        //         .unwrap()
        //         .as_float3()
        //         .unwrap(),
        //     &odm_data.indices,
        // );
        //mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);

        mesh.compute_flat_normals();
        mesh.compute_aabb();
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
            unlit: false,
            alpha_mode: AlphaMode::Opaque,
            fog_enabled: true,
            perceptual_roughness: 0.5,
            reflectance: 0.1,
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

fn calculate_terrain_normals(vertices: &[[f32; 3]], indices: &[u32]) -> Vec<[f32; 3]> {
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

pub(super) fn generate_bsp_model_mesh(model: &lod::bsp_model::BSPModel) -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);
    mesh.set_indices(Some(bevy::render::mesh::Indices::U32(
        model.indices.clone(),
    )));
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, model.vertices.clone());
    mesh.duplicate_vertices();
    mesh.compute_flat_normals();
    mesh
}

pub(super) fn generate_bsp_model_bounding_box(model: &lod::bsp_model::BSPModel) -> shape::Box {
    shape::Box::from_corners(
        [
            model.header.bounding_box.min_x as f32,
            model.header.bounding_box.min_z as f32,
            -model.header.bounding_box.min_y as f32,
        ]
        .into(),
        [
            model.header.bounding_box.max_x as f32,
            model.header.bounding_box.max_z as f32,
            -model.header.bounding_box.max_y as f32,
        ]
        .into(),
    )
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
