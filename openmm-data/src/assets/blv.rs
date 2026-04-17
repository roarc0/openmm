//! BLV indoor map parser — faces, rooms, doors, lights, decorations, spawn points.
use byteorder::{LittleEndian, ReadBytesExt};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::io::{Cursor, Seek};

use crate::Assets;
use crate::LodSerialise;
use crate::assets::lod_data::LodData;

use super::blv_types::read_string_lossy;
pub use super::blv_types::*;

/// Parsed BLV indoor map.
#[derive(Debug, Serialize, Deserialize)]
pub struct Blv {
    /// Map display name from the BLV header (offset 0x04, 60 bytes).
    pub header_name: String,
    pub vertices: Vec<BlvVertex>,
    pub faces: Vec<BlvFace>,
    pub texture_names: Vec<String>,
    /// All face-extras records (section 6). Full 36 bytes each, including unknowns.
    pub face_extras: Vec<BlvFaceExtra>,
    /// Texture names for face extras (one per face extra, section 7 of BLV).
    /// Parallel to `face_extras`. May be empty strings.
    pub face_extra_texture_names: Vec<String>,
    pub sectors: Vec<BlvSector>,
    /// Sector light data blob (section 10). Raw u16 values — structure unknown,
    /// preserved for future analysis and round-trip saving.
    pub sector_light_data: Vec<u16>,
    pub decorations: Vec<BlvDecoration>,
    pub lights: Vec<BlvLight>,
    pub bsp_nodes: Vec<BlvBspNode>,
    pub spawn_points: Vec<BlvSpawnPoint>,
    pub map_outlines: Vec<BlvMapOutline>,
    pub door_count: u32,
    pub doors_data_size: i32,
    pub face_data_size: i32,
}

impl Blv {
    /// Parse a BLV file from a LOD archive.
    pub fn load(assets: &Assets, name: &str) -> Result<Self, Box<dyn Error>> {
        let raw = assets.get_bytes(format!("games/{}", name))?;
        Self::try_from(raw.as_slice())
    }

