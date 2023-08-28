use std::error::Error;

use bevy::{prelude::*, render::render_resource::PrimitiveTopology};
use lod::{
    dtile::{Dtile, TileTable},
    lod_data::LodData,
    odm::{Odm, OdmData},
    LodManager,
};

pub struct OdmAsset {
    pub map: Odm,
    pub mesh: Mesh,
    pub material: StandardMaterial,
}

impl OdmAsset {
    pub fn new(
        mut images: ResMut<Assets<Image>>,
        lod_manager: &LodManager,
        map_name: impl AsRef<str>,
    ) -> Result<Self, Box<dyn Error>> {
        let map =
            LodData::try_from(lod_manager.try_get_bytes(format!("games/{}", map_name.as_ref()))?)?;
        let map = Odm::try_from(map.data.as_slice())?;

        let tile_table = load_tile_table(lod_manager, &map)?;

        let mesh = generate_mesh(&map, &tile_table);

        let image =
            bevy::render::texture::Image::from_dynamic(tile_table.atlas_image(lod_manager)?, true);
        let image_handle = images.add(image);

        let material = StandardMaterial {
            base_color_texture: Some(image_handle),
            unlit: false,
            alpha_mode: AlphaMode::Opaque,
            fog_enabled: true,
            perceptual_roughness: 1.0,
            reflectance: 0.2,
            ..default()
        };

        Ok(OdmAsset {
            map,
            mesh,
            material,
        })
    }
}

fn load_tile_table(lod_manager: &LodManager, map: &Odm) -> Result<TileTable, Box<dyn Error>> {
    let dtile_data = LodData::try_from(lod_manager.try_get_bytes("icons/dtile.bin").unwrap())?;
    Dtile::new(&dtile_data.data).table(map.tile_data)
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
