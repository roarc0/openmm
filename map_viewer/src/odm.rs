use std::error::Error;

use bevy::{prelude::*, render::render_resource::PrimitiveTopology};
use lod::{
    dtile::TileTable,
    lod_data::LodData,
    odm::{Odm, OdmData},
};

pub(super) struct OdmAsset {
    pub map: Odm,
    pub mesh: Mesh,
    pub image: Image,
}

impl OdmAsset {
    pub(super) fn new(settings: &super::world::WorldSettings) -> Result<Self, Box<dyn Error>> {
        let map = LodData::try_from(
            settings
                .lod_manager
                .try_get_bytes(format!("games/{}", &settings.map_name))?,
        )?;
        let map = Odm::try_from(map.data.as_slice())?;

        let tile_table = map.tile_table(&settings.lod_manager)?;
        let mesh = Self::generate_mesh(&map, &tile_table);

        let image = bevy::render::texture::Image::from_dynamic(
            tile_table.atlas_image(&settings.lod_manager)?,
            true,
        );

        Ok(OdmAsset { map, mesh, image })
    }

    pub fn material(&self, image_handle: Handle<Image>) -> StandardMaterial {
        StandardMaterial {
            base_color_texture: Some(image_handle),
            unlit: false,
            alpha_mode: AlphaMode::Opaque,
            fog_enabled: true,
            perceptual_roughness: 1.0,
            reflectance: 0.2,
            ..default()
        }
    }

    fn generate_mesh(odm: &Odm, tile_table: &TileTable) -> Mesh {
        let odm_data = OdmData::new(odm, tile_table);
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);
        mesh.set_indices(Some(bevy::render::mesh::Indices::U32(odm_data.indices)));
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, odm_data.positions);
        mesh.duplicate_vertices();
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, odm_data.uvs);
        mesh.compute_flat_normals();
        mesh.compute_aabb();
        mesh
    }
}

pub(super) fn bsp_model_generate_mesh(model: lod::bsp_model::BSPModel) -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);
    mesh.set_indices(Some(bevy::render::mesh::Indices::U32(model.indices)));
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, model.vertices);
    mesh.duplicate_vertices();
    mesh.compute_flat_normals();
    mesh
}

pub(super) fn bsp_model_bounding_box(model: &lod::bsp_model::BSPModel) -> shape::Box {
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