    pub fn parse(data: &[u8]) -> Result<Self, Box<dyn Error>> {
        let mut cursor = Cursor::new(data);

        // 1. Header (136 bytes)
        let header = Self::read_header(&mut cursor)?;

        // 2. Vertices: u32 count, then count x 6 bytes (i16 x, y, z)
        let vertex_count = cursor.read_u32::<LittleEndian>()? as usize;
        let mut vertices = Vec::with_capacity(vertex_count);
        for _ in 0..vertex_count {
            vertices.push(BlvVertex {
                x: cursor.read_i16::<LittleEndian>()?,
                y: cursor.read_i16::<LittleEndian>()?,
                z: cursor.read_i16::<LittleEndian>()?,
            });
        }

        // 3. Faces: u32 count, then count x 80 bytes each (MM6)
        let face_count = cursor.read_u32::<LittleEndian>()? as usize;
        let mut faces = Vec::with_capacity(face_count);
        for _ in 0..face_count {
            faces.push(Self::read_face(&mut cursor)?);
        }

        // 4. Face data blob: (header.face_data_size / 2) x i16
        let face_data_count = (header.face_data_size / 2) as usize;
        let mut face_data_blob = Vec::with_capacity(face_data_count);
        for _ in 0..face_data_count {
            face_data_blob.push(cursor.read_i16::<LittleEndian>()?);
        }
        Self::unpack_face_data(&mut faces, &face_data_blob);

        // 5. Face texture names: faces.len x 10-byte null-terminated strings
        let mut texture_names = Vec::with_capacity(face_count);
        for _ in 0..face_count {
            texture_names.push(read_string_lossy(&mut cursor, 10)?);
        }

        // 6. Face extras: u32 count, then count x 36 bytes each.
        //    Layout: unknown_head[20] | delta_u(i16) | delta_v(i16) | cog_number(i16) | event_id(u16) | unknown_tail[8]
        let face_extras_count = cursor.read_u32::<LittleEndian>()? as usize;
        let mut face_extras: Vec<BlvFaceExtra> = Vec::with_capacity(face_extras_count);
        for _ in 0..face_extras_count {
            let mut unknown_head = [0u8; 20];
            std::io::Read::read_exact(&mut cursor, &mut unknown_head)?;
            let texture_delta_u = cursor.read_i16::<LittleEndian>()?;
            let texture_delta_v = cursor.read_i16::<LittleEndian>()?;
            let cog_number = cursor.read_i16::<LittleEndian>()?;
            let event_id = cursor.read_u16::<LittleEndian>()?;
            let mut unknown_tail = [0u8; 8];
            std::io::Read::read_exact(&mut cursor, &mut unknown_tail)?;
            face_extras.push(BlvFaceExtra {
                unknown_head,
                texture_delta_u,
                texture_delta_v,
                cog_number,
                event_id,
                unknown_tail,
            });
        }
        // Assign deltas, cog_number, and event_id to faces via face_extra_id
        for face in &mut faces {
            if let Some(fe) = face_extras.get(face.face_extra_id as usize) {
                face.texture_delta_u = fe.texture_delta_u;
                face.texture_delta_v = fe.texture_delta_v;
                face.cog_number = fe.cog_number;
                face.event_id = fe.event_id;
            }
        }

        // 7. Face extra texture names: face_extras.len x 10 bytes
        let mut face_extra_texture_names = Vec::with_capacity(face_extras_count);
        for _ in 0..face_extras_count {
            face_extra_texture_names.push(read_string_lossy(&mut cursor, 10)?);
        }

        // 8. Sectors: u32 count, then count x 116 bytes each
        let sector_count = cursor.read_u32::<LittleEndian>()? as usize;
        let mut sectors = Vec::with_capacity(sector_count);
        for _ in 0..sector_count {
            sectors.push(Self::read_sector(&mut cursor)?);
        }

        // 9. Sector data blob: (header.sector_data_size / 2) x u16
        let sector_data_count = (header.sector_data_size / 2) as usize;
        let mut sector_data_blob = Vec::with_capacity(sector_data_count);
        for _ in 0..sector_data_count {
            sector_data_blob.push(cursor.read_u16::<LittleEndian>()?);
        }
        Self::unpack_sector_data(&mut sectors, &sector_data_blob);

        // 10. Sector light data blob: (header.sector_light_data_size / 2) x u16.
        //     Structure unknown — preserved for future analysis and round-trip saving.
        let sector_light_count = (header.sector_light_data_size / 2) as usize;
        let mut sector_light_data = Vec::with_capacity(sector_light_count);
        for _ in 0..sector_light_count {
            sector_light_data.push(cursor.read_u16::<LittleEndian>()?);
        }

        // 11. Door count (actual doors stored in DLV)
        let door_count = cursor.read_u32::<LittleEndian>()?;

        // 12. Decorations: u32 count, then count x 32 bytes
        let decoration_count = cursor.read_u32::<LittleEndian>()? as usize;
        let mut decorations = Vec::with_capacity(decoration_count);
        for _ in 0..decoration_count {
            decorations.push(Self::read_decoration(&mut cursor)?);
        }

        // 13. Decoration names: 32-byte null-terminated name buffers
        //     (MM6 BLV files store 32-byte name fields, verified by binary trace)
        for dec in &mut decorations {
            dec.name = read_string_lossy(&mut cursor, 32)?;
        }

        // 14. Lights: u32 count, then count x 12 bytes (MM6 format)
        let light_count = cursor.read_u32::<LittleEndian>()? as usize;
        let mut lights = Vec::with_capacity(light_count);
        for _ in 0..light_count {
            lights.push(BlvLight {
                position: [
                    cursor.read_i16::<LittleEndian>()?,
                    cursor.read_i16::<LittleEndian>()?,
                    cursor.read_i16::<LittleEndian>()?,
                ],
                radius: cursor.read_i16::<LittleEndian>()?,
                attributes: cursor.read_i16::<LittleEndian>()?,
                brightness: cursor.read_u16::<LittleEndian>()?,
            });
        }

        // 15. BSP nodes: u32 count, then count x 8 bytes
        let bsp_node_count = cursor.read_u32::<LittleEndian>()? as usize;
        let mut bsp_nodes = Vec::with_capacity(bsp_node_count);
        for _ in 0..bsp_node_count {
            bsp_nodes.push(BlvBspNode {
                front: cursor.read_i16::<LittleEndian>()?,
                back: cursor.read_i16::<LittleEndian>()?,
                face_id_offset: cursor.read_i16::<LittleEndian>()?,
                num_faces: cursor.read_i16::<LittleEndian>()?,
            });
        }

        // 16. Spawn points: u32 count, then count x 20 bytes (MM6, no group field)
        let spawn_count = cursor.read_u32::<LittleEndian>()? as usize;
        let mut spawn_points = Vec::with_capacity(spawn_count);
        for _ in 0..spawn_count {
            spawn_points.push(BlvSpawnPoint {
                position: [
                    cursor.read_i32::<LittleEndian>()?,
                    cursor.read_i32::<LittleEndian>()?,
                    cursor.read_i32::<LittleEndian>()?,
                ],
                radius: cursor.read_u16::<LittleEndian>()?,
                spawn_type: cursor.read_u16::<LittleEndian>()?,
                monster_index: cursor.read_u16::<LittleEndian>()?,
                attributes: cursor.read_u16::<LittleEndian>()?,
            });
        }

        // 17. Map outlines: u32 count, then count x 12 bytes
        let outline_count = cursor.read_u32::<LittleEndian>()? as usize;
        let mut map_outlines = Vec::with_capacity(outline_count);
        for _ in 0..outline_count {
            map_outlines.push(BlvMapOutline {
                vertex1_id: cursor.read_u16::<LittleEndian>()?,
                vertex2_id: cursor.read_u16::<LittleEndian>()?,
                face1_id: cursor.read_u16::<LittleEndian>()?,
                face2_id: cursor.read_u16::<LittleEndian>()?,
                z: cursor.read_i16::<LittleEndian>()?,
                flags: cursor.read_u16::<LittleEndian>()?,
            });
        }

        Ok(Blv {
            header_name: header.name,
            vertices,
            faces,
            texture_names,
            face_extras,
            face_extra_texture_names,
            sectors,
            sector_light_data,
            decorations,
            lights,
            bsp_nodes,
            spawn_points,
            map_outlines,
            door_count,
            doors_data_size: header.doors_data_size,
            face_data_size: header.face_data_size,
        })
    }

    fn read_header(cursor: &mut Cursor<&[u8]>) -> Result<BlvHeader, Box<dyn Error>> {
        // 0x00: unknown[4]
        let mut unknown_head = [0u8; 4];
        std::io::Read::read_exact(cursor, &mut unknown_head)?;
        // 0x04: name[60]
        let name = read_string_lossy(cursor, 60)?;
        // 0x40: unknown[40]
        let mut unknown_mid = [0u8; 40];
        std::io::Read::read_exact(cursor, &mut unknown_mid)?;
        // 0x68: four size fields
        let face_data_size = cursor.read_i32::<LittleEndian>()?;
        let sector_data_size = cursor.read_i32::<LittleEndian>()?;
        let sector_light_data_size = cursor.read_i32::<LittleEndian>()?;
        let doors_data_size = cursor.read_i32::<LittleEndian>()?;
        // 0x78: unknown[16]
        let mut unknown_tail = [0u8; 16];
        std::io::Read::read_exact(cursor, &mut unknown_tail)?;
        Ok(BlvHeader {
            name,
            unknown_head,
            unknown_mid,
            face_data_size,
            sector_data_size,
            sector_light_data_size,
            doors_data_size,
            unknown_tail,
        })
    }

    /// Read a single face (80 bytes, MM6 format — no float normals).
    fn read_face(cursor: &mut Cursor<&[u8]>) -> Result<BlvFace, Box<dyn Error>> {
        let mut normal_fixed = [0i32; 4];
        for v in &mut normal_fixed {
            *v = cursor.read_i32::<LittleEndian>()?;
        }
        let mut z_calc = [0i32; 3];
        for v in &mut z_calc {
            *v = cursor.read_i32::<LittleEndian>()?;
        }
        let attributes = cursor.read_u32::<LittleEndian>()?;
        // 6 runtime pointers (24 bytes, zeroed in file)
        cursor.seek(std::io::SeekFrom::Current(24))?;
        let face_extra_id = cursor.read_u16::<LittleEndian>()?;
        let bitmap_id = cursor.read_u16::<LittleEndian>()?;
        let sector_id = cursor.read_u16::<LittleEndian>()?;
        let back_sector_id = cursor.read_i16::<LittleEndian>()?;
        let bbox_min = [
            cursor.read_i16::<LittleEndian>()?,
            cursor.read_i16::<LittleEndian>()?,
            cursor.read_i16::<LittleEndian>()?,
        ];
        let bbox_max = [
            cursor.read_i16::<LittleEndian>()?,
            cursor.read_i16::<LittleEndian>()?,
            cursor.read_i16::<LittleEndian>()?,
        ];
        let polygon_type = cursor.read_u8()?;
        let num_vertices = cursor.read_u8()?;
        let _padding = cursor.read_i16::<LittleEndian>()?;

        Ok(BlvFace {
            normal_fixed,
            z_calc,
            attributes,
            face_extra_id,
            bitmap_id,
            sector_id,
            back_sector_id,
            bbox_min,
            bbox_max,
            polygon_type,
            num_vertices,
            texture_delta_u: 0,
            texture_delta_v: 0,
            vertex_ids: Vec::new(),
            texture_us: Vec::new(),
            texture_vs: Vec::new(),
            event_id: 0,
            cog_number: 0,
        })
    }

    /// Unpack the face data blob into per-face vertex IDs and UV coordinates.
    ///
    /// Each face has 6 sub-arrays of (num_vertices + 1) i16 values:
    /// [vertexIds, xDisp, yDisp, zDisp, textureUs, textureVs].
    /// The +1 is a closing vertex duplicated from the source data (skipped when reading).
    fn unpack_face_data(faces: &mut [BlvFace], blob: &[i16]) {
        let mut offset = 0;
        for face in faces.iter_mut() {
            let n = face.num_vertices as usize;
            let stride = n + 1; // +1 closing vertex per sub-array
            let total = 6 * stride;
            if offset + total > blob.len() {
                break;
            }
            face.vertex_ids = blob[offset..offset + n].iter().map(|&v| v as u16).collect();
            let us_start = offset + 4 * stride;
            face.texture_us = blob[us_start..us_start + n].to_vec();
            let vs_start = offset + 5 * stride;
            face.texture_vs = blob[vs_start..vs_start + n].to_vec();
            offset += total;
        }
    }

    /// Read a sector (116 bytes).
    fn read_sector(cursor: &mut Cursor<&[u8]>) -> Result<BlvSector, Box<dyn Error>> {
        let flags = cursor.read_i32::<LittleEndian>()?;

        // 5 groups of: u16 count, u16 pad, u32 pointer
        let floor_count = cursor.read_u16::<LittleEndian>()?;
        let _pad = cursor.read_u16::<LittleEndian>()?;
        let _ptr = cursor.read_u32::<LittleEndian>()?;
        let wall_count = cursor.read_u16::<LittleEndian>()?;
        let _pad = cursor.read_u16::<LittleEndian>()?;
        let _ptr = cursor.read_u32::<LittleEndian>()?;
        let ceiling_count = cursor.read_u16::<LittleEndian>()?;
        let _pad = cursor.read_u16::<LittleEndian>()?;
        let _ptr = cursor.read_u32::<LittleEndian>()?;
        let fluid_count = cursor.read_u16::<LittleEndian>()?;
        let _pad = cursor.read_u16::<LittleEndian>()?;
        let _ptr = cursor.read_u32::<LittleEndian>()?;
        let portal_count = cursor.read_u16::<LittleEndian>()?;
        let _pad = cursor.read_u16::<LittleEndian>()?;
        let _ptr = cursor.read_u32::<LittleEndian>()?;

        let num_faces = cursor.read_u16::<LittleEndian>()?;
        let num_non_bsp_faces = cursor.read_u16::<LittleEndian>()?;
        let _ptr = cursor.read_u32::<LittleEndian>()?;

        let cylinder_count = cursor.read_u16::<LittleEndian>()?;
        let _pad = cursor.read_u16::<LittleEndian>()?;
        let _ptr = cursor.read_u32::<LittleEndian>()?;
        let cog_count = cursor.read_u16::<LittleEndian>()?;
        let _pad = cursor.read_u16::<LittleEndian>()?;
        let _ptr = cursor.read_u32::<LittleEndian>()?;
        let decoration_count = cursor.read_u16::<LittleEndian>()?;
        let _pad = cursor.read_u16::<LittleEndian>()?;
        let _ptr = cursor.read_u32::<LittleEndian>()?;
        let marker_count = cursor.read_u16::<LittleEndian>()?;
        let _pad = cursor.read_u16::<LittleEndian>()?;
        let _ptr = cursor.read_u32::<LittleEndian>()?;
        let light_count = cursor.read_u16::<LittleEndian>()?;
        let _pad = cursor.read_u16::<LittleEndian>()?;
        let _ptr = cursor.read_u32::<LittleEndian>()?;

        let water_level = cursor.read_i16::<LittleEndian>()?;
        let mist_level = cursor.read_i16::<LittleEndian>()?;
        let light_dist_mul = cursor.read_i16::<LittleEndian>()?;
        let min_ambient_light = cursor.read_i16::<LittleEndian>()?;
        let first_bsp_node = cursor.read_i16::<LittleEndian>()?;
        let exit_tag = cursor.read_i16::<LittleEndian>()?;
        // BBoxs is interleaved: x1, x2, y1, y2, z1, z2 (min/max alternating per axis)
        let x1 = cursor.read_i16::<LittleEndian>()?;
        let x2 = cursor.read_i16::<LittleEndian>()?;
        let y1 = cursor.read_i16::<LittleEndian>()?;
        let y2 = cursor.read_i16::<LittleEndian>()?;
        let z1 = cursor.read_i16::<LittleEndian>()?;
        let z2 = cursor.read_i16::<LittleEndian>()?;

        Ok(BlvSector {
            flags,
            floor_count,
            wall_count,
            ceiling_count,
            fluid_count,
            portal_count,
            num_faces,
            num_non_bsp_faces,
            cylinder_count,
            cog_count,
            decoration_count,
            marker_count,
            light_count,
            water_level,
            mist_level,
            light_dist_mul,
            min_ambient_light,
            first_bsp_node,
            exit_tag,
            bbox_min: [x1, y1, z1],
            bbox_max: [x2, y2, z2],
            face_ids: Vec::new(),
        })
    }

    /// Unpack sector data blob: per-sector face indices.
    fn unpack_sector_data(sectors: &mut [BlvSector], blob: &[u16]) {
        let mut offset = 0;
        for sector in sectors.iter_mut() {
            let total = sector.floor_count as usize
                + sector.wall_count as usize
                + sector.ceiling_count as usize
                + sector.fluid_count as usize
                + sector.portal_count as usize;
            if offset + total > blob.len() {
                break;
            }
            sector.face_ids = blob[offset..offset + total].to_vec();
            offset += total;
        }
    }

    /// Read a decoration (28 bytes, MM6 MapSprite format).
    fn read_decoration(cursor: &mut Cursor<&[u8]>) -> Result<BlvDecoration, Box<dyn Error>> {
        let decoration_desc_id = cursor.read_u16::<LittleEndian>()?;
        let flags = cursor.read_u16::<LittleEndian>()?;
        let position = [
            cursor.read_i32::<LittleEndian>()?,
            cursor.read_i32::<LittleEndian>()?,
            cursor.read_i32::<LittleEndian>()?,
        ];
        let yaw = cursor.read_i32::<LittleEndian>()?;
        let event_variable = cursor.read_i16::<LittleEndian>()?;
        let event = cursor.read_i16::<LittleEndian>()?;
        let trigger_radius = cursor.read_i16::<LittleEndian>()?;
        let direction_degrees = cursor.read_i16::<LittleEndian>()?;
        Ok(BlvDecoration {
            decoration_desc_id,
            flags,
            position,
            yaw,
            event_variable,
            event,
            trigger_radius,
            direction_degrees,
            name: String::new(),
        })
    }
}

impl TryFrom<&[u8]> for Blv {
    type Error = Box<dyn Error>;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        let data = LodData::try_from(data)?;
        Self::parse(&data.data)
    }
}

impl LodSerialise for Blv {
    fn to_bytes(&self) -> Vec<u8> {
        // TODO: Implement full BLV serialization if needed.
        // For now, this is a placeholder to satisfy the trait.
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::{Assets, get_data_path};

    #[test]
    fn parse_d01_blv() {
        let assets = Assets::new(get_data_path()).unwrap();
        let blv = Blv::load(&assets, "d01.blv").unwrap();

        println!("d01.blv:");
        println!("  vertices: {}", blv.vertices.len());
        println!("  faces: {}", blv.faces.len());
        println!("  sectors: {}", blv.sectors.len());
        println!("  texture_names: {}", blv.texture_names.len());
        println!("  decorations: {}", blv.decorations.len());
        println!("  lights: {}", blv.lights.len());
        println!("  bsp_nodes: {}", blv.bsp_nodes.len());
        println!("  spawn_points: {}", blv.spawn_points.len());
        for (i, sp) in blv.spawn_points.iter().enumerate() {
            println!(
                "    sp[{}]: pos={:?} type={} monster={} radius={} attrs=0x{:04x}",
                i, sp.position, sp.spawn_type, sp.monster_index, sp.radius, sp.attributes
            );
        }
        println!("  map_outlines: {}", blv.map_outlines.len());
        println!("  door_count: {}", blv.door_count);

        assert!(!blv.vertices.is_empty(), "should have vertices");
        assert!(!blv.faces.is_empty(), "should have faces");
        assert!(!blv.sectors.is_empty(), "should have sectors");

        // Verify face data was unpacked
        let faces_with_verts = blv.faces.iter().filter(|f| !f.vertex_ids.is_empty()).count();
        println!("  faces with vertex_ids: {}", faces_with_verts);
        assert!(faces_with_verts > 0, "face data blob should be unpacked");

        // Verify sector data was unpacked
        let sectors_with_faces = blv.sectors.iter().filter(|s| !s.face_ids.is_empty()).count();
        println!("  sectors with face_ids: {}", sectors_with_faces);
        assert!(sectors_with_faces > 0, "sector data blob should be unpacked");

        // Print some unique texture names
        let mut unique: Vec<&str> = blv
            .texture_names
            .iter()
            .filter(|s| !s.is_empty())
            .map(|s| s.as_str())
            .collect();
        unique.sort();
        unique.dedup();
        println!("  unique textures ({}):", unique.len());
        for t in unique.iter().take(10) {
            println!("    {}", t);
        }

        // Print a few wall faces to check UV data
        let wall_faces: Vec<_> = blv
            .faces
            .iter()
            .enumerate()
            .filter(|(_, f)| f.polygon_type == 1 && f.num_vertices == 4) // walls with 4 verts
            .take(3)
            .collect();
        println!("  sample wall faces (4-vert quads):");
        for (i, face) in &wall_faces {
            let verts: Vec<_> = face
                .vertex_ids
                .iter()
                .map(|&vid| {
                    let v = &blv.vertices[vid as usize];
                    (v.x, v.y, v.z)
                })
                .collect();
            println!("    face[{}]: vids={:?}", i, &face.vertex_ids);
            println!("      positions: {:?}", verts);
            println!("      Us: {:?}  Vs: {:?}", &face.texture_us, &face.texture_vs);
            println!("      delta_uv: ({}, {})", face.texture_delta_u, face.texture_delta_v);
            let tex = blv.texture_names.get(*i).map(|s| s.as_str()).unwrap_or("?");
            println!("      texture: {} attrs=0x{:08X}", tex, face.attributes);
        }

        // Find door-textured faces
        let door_faces: Vec<_> = blv
            .faces
            .iter()
            .enumerate()
            .filter(|(i, _)| {
                blv.texture_names
                    .get(*i)
                    .map(|t| {
                        let tl = t.to_lowercase();
                        tl.contains("door") || tl.contains("dr") || tl.contains("gate")
                    })
                    .unwrap_or(false)
            })
            .take(5)
            .collect();
        println!("  door-textured faces ({} found, showing 5):", door_faces.len());
        for (i, face) in &door_faces {
            let verts: Vec<_> = face
                .vertex_ids
                .iter()
                .map(|&vid| {
                    let v = &blv.vertices[vid as usize];
                    (v.x, v.y, v.z)
                })
                .collect();
            let tex = blv.texture_names.get(*i).unwrap();
            println!(
                "    face[{}]: nverts={} tex={} attrs=0x{:08X} extra_id={}",
                i, face.num_vertices, tex, face.attributes, face.face_extra_id
            );
            println!("      verts: {:?}", verts);
            println!("      Us: {:?}", &face.texture_us);
            println!("      Vs: {:?}", &face.texture_vs);
            println!("      delta: ({}, {})", face.texture_delta_u, face.texture_delta_v);
        }

        // Print sector bounding boxes
        for (i, s) in blv.sectors.iter().take(5).enumerate() {
            println!(
                "  sector[{}]: bbox min={:?} max={:?} floors={} walls={} ceilings={}",
                i, s.bbox_min, s.bbox_max, s.floor_count, s.wall_count, s.ceiling_count
            );
        }

        // Print decoration info, resolving names via ddeclist
        let ddeclist = crate::assets::ddeclist::DDecList::load(&assets).unwrap();
        println!("  first 20 decorations:");
        for (i, d) in blv.decorations.iter().take(20).enumerate() {
            let resolved = ddeclist
                .items
                .get(d.decoration_desc_id as usize)
                .and_then(|item| item.display_name());
            println!(
                "    [{}] desc_id={} resolved={:?} pos={:?}",
                i, d.decoration_desc_id, resolved, d.position
            );
        }

        // Verify faces have plausible vertex counts
        let max_verts = blv.faces.iter().map(|f| f.num_vertices).max().unwrap_or(0);
        println!("  max face vertex count: {}", max_verts);
        assert!(
            max_verts <= 30,
            "max vertex count should be reasonable (got {})",
            max_verts
        );
    }

    #[test]
    fn parse_sewer_blv() {
        let assets = Assets::new(get_data_path()).unwrap();
        let blv = Blv::load(&assets, "sewer.blv").unwrap();

        println!("sewer.blv:");
        println!("  vertices: {}", blv.vertices.len());
        println!("  faces: {}", blv.faces.len());
        println!("  sectors: {}", blv.sectors.len());
        println!("  decorations: {}", blv.decorations.len());
        println!("  lights: {}", blv.lights.len());
        println!("  bsp_nodes: {}", blv.bsp_nodes.len());
        println!("  spawn_points: {}", blv.spawn_points.len());

        assert!(!blv.vertices.is_empty(), "should have vertices");
        assert!(!blv.faces.is_empty(), "should have faces");
        assert!(!blv.sectors.is_empty(), "should have sectors");
    }

    #[test]
    fn textured_meshes_d01() {
        let assets = Assets::new(get_data_path()).unwrap();
        let blv = Blv::load(&assets, "d01.blv").unwrap();

        let texture_sizes = HashMap::new();
        let meshes = blv.textured_meshes(&texture_sizes, &std::collections::HashSet::new());

        println!("d01.blv textured meshes: {}", meshes.len());
        assert!(!meshes.is_empty(), "should produce textured meshes");

        let total_tris: usize = meshes.iter().map(|m| m.positions.len() / 3).sum();
        println!("  total triangles: {}", total_tris);
        assert!(total_tris > 0, "should have triangles");

        for mesh in meshes.iter().take(5) {
            println!("  texture '{}': {} tris", mesh.texture_name, mesh.positions.len() / 3);
        }
    }

    #[test]
    fn face_extras_event_id() {
        let assets = Assets::new(get_data_path()).unwrap();
        let blv = Blv::load(&assets, "d01.blv").unwrap();

        let clickable: Vec<_> = blv
            .faces
            .iter()
            .enumerate()
            .filter(|(_, f)| f.is_clickable() && f.event_id != 0)
            .collect();
        println!("d01.blv clickable faces with events: {}", clickable.len());
        for (i, face) in clickable.iter().take(10) {
            println!(
                "  face[{}]: event_id={} cog={} attrs=0x{:08X}",
                i, face.event_id, face.cog_number, face.attributes
            );
        }
        assert!(!clickable.is_empty(), "d01 should have clickable faces with event IDs");

        // Dump door-related faces (moves_by_door flag)
        let door_faces: Vec<_> = blv
            .faces
            .iter()
            .enumerate()
            .filter(|(_, f)| f.moves_by_door())
            .collect();
        println!("\nd01.blv moves_by_door faces: {}", door_faces.len());
        for (i, face) in door_faces.iter().take(20) {
            let tex = blv.texture_names.get(*i).map(|s| s.as_str()).unwrap_or("?");
            println!(
                "  face[{}]: cog={} event_id={} tex={} nverts={} attrs=0x{:08X}",
                i, face.cog_number, face.event_id, tex, face.num_vertices, face.attributes
            );
        }

        // Show unique cog numbers for door faces
        let mut door_cogs: Vec<i16> = door_faces.iter().map(|(_, f)| f.cog_number).collect();
        door_cogs.sort();
        door_cogs.dedup();
        println!("\nUnique cog numbers on door faces: {:?}", door_cogs);
    }

    #[test]
    fn initialize_doors_d01() {
        let assets = Assets::new(get_data_path()).unwrap();
        let blv = Blv::load(&assets, "d01.blv").unwrap();
        let dlv = crate::assets::dlv::Dlv::new(&assets, "d01.blv", blv.door_count, blv.doors_data_size).unwrap();

        // Check cog_numbers on all faces
        let faces_with_cog: Vec<_> = blv
            .faces
            .iter()
            .enumerate()
            .filter(|(_, f)| f.cog_number != 0)
            .collect();
        println!("Faces with non-zero cog_number: {}", faces_with_cog.len());
        for (i, f) in faces_with_cog.iter().take(20) {
            let tex = blv.texture_names.get(*i).map(|s| s.as_str()).unwrap_or("?");
            println!(
                "  face[{}]: cog={} attrs=0x{:08X} tex={} moves_by_door={}",
                i,
                f.cog_number,
                f.attributes,
                tex,
                f.moves_by_door()
            );
        }

        // Check MOVES_BY_DOOR flag
        let mbd_count = blv.faces.iter().filter(|f| f.moves_by_door()).count();
        println!("\nFaces with MOVES_BY_DOOR flag: {}", mbd_count);

        // Dump DLV door data for first non-empty door
        println!("\nDLV door data (non-empty):");
        for (i, d) in dlv.doors.iter().enumerate() {
            if d.face_ids.is_empty() {
                continue;
            }
            println!(
                "  door[{}]: id={} dir={:?} move_len={} speed=({},{})",
                i, d.door_id, d.direction, d.move_length, d.open_speed, d.close_speed
            );
            println!("    face_ids: {:?}", &d.face_ids);
            println!("    vertex_ids: {:?}", &d.vertex_ids);
            println!("    x_offsets: {:?}", &d.x_offsets);
            println!("    y_offsets: {:?}", &d.y_offsets);
            println!("    z_offsets: {:?}", &d.z_offsets);
            // Compare offsets to BLV vertices
            for (vi, &vid) in d.vertex_ids.iter().enumerate() {
                if let Some(v) = blv.vertices.get(vid as usize) {
                    let matches = d.x_offsets[vi] == v.x && d.y_offsets[vi] == v.y && d.z_offsets[vi] == v.z;
                    println!(
                        "    vert[{}] id={}: blv=({},{},{}) offset=({},{},{}) match={}",
                        vi, vid, v.x, v.y, v.z, d.x_offsets[vi], d.y_offsets[vi], d.z_offsets[vi], matches
                    );
                }
            }
            if i > 5 {
                break;
            } // Limit output
        }

        // Verify door_face_set includes ALL faces from door face_ids
        // (no filtering — the original engine moves all door-referenced faces)
        let door_set = Blv::door_face_set(&dlv.doors, &blv.faces);
        let all_face_ids: std::collections::HashSet<u16> =
            dlv.doors.iter().flat_map(|d| d.face_ids.iter().copied()).collect();
        let included = all_face_ids
            .iter()
            .filter(|&&fid| door_set.contains(&(fid as usize)))
            .count();
        println!(
            "\nDoor face set: {} included out of {} total door face_ids",
            included,
            all_face_ids.len()
        );
        // Only faces appearing > 1 time in any single door's face_ids are included.
        // Single-occurrence faces (one corner shared with door panel) stay in static geometry.
        // Verify: included count must be <= total unique face count with vertex data.
        let faces_with_vids = all_face_ids
            .iter()
            .filter(|&&fid| {
                blv.faces
                    .get(fid as usize)
                    .map(|f| !f.vertex_ids.is_empty())
                    .unwrap_or(false)
            })
            .count();
        assert!(
            included <= faces_with_vids,
            "door_face_set must not include faces without vertex data (included={}, with_vids={})",
            included,
            faces_with_vids
        );
        assert!(included > 0, "door_face_set should include at least some door faces");
    }

    #[test]
    fn hybrid_door_faces_d01() {
        let assets = Assets::new(get_data_path()).unwrap();
        let blv = Blv::load(&assets, "d01.blv").unwrap();
        let dlv = crate::assets::dlv::Dlv::new(&assets, "d01.blv", blv.door_count, blv.doors_data_size).unwrap();

        let mut hybrid_count = 0;
        let mut panel_count = 0;
        let mut frame_count = 0;

        for (di, door) in dlv.doors.iter().enumerate() {
            if door.face_ids.is_empty() {
                continue;
            }
            let moving: std::collections::HashSet<u16> = door.vertex_ids.iter().copied().collect();
            let mut seen = std::collections::HashSet::new();
            for &fid in &door.face_ids {
                let fi = fid as usize;
                if !seen.insert(fi) {
                    continue;
                }
                let Some(face) = blv.faces.get(fi) else { continue };
                if face.vertex_ids.is_empty() {
                    continue;
                }
                let moving_in_face: Vec<u16> = face.vertex_ids.iter().filter(|v| moving.contains(v)).copied().collect();
                let is_hybrid = !moving_in_face.is_empty() && moving_in_face.len() < face.vertex_ids.len();
                let is_pure_panel = face.vertex_ids.iter().all(|v| moving.contains(v));
                let tex = blv.texture_names.get(fi).map(|s| s.as_str()).unwrap_or("?");
                if is_hybrid {
                    hybrid_count += 1;
                    println!(
                        "HYBRID door[{}](id={}) face[{}] poly={} tex={} vids={:?} moving={:?}",
                        di, door.door_id, fi, face.polygon_type, tex, &face.vertex_ids, &moving_in_face
                    );
                } else if is_pure_panel {
                    panel_count += 1;
                } else {
                    frame_count += 1;
                }
            }
        }
        println!("panel={} frame={} hybrid={}", panel_count, frame_count, hybrid_count);
        // After door_face_set filters single-occurrence faces (1-moving-vertex accidentals),
        // all remaining hybrid faces should have >= 2 moving vertices. Verify no 1-vertex
        // hybrids slipped through (they would deform large room floor/ceiling faces).
        let bad = dlv
            .doors
            .iter()
            .enumerate()
            .flat_map(|(di, door)| {
                if door.face_ids.is_empty() {
                    return vec![];
                }
                let moving: std::collections::HashSet<u16> = door.vertex_ids.iter().copied().collect();
                let door_set = Blv::door_face_set(&dlv.doors, &blv.faces);
                let mut seen = std::collections::HashSet::new();
                let mut bad = vec![];
                for &fid in &door.face_ids {
                    let fi = fid as usize;
                    if !seen.insert(fi) || !door_set.contains(&fi) {
                        continue;
                    }
                    let Some(face) = blv.faces.get(fi) else {
                        continue;
                    };
                    let moving_in_face: Vec<u16> =
                        face.vertex_ids.iter().filter(|v| moving.contains(v)).copied().collect();
                    if moving_in_face.len() == 1 {
                        bad.push((di, door.door_id, fi, moving_in_face.len()));
                    }
                }
                bad
            })
            .collect::<Vec<_>>();
        assert!(
            bad.is_empty(),
            "door_face_set contains 1-moving-vertex faces that deform room geometry: {:?}",
            bad
        );
    }
}
