# BLV Indoor Map Loading — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Parse BLV indoor map geometry and render it in-engine, with `--map d01` CLI support for loading any indoor map.

**Architecture:** BLV files contain face-based geometry (vertices + textured polygonal faces organized into sectors). We parse the BLV binary format into a `Blv` struct, extract per-texture meshes using the same fan-triangulation pattern as outdoor BSP models, and introduce a `MapName` enum to let the loading pipeline branch between outdoor (ODM) and indoor (BLV) paths. DLV delta files are parsed for actor data (same format as DDM actors). Sector-based visibility and doors are deferred — we render all visible faces with ambient lighting.

**Tech Stack:** Rust, byteorder (binary parsing), Bevy 0.18 (rendering), existing openmm-data crate patterns.

---

## File Map

| Action | File | Responsibility |
|--------|------|----------------|
| Create | `openmm-data/src/blv.rs` | BLV binary parser: header, vertices, faces, face data, textures, sectors, decorations, lights, BSP nodes, spawn points, map outlines |
| Create | `openmm-data/src/dlv.rs` | DLV binary parser: actors (reuses DdmActor struct) |
| Modify | `openmm-data/src/lib.rs` | Export `blv` and `dlv` modules |
| Create | `openmm/src/game/map_name.rs` | `MapName` enum (Outdoor/Indoor), parsing from string, Display |
| Modify | `openmm/src/states/loading.rs` | `LoadRequest` uses `MapName`; add indoor loading branch |
| Modify | `openmm/src/game/odm.rs` | Move `OdmName` into `map_name.rs`, re-export for compatibility |
| Modify | `openmm/src/game/debug.rs` | Use `MapName` in `CurrentMapName` |
| Modify | `openmm/src/game/mod.rs` | Register `map_name` module |

---

### Task 1: BLV Parser — Vertices and Faces

**Files:**
- Create: `openmm-data/src/blv.rs`
- Modify: `openmm-data/src/lib.rs`

- [ ] **Step 1: Create `openmm-data/src/blv.rs` with header + vertex + face parsing**

```rust
use std::error::Error;
use std::io::{Cursor, Read, Seek, SeekFrom};
use byteorder::{LittleEndian, ReadBytesExt};
use crate::{lod_data::LodData, utils::try_read_string_block, LodManager};

const BLV_HEADER_SIZE: usize = 136;
const BLV_FACE_SIZE: usize = 96;
const FACE_TEXTURE_NAME_SIZE: usize = 10;
const FACE_EXTRA_SIZE: usize = 36;
const FACE_EXTRA_TEXTURE_NAME_SIZE: usize = 10;
const SECTOR_SIZE: usize = 116;
const DECORATION_SIZE: usize = 32;
const DECORATION_NAME_SIZE: usize = 32;
const LIGHT_SIZE_MM6: usize = 12;
const BSP_NODE_SIZE: usize = 8;
const SPAWN_POINT_SIZE_MM6: usize = 20;
const MAP_OUTLINE_SIZE: usize = 12;

/// BLV file header (136 bytes).
#[derive(Debug)]
struct BlvHeader {
    face_data_size: i32,
    sector_data_size: i32,
    sector_light_data_size: i32,
    doors_data_size: i32,
}

/// A vertex in MM6 coordinates (i16 each).
#[derive(Debug, Clone, Copy)]
pub struct BlvVertex {
    pub x: i16,
    pub y: i16,
    pub z: i16,
}

/// A face in the BLV file (96 bytes on disk, plus variable-length data from the face data blob).
#[derive(Debug)]
pub struct BlvFace {
    /// Float plane normal (x, y, z) and distance.
    pub normal: [f32; 4],
    /// Fixed-point plane normal and distance (for collision).
    pub normal_fixed: [i32; 4],
    /// Z calculation coefficients (fixed-point).
    pub z_calc: [i32; 3],
    /// Face attribute flags.
    pub attributes: u32,
    /// Index into face extras array.
    pub face_extra_id: u16,
    /// Texture bitmap ID (index, not used directly — we use the texture name).
    pub bitmap_id: u16,
    /// Front sector index.
    pub sector_id: u16,
    /// Back sector index (for portals).
    pub back_sector_id: i16,
    /// Bounding box (i16: x1, x2, y1, y2, z1, z2).
    pub bounding: [i16; 6],
    /// Polygon type (0=invalid, 1=wall, 3=floor, 5=ceiling, etc.).
    pub polygon_type: u8,
    /// Number of vertices in this face.
    pub num_vertices: u8,

    // Variable-length data from face data blob (populated after reading):
    /// Vertex indices into the vertex array.
    pub vertex_ids: Vec<u16>,
    /// U texture coordinates (in texture pixels).
    pub texture_us: Vec<i16>,
    /// V texture coordinates (in texture pixels).
    pub texture_vs: Vec<i16>,

    /// Texture name from the per-face texture name array.
    pub texture_name: String,
}

impl BlvFace {
    pub fn is_portal(&self) -> bool { (self.attributes & 0x0001) != 0 }
    pub fn is_invisible(&self) -> bool { (self.attributes & 0x2000) != 0 }
    pub fn is_sky(&self) -> bool { (self.attributes & 0x00100000) != 0 }
    pub fn is_floor(&self) -> bool { self.polygon_type == 3 || self.polygon_type == 4 }
}

/// A sector (room) in the BLV file.
#[derive(Debug)]
pub struct BlvSector {
    pub flags: i32,
    pub num_floors: u16,
    pub num_walls: u16,
    pub num_ceilings: u16,
    pub num_fluids: u16,
    pub num_portals: i16,
    pub num_faces: u16,
    pub num_non_bsp_faces: u16,
    pub num_decorations: u16,
    pub num_lights: u16,
    pub min_ambient_light: i16,
    pub first_bsp_node: i16,
    pub bounding_box: [i16; 6],

    // Populated from sector data blob:
    pub floor_face_ids: Vec<u16>,
    pub wall_face_ids: Vec<u16>,
    pub ceiling_face_ids: Vec<u16>,
    pub portal_face_ids: Vec<u16>,
    pub face_ids: Vec<u16>,
    pub decoration_ids: Vec<u16>,
    pub light_ids: Vec<u16>,
}

/// A decoration (object/billboard) placed in the indoor map.
#[derive(Debug)]
pub struct BlvDecoration {
    pub decoration_desc_id: u16,
    pub flags: u16,
    pub position: [i32; 3],
    pub yaw: i32,
    pub name: String,
}

/// A light source in the indoor map (MM6 format, 12 bytes).
#[derive(Debug)]
pub struct BlvLight {
    pub position: [i16; 3],
    pub radius: i16,
    pub attributes: i16,
    pub brightness: u16,
}

/// A BSP node for indoor sector traversal.
#[derive(Debug)]
pub struct BlvBspNode {
    pub front: i16,
    pub back: i16,
    pub face_id_offset: i16,
    pub num_faces: i16,
}

/// A spawn point in the indoor map (MM6 format, 20 bytes).
#[derive(Debug)]
pub struct BlvSpawnPoint {
    pub position: [i32; 3],
    pub radius: u16,
    pub spawn_type: u16,
    pub monster_index: u16,
    pub attributes: u16,
}

/// A map outline edge for the automap.
#[derive(Debug)]
pub struct BlvMapOutline {
    pub vertex1_id: u16,
    pub vertex2_id: u16,
    pub face1_id: u16,
    pub face2_id: u16,
    pub z: i16,
    pub flags: u16,
}

/// Parsed BLV indoor map file.
#[derive(Debug)]
pub struct Blv {
    pub vertices: Vec<BlvVertex>,
    pub faces: Vec<BlvFace>,
    pub sectors: Vec<BlvSector>,
    pub decorations: Vec<BlvDecoration>,
    pub lights: Vec<BlvLight>,
    pub bsp_nodes: Vec<BlvBspNode>,
    pub spawn_points: Vec<BlvSpawnPoint>,
    pub map_outlines: Vec<BlvMapOutline>,
    pub door_count: u32,
}
```

- [ ] **Step 2: Implement `Blv::new()` and the full parsing pipeline**

```rust
impl Blv {
    pub fn new(lod_manager: &LodManager, name: &str) -> Result<Self, Box<dyn Error>> {
        let blv_name = if name.ends_with(".blv") {
            name.to_string()
        } else {
            format!("{}.blv", name)
        };
        let raw = lod_manager.try_get_bytes(&format!("games/{}", blv_name))?;
        let data = LodData::try_from(raw)?;
        Self::parse(&data.data)
    }

    fn parse(data: &[u8]) -> Result<Self, Box<dyn Error>> {
        let mut cursor = Cursor::new(data);

        // 1. Header (136 bytes)
        let header = Self::read_header(&mut cursor)?;

        // 2. Vertices
        let vertex_count = cursor.read_u32::<LittleEndian>()? as usize;
        let mut vertices = Vec::with_capacity(vertex_count);
        for _ in 0..vertex_count {
            vertices.push(BlvVertex {
                x: cursor.read_i16::<LittleEndian>()?,
                y: cursor.read_i16::<LittleEndian>()?,
                z: cursor.read_i16::<LittleEndian>()?,
            });
        }

        // 3. Faces (96 bytes each)
        let face_count = cursor.read_u32::<LittleEndian>()? as usize;
        let mut faces = Vec::with_capacity(face_count);
        for _ in 0..face_count {
            faces.push(Self::read_face(&mut cursor)?);
        }

        // 4. Face data blob — variable-length vertex/UV data per face
        let face_data_count = (header.face_data_size / 2) as usize;
        let mut face_data = Vec::with_capacity(face_data_count);
        for _ in 0..face_data_count {
            face_data.push(cursor.read_i16::<LittleEndian>()?);
        }
        Self::assign_face_data(&mut faces, &face_data);

        // 5. Face texture names (10 bytes each, one per face)
        for face in &mut faces {
            face.texture_name = try_read_string_block(&mut cursor, FACE_TEXTURE_NAME_SIZE)?;
        }

        // 6. Face extras
        let face_extra_count = cursor.read_u32::<LittleEndian>()? as usize;
        // Skip face extras for now (36 bytes each)
        cursor.seek(SeekFrom::Current((face_extra_count * FACE_EXTRA_SIZE) as i64))?;

        // 7. Face extra texture names (10 bytes each)
        cursor.seek(SeekFrom::Current((face_extra_count * FACE_EXTRA_TEXTURE_NAME_SIZE) as i64))?;

        // 8. Sectors (116 bytes each)
        let sector_count = cursor.read_u32::<LittleEndian>()? as usize;
        let mut sectors = Vec::with_capacity(sector_count);
        for _ in 0..sector_count {
            sectors.push(Self::read_sector(&mut cursor)?);
        }

        // 9. Sector data blob — face indices per sector
        let sector_data_count = (header.sector_data_size / 2) as usize;
        let mut sector_data = Vec::with_capacity(sector_data_count);
        for _ in 0..sector_data_count {
            sector_data.push(cursor.read_u16::<LittleEndian>()?);
        }
        Self::assign_sector_data(&mut sectors, &sector_data);

        // 10. Sector light data blob
        let sector_light_count = (header.sector_light_data_size / 2) as usize;
        let mut sector_light_data = Vec::with_capacity(sector_light_count);
        for _ in 0..sector_light_count {
            sector_light_data.push(cursor.read_u16::<LittleEndian>()?);
        }
        Self::assign_sector_light_data(&mut sectors, &sector_light_data);

        // 11. Door count (actual doors in DLV)
        let door_count = cursor.read_u32::<LittleEndian>()?;
        // Skip door data blob (doorsDataSizeBytes from header)
        // Doors are in DLV, BLV just stores the byte size reservation

        // 12. Decorations (32 bytes each)
        let decoration_count = cursor.read_u32::<LittleEndian>()? as usize;
        let mut decorations = Vec::with_capacity(decoration_count);
        for _ in 0..decoration_count {
            decorations.push(Self::read_decoration(&mut cursor)?);
        }

        // 13. Decoration names (32 bytes each)
        for dec in &mut decorations {
            dec.name = try_read_string_block(&mut cursor, DECORATION_NAME_SIZE)?;
        }

        // 14. Lights (12 bytes each for MM6)
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

        // 15. BSP nodes (8 bytes each)
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

        // 16. Spawn points (20 bytes each for MM6)
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

        // 17. Map outlines (12 bytes each)
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
            vertices,
            faces,
            sectors,
            decorations,
            lights,
            bsp_nodes,
            spawn_points,
            map_outlines,
            door_count,
        })
    }

    fn read_header(cursor: &mut Cursor<&[u8]>) -> Result<BlvHeader, Box<dyn Error>> {
        // Skip first 104 bytes of padding
        cursor.seek(SeekFrom::Current(104))?;
        let face_data_size = cursor.read_i32::<LittleEndian>()?;
        let sector_data_size = cursor.read_i32::<LittleEndian>()?;
        let sector_light_data_size = cursor.read_i32::<LittleEndian>()?;
        let doors_data_size = cursor.read_i32::<LittleEndian>()?;
        // Skip last 16 bytes of padding
        cursor.seek(SeekFrom::Current(16))?;
        Ok(BlvHeader {
            face_data_size,
            sector_data_size,
            sector_light_data_size,
            doors_data_size,
        })
    }

    fn read_face(cursor: &mut Cursor<&[u8]>) -> Result<BlvFace, Box<dyn Error>> {
        let normal = [
            cursor.read_f32::<LittleEndian>()?,
            cursor.read_f32::<LittleEndian>()?,
            cursor.read_f32::<LittleEndian>()?,
            cursor.read_f32::<LittleEndian>()?,
        ];
        let normal_fixed = [
            cursor.read_i32::<LittleEndian>()?,
            cursor.read_i32::<LittleEndian>()?,
            cursor.read_i32::<LittleEndian>()?,
            cursor.read_i32::<LittleEndian>()?,
        ];
        let z_calc = [
            cursor.read_i32::<LittleEndian>()?,
            cursor.read_i32::<LittleEndian>()?,
            cursor.read_i32::<LittleEndian>()?,
        ];
        let attributes = cursor.read_u32::<LittleEndian>()?;
        // Skip 6 runtime pointers (6 × 4 = 24 bytes)
        cursor.seek(SeekFrom::Current(24))?;
        let face_extra_id = cursor.read_u16::<LittleEndian>()?;
        let bitmap_id = cursor.read_u16::<LittleEndian>()?;
        let sector_id = cursor.read_u16::<LittleEndian>()?;
        let back_sector_id = cursor.read_i16::<LittleEndian>()?;
        let bounding = [
            cursor.read_i16::<LittleEndian>()?,
            cursor.read_i16::<LittleEndian>()?,
            cursor.read_i16::<LittleEndian>()?,
            cursor.read_i16::<LittleEndian>()?,
            cursor.read_i16::<LittleEndian>()?,
            cursor.read_i16::<LittleEndian>()?,
        ];
        let polygon_type = cursor.read_u8()?;
        let num_vertices = cursor.read_u8()?;
        let _pad = cursor.read_i16::<LittleEndian>()?;

        Ok(BlvFace {
            normal,
            normal_fixed,
            z_calc,
            attributes,
            face_extra_id,
            bitmap_id,
            sector_id,
            back_sector_id,
            bounding,
            polygon_type,
            num_vertices,
            vertex_ids: Vec::new(),
            texture_us: Vec::new(),
            texture_vs: Vec::new(),
            texture_name: String::new(),
        })
    }

    /// Unpack the face data blob into per-face vertex IDs and UV coordinates.
    /// For each face, 6 sub-arrays of (num_vertices + 1) i16 values are packed:
    /// vertexIds, xDisplacements, yDisplacements, zDisplacements, textureUs, textureVs.
    fn assign_face_data(faces: &mut [BlvFace], data: &[i16]) {
        let mut offset = 0;
        for face in faces.iter_mut() {
            let n = face.num_vertices as usize + 1;
            let chunk = 6 * n;
            if offset + chunk > data.len() { break; }
            let slice = &data[offset..offset + chunk];
            face.vertex_ids = slice[..n - 1].iter().map(|&v| v as u16).collect();
            // Skip x/y/z displacements (3 × n values)
            let us_start = 4 * n;
            let vs_start = 5 * n;
            face.texture_us = slice[us_start..us_start + n - 1].to_vec();
            face.texture_vs = slice[vs_start..vs_start + n - 1].to_vec();
            offset += chunk;
        }
    }

    fn read_sector(cursor: &mut Cursor<&[u8]>) -> Result<BlvSector, Box<dyn Error>> {
        let flags = cursor.read_i32::<LittleEndian>()?;
        let num_floors = cursor.read_u16::<LittleEndian>()?;
        cursor.seek(SeekFrom::Current(2))?; // pad
        cursor.seek(SeekFrom::Current(4))?; // pointer
        let num_walls = cursor.read_u16::<LittleEndian>()?;
        cursor.seek(SeekFrom::Current(2))?;
        cursor.seek(SeekFrom::Current(4))?;
        let num_ceilings = cursor.read_u16::<LittleEndian>()?;
        cursor.seek(SeekFrom::Current(2))?;
        cursor.seek(SeekFrom::Current(4))?;
        let num_fluids = cursor.read_u16::<LittleEndian>()?;
        cursor.seek(SeekFrom::Current(2))?;
        cursor.seek(SeekFrom::Current(4))?;
        let num_portals = cursor.read_i16::<LittleEndian>()?;
        cursor.seek(SeekFrom::Current(2))?;
        cursor.seek(SeekFrom::Current(4))?;
        let num_faces = cursor.read_u16::<LittleEndian>()?;
        let num_non_bsp_faces = cursor.read_u16::<LittleEndian>()?;
        cursor.seek(SeekFrom::Current(4))?; // pointer
        let _num_cylinder = cursor.read_u16::<LittleEndian>()?;
        cursor.seek(SeekFrom::Current(2))?;
        cursor.seek(SeekFrom::Current(4))?;
        let _num_cogs = cursor.read_u16::<LittleEndian>()?;
        cursor.seek(SeekFrom::Current(2))?;
        cursor.seek(SeekFrom::Current(4))?;
        let num_decorations = cursor.read_u16::<LittleEndian>()?;
        cursor.seek(SeekFrom::Current(2))?;
        cursor.seek(SeekFrom::Current(4))?;
        let _num_markers = cursor.read_u16::<LittleEndian>()?;
        cursor.seek(SeekFrom::Current(2))?;
        cursor.seek(SeekFrom::Current(4))?;
        let num_lights = cursor.read_u16::<LittleEndian>()?;
        cursor.seek(SeekFrom::Current(2))?;
        cursor.seek(SeekFrom::Current(4))?;
        let _water_level = cursor.read_i16::<LittleEndian>()?;
        let _mist_level = cursor.read_i16::<LittleEndian>()?;
        let _light_dist_mul = cursor.read_i16::<LittleEndian>()?;
        let min_ambient_light = cursor.read_i16::<LittleEndian>()?;
        let first_bsp_node = cursor.read_i16::<LittleEndian>()?;
        let _exit_tag = cursor.read_i16::<LittleEndian>()?;
        let bounding_box = [
            cursor.read_i16::<LittleEndian>()?,
            cursor.read_i16::<LittleEndian>()?,
            cursor.read_i16::<LittleEndian>()?,
            cursor.read_i16::<LittleEndian>()?,
            cursor.read_i16::<LittleEndian>()?,
            cursor.read_i16::<LittleEndian>()?,
        ];

        Ok(BlvSector {
            flags,
            num_floors,
            num_walls,
            num_ceilings,
            num_fluids,
            num_portals,
            num_faces,
            num_non_bsp_faces,
            num_decorations,
            num_lights,
            min_ambient_light,
            first_bsp_node,
            bounding_box,
            floor_face_ids: Vec::new(),
            wall_face_ids: Vec::new(),
            ceiling_face_ids: Vec::new(),
            portal_face_ids: Vec::new(),
            face_ids: Vec::new(),
            decoration_ids: Vec::new(),
            light_ids: Vec::new(),
        })
    }

    fn assign_sector_data(sectors: &mut [BlvSector], data: &[u16]) {
        let mut offset = 0;
        for sector in sectors.iter_mut() {
            let read = |off: &mut usize, count: usize| -> Vec<u16> {
                let end = (*off + count).min(data.len());
                let result = data[*off..end].to_vec();
                *off += count;
                result
            };
            sector.floor_face_ids = read(&mut offset, sector.num_floors as usize);
            sector.wall_face_ids = read(&mut offset, sector.num_walls as usize);
            sector.ceiling_face_ids = read(&mut offset, sector.num_ceilings as usize);
            // Skip fluids (always 0)
            offset += sector.num_fluids as usize;
            sector.portal_face_ids = read(&mut offset, sector.num_portals.max(0) as usize);
            sector.face_ids = read(&mut offset, sector.num_faces as usize);
            // Skip cylinder faces (always 0) + cogs (always 0)
            offset += 0; // numCylinder always 0
            offset += 0; // numCogs always 0
            sector.decoration_ids = read(&mut offset, sector.num_decorations as usize);
            // Skip markers (always 0)
        }
    }

    fn assign_sector_light_data(sectors: &mut [BlvSector], data: &[u16]) {
        let mut offset = 0;
        for sector in sectors.iter_mut() {
            let count = sector.num_lights as usize;
            let end = (offset + count).min(data.len());
            sector.light_ids = data[offset..end].to_vec();
            offset += count;
        }
    }

    fn read_decoration(cursor: &mut Cursor<&[u8]>) -> Result<BlvDecoration, Box<dyn Error>> {
        let decoration_desc_id = cursor.read_u16::<LittleEndian>()?;
        let flags = cursor.read_u16::<LittleEndian>()?;
        let position = [
            cursor.read_i32::<LittleEndian>()?,
            cursor.read_i32::<LittleEndian>()?,
            cursor.read_i32::<LittleEndian>()?,
        ];
        let yaw = cursor.read_i32::<LittleEndian>()?;
        // Skip remaining fields (12 bytes)
        cursor.seek(SeekFrom::Current(12))?;
        Ok(BlvDecoration {
            decoration_desc_id,
            flags,
            position,
            yaw,
            name: String::new(),
        })
    }
}
```

- [ ] **Step 3: Add `textured_meshes()` method for rendering**

This converts BLV faces into per-texture mesh data, using the same pattern as `BSPModel::textured_meshes()`.

```rust
use crate::odm::mm6_to_bevy;

/// A per-texture mesh extracted from BLV faces, ready for rendering.
pub struct BlvTexturedMesh {
    pub texture_name: String,
    pub positions: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub normals: Vec<[f32; 3]>,
}

impl Blv {
    /// Extract per-texture meshes from all visible faces.
    /// `texture_sizes` maps texture name → (width, height) in pixels.
    pub fn textured_meshes(
        &self,
        texture_sizes: &std::collections::HashMap<String, (u32, u32)>,
    ) -> Vec<BlvTexturedMesh> {
        let mut meshes: std::collections::HashMap<String, BlvTexturedMesh> =
            std::collections::HashMap::new();

        for face in &self.faces {
            if face.num_vertices < 3 { continue; }
            if face.is_invisible() || face.is_portal() { continue; }
            if face.texture_name.is_empty() { continue; }

            let (tex_w, tex_h) = texture_sizes
                .get(&face.texture_name)
                .copied()
                .unwrap_or((128, 128));

            // Face normal: BLV stores float normal in MM6 coords (x, y, z)
            // Convert to Bevy: (x, z, -y)
            let normal = [face.normal[0], face.normal[2], -face.normal[1]];

            let mesh = meshes
                .entry(face.texture_name.clone())
                .or_insert_with(|| BlvTexturedMesh {
                    texture_name: face.texture_name.clone(),
                    positions: Vec::new(),
                    uvs: Vec::new(),
                    normals: Vec::new(),
                });

            // Fan triangulation: (v0, v1, v2), (v0, v2, v3), ...
            for i in 0..(face.num_vertices as usize).saturating_sub(2) {
                let tri = [0, i + 1, i + 2];
                for &vi in &tri {
                    let vert_idx = face.vertex_ids.get(vi).copied().unwrap_or(0) as usize;
                    let pos = if vert_idx < self.vertices.len() {
                        let v = &self.vertices[vert_idx];
                        mm6_to_bevy(v.x as i32, v.y as i32, v.z as i32)
                    } else {
                        [0.0, 0.0, 0.0]
                    };
                    mesh.positions.push(pos);

                    let u = face.texture_us.get(vi).copied().unwrap_or(0) as f32 / tex_w as f32;
                    let v = face.texture_vs.get(vi).copied().unwrap_or(0) as f32 / tex_h as f32;
                    mesh.uvs.push([u, v]);
                    mesh.normals.push(normal);
                }
            }
        }

        meshes.into_values().collect()
    }
}
```

- [ ] **Step 4: Add test for BLV parsing**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{get_lod_path, LodManager};

    #[test]
    fn parse_d01_blv() {
        let lod_manager = LodManager::new(get_lod_path()).unwrap();
        let blv = Blv::new(&lod_manager, "d01").unwrap();
        assert!(!blv.vertices.is_empty(), "should have vertices");
        assert!(!blv.faces.is_empty(), "should have faces");
        assert!(!blv.sectors.is_empty(), "should have sectors");
        // Verify face data was populated
        let face_with_verts = blv.faces.iter().find(|f| !f.vertex_ids.is_empty());
        assert!(face_with_verts.is_some(), "faces should have vertex data");
        println!(
            "d01.blv: {} vertices, {} faces, {} sectors, {} decorations, {} lights, {} spawns",
            blv.vertices.len(), blv.faces.len(), blv.sectors.len(),
            blv.decorations.len(), blv.lights.len(), blv.spawn_points.len()
        );
    }

    #[test]
    fn parse_sewer_blv() {
        let lod_manager = LodManager::new(get_lod_path()).unwrap();
        let blv = Blv::new(&lod_manager, "sewer").unwrap();
        assert!(!blv.vertices.is_empty());
        assert!(!blv.faces.is_empty());
    }
}
```

- [ ] **Step 5: Export `blv` module from `openmm-data/src/lib.rs`**

Add `pub mod blv;` to `openmm-data/src/lib.rs` alongside the existing module declarations.

- [ ] **Step 6: Run tests**

Run: `OPENMM_6_PATH=./data/mm6/data cargo test -p openmm-data -- blv --nocapture`
Expected: Both tests pass, prints face/vertex counts for d01.

- [ ] **Step 7: Commit**

```bash
git add openmm-data/src/blv.rs openmm-data/src/lib.rs
git commit -m "feat: add BLV indoor map parser (vertices, faces, sectors, decorations, lights, spawns)"
```

---

### Task 2: DLV Parser — Indoor Actor Loading

**Files:**
- Create: `openmm-data/src/dlv.rs`
- Modify: `openmm-data/src/lib.rs`

- [ ] **Step 1: Create `openmm-data/src/dlv.rs`**

The DLV format stores actors using the same `MapMonster` struct as DDM. We reuse `DdmActor` and the same parsing logic.

```rust
use std::error::Error;

use crate::{ddm::DdmActor, lod_data::LodData, LodManager};

/// Parsed DLV delta file (indoor map mutable state).
/// Currently only extracts actors; doors/chests/items are deferred.
pub struct Dlv {
    pub actors: Vec<DdmActor>,
}

impl Dlv {
    pub fn new(lod_manager: &LodManager, map_name: &str) -> Result<Self, Box<dyn Error>> {
        let dlv_name = if map_name.ends_with(".dlv") {
            map_name.to_string()
        } else if map_name.ends_with(".blv") {
            map_name.replace(".blv", ".dlv")
        } else {
            format!("{}.dlv", map_name)
        };
        let raw = lod_manager.try_get_bytes(&format!("games/{}", dlv_name))?;
        let data = LodData::try_from(raw)?;
        Self::parse(&data.data)
    }

    fn parse(data: &[u8]) -> Result<Self, Box<dyn Error>> {
        // DLV layout (indoor delta):
        // 1. LocationHeader (40 bytes)
        // 2. visibleOutlines (875 bytes)
        // 3. faceAttributes (numFaces × 4 bytes) — need BLV face count
        // 4. decorationFlags (numDecorations × 2 bytes) — need BLV decoration count
        // 5. actors: u32 count + count × 548 bytes (same as DDM MapMonster)
        //
        // Since we don't always have BLV context here, we scan for the actor
        // array the same way DDM does — look for a count followed by valid names.
        // This reuses the DDM heuristic which works for both formats.
        let ddm_result = crate::ddm::Ddm::parse_from_data(data);
        match ddm_result {
            Ok(actors) => Ok(Dlv { actors }),
            Err(_) => Ok(Dlv { actors: Vec::new() }),
        }
    }
}
```

- [ ] **Step 2: Expose `Ddm::parse_from_data` as a public helper**

In `openmm-data/src/ddm.rs`, rename the internal `parse` method or add a public entry point so DLV can reuse it:

```rust
// Add to Ddm impl block:
/// Parse actor data from raw bytes. Used by both DDM and DLV parsers.
pub fn parse_from_data(data: &[u8]) -> Result<Vec<DdmActor>, Box<dyn Error>> {
    let ddm = Self::parse(data)?;
    Ok(ddm.actors)
}
```

- [ ] **Step 3: Add test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{get_lod_path, LodManager};

    #[test]
    fn parse_d01_dlv() {
        let lod_manager = LodManager::new(get_lod_path()).unwrap();
        let dlv = Dlv::new(&lod_manager, "d01").unwrap();
        println!("d01.dlv: {} actors", dlv.actors.len());
    }
}
```

- [ ] **Step 4: Export `dlv` module from `openmm-data/src/lib.rs`**

Add `pub mod dlv;` to `openmm-data/src/lib.rs`.

- [ ] **Step 5: Run tests**

Run: `OPENMM_6_PATH=./data/mm6/data cargo test -p openmm-data -- dlv --nocapture`

- [ ] **Step 6: Commit**

```bash
git add openmm-data/src/dlv.rs openmm-data/src/ddm.rs openmm-data/src/lib.rs
git commit -m "feat: add DLV indoor delta parser (reuses DDM actor parsing)"
```

---

### Task 3: MapName Enum — Unified Map Naming

**Files:**
- Create: `openmm/src/game/map_name.rs`
- Modify: `openmm/src/game/mod.rs`
- Modify: `openmm/src/game/odm.rs`
- Modify: `openmm/src/states/loading.rs`
- Modify: `openmm/src/game/debug.rs`

- [ ] **Step 1: Create `openmm/src/game/map_name.rs`**

```rust
use std::fmt::Display;

use super::odm::OdmName;

/// A map identifier — either an outdoor ODM zone or an indoor BLV dungeon.
#[derive(Clone, Debug)]
pub enum MapName {
    Outdoor(OdmName),
    Indoor(String),
}

impl MapName {
    /// The filename for loading from the LOD archive (e.g. "oute3.odm" or "d01.blv").
    pub fn filename(&self) -> String {
        match self {
            MapName::Outdoor(odm) => odm.to_string(),
            MapName::Indoor(name) => format!("{}.blv", name),
        }
    }

    pub fn is_indoor(&self) -> bool {
        matches!(self, MapName::Indoor(_))
    }

    pub fn is_outdoor(&self) -> bool {
        matches!(self, MapName::Outdoor(_))
    }
}

impl Display for MapName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MapName::Outdoor(odm) => write!(f, "{}", odm),
            MapName::Indoor(name) => write!(f, "{}.blv", name),
        }
    }
}

impl TryFrom<&str> for MapName {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let name = value.to_lowercase();
        // Strip extension if present
        let stem = name.strip_suffix(".odm")
            .or_else(|| name.strip_suffix(".blv"))
            .unwrap_or(&name);

        // Outdoor maps match pattern "outXY" where X=a-e, Y=1-3
        if stem.starts_with("out") && stem.len() == 5 {
            if let Ok(odm) = OdmName::try_from(stem) {
                return Ok(MapName::Outdoor(odm));
            }
        }

        // Everything else is indoor
        if stem.is_empty() {
            return Err("empty map name".to_string());
        }
        Ok(MapName::Indoor(stem.to_string()))
    }
}
```

- [ ] **Step 2: Register the module in `openmm/src/game/mod.rs`**

Add `pub mod map_name;` to the game module.

- [ ] **Step 3: Update `LoadRequest` to use `MapName`**

In `openmm/src/states/loading.rs`:

Change `LoadRequest`:
```rust
use crate::game::map_name::MapName;

#[derive(Resource)]
pub struct LoadRequest {
    pub map_name: MapName,
}
```

Update `loading_setup` to parse `MapName` from config:
```rust
let map_name = load_request
    .map(|r| r.map_name.clone())
    .or_else(|| {
        cfg.map.as_ref().and_then(|m| {
            MapName::try_from(m.as_str())
                .inspect_err(|e| eprintln!("warning: invalid map in config: {e}"))
                .ok()
        })
    })
    .unwrap_or_else(|| MapName::Outdoor(OdmName {
        x: save_data.map.map_x,
        y: save_data.map.map_y,
    }));
```

- [ ] **Step 4: Update `CurrentMapName` in debug.rs**

```rust
use crate::game::map_name::MapName;

#[derive(Resource)]
pub struct CurrentMapName(pub MapName);

impl Default for CurrentMapName {
    fn default() -> Self {
        Self(MapName::Outdoor(OdmName::default()))
    }
}
```

Update `debug_change_map` to only navigate outdoors when current map is outdoor:
```rust
fn debug_change_map(
    keys: Res<ButtonInput<KeyCode>>,
    mut current_map: ResMut<CurrentMapName>,
    mut commands: Commands,
    mut game_state: ResMut<NextState<GameState>>,
) {
    // H/J/K/L map switching only works for outdoor maps
    let MapName::Outdoor(ref odm) = current_map.0 else { return };

    let new_map = if keys.just_pressed(KeyCode::KeyJ) {
        odm.go_north()
    } else if keys.just_pressed(KeyCode::KeyH) {
        odm.go_west()
    } else if keys.just_pressed(KeyCode::KeyK) {
        odm.go_south()
    } else if keys.just_pressed(KeyCode::KeyL) {
        odm.go_east()
    } else {
        None
    };

    if let Some(new_odm) = new_map {
        let map_name = MapName::Outdoor(new_odm);
        info!("Dev: changing map to {}", &map_name);
        commands.insert_resource(LoadRequest { map_name: map_name.clone() });
        current_map.0 = map_name;
        game_state.set(GameState::Loading);
    }
}
```

- [ ] **Step 5: Update boundary crossing in odm.rs**

In `check_map_boundary` and `spawn_world`, update `LoadRequest` construction to use `MapName::Outdoor(new_map)` instead of bare `OdmName`.

In `odm.rs`, update all `LoadRequest { map_name: ... }` to wrap in `MapName::Outdoor(...)`:
```rust
commands.insert_resource(crate::states::loading::LoadRequest {
    map_name: crate::game::map_name::MapName::Outdoor(new_map.clone()),
});
```

- [ ] **Step 6: Build and fix all compilation errors**

Run: `cargo check --package openmm`
Fix any remaining type mismatches from the `OdmName` → `MapName` migration.

- [ ] **Step 7: Commit**

```bash
git add openmm/src/game/map_name.rs openmm/src/game/mod.rs openmm/src/game/odm.rs openmm/src/game/debug.rs openmm/src/states/loading.rs
git commit -m "refactor: introduce MapName enum for outdoor/indoor map support"
```

---

### Task 4: Indoor Loading Pipeline

**Files:**
- Modify: `openmm/src/states/loading.rs`

This is the core integration — when `MapName::Indoor` is loaded, use BLV parser instead of ODM, skip terrain/atlas steps, build face meshes directly.

- [ ] **Step 1: Add indoor-specific prepared data to `LoadingProgress`**

```rust
use openmm_data::blv::Blv;

// Add to LoadingProgress struct:
    blv: Option<Blv>,
    blv_models: Option<Vec<PreparedModel>>,
```

- [ ] **Step 2: Branch `ParseMap` step for indoor maps**

In the `LoadingStep::ParseMap` match arm, check if the map is indoor:

```rust
LoadingStep::ParseMap => {
    let map_name = &load_request.map_name;
    if map_name.is_indoor() {
        // Indoor BLV path
        let blv_stem = match map_name {
            MapName::Indoor(name) => name.clone(),
            _ => unreachable!(),
        };
        match openmm_data::blv::Blv::new(game_assets.lod_manager(), &blv_stem) {
            Ok(blv) => {
                // Load actors from DLV
                let actors = openmm_data::dlv::Dlv::new(game_assets.lod_manager(), &blv_stem)
                    .map(|dlv| dlv.actors)
                    .unwrap_or_default();
                progress.actors = Some(actors);
                progress.monsters = Some(Vec::new());
                progress.blv = Some(blv);
                // Skip terrain steps — jump straight to BuildModels
                progress.step = LoadingStep::BuildModels;
            }
            Err(e) => {
                error!("Failed to parse indoor map {}: {}", blv_stem, e);
                return;
            }
        }
    } else {
        // Existing outdoor ODM path (unchanged)
        // ...
    }
}
```

- [ ] **Step 3: Branch `BuildModels` step for indoor maps**

When BLV data is present, build meshes from BLV faces instead of BSP models:

```rust
LoadingStep::BuildModels => {
    if let Some(blv) = &progress.blv {
        // Indoor: build meshes from BLV faces
        let mut texture_sizes: HashMap<String, (u32, u32)> = HashMap::new();
        for face in &blv.faces {
            if face.texture_name.is_empty() { continue; }
            if !texture_sizes.contains_key(&face.texture_name) {
                if let Some(img) = game_assets.lod_manager().bitmap(&face.texture_name) {
                    texture_sizes.insert(face.texture_name.clone(), (img.width(), img.height()));
                }
            }
        }

        let textured = blv.textured_meshes(&texture_sizes);
        let models = vec![PreparedModel {
            sub_meshes: textured.into_iter().map(|tm| {
                let mut mesh = Mesh::new(
                    PrimitiveTopology::TriangleList,
                    RenderAssetUsages::RENDER_WORLD,
                );
                mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, tm.positions);
                mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, tm.normals);
                mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, tm.uvs);
                _ = mesh.generate_tangents();

                let texture = game_assets.lod_manager().bitmap(&tm.texture_name)
                    .map(|img| {
                        let mut image = crate::assets::dynamic_to_bevy_image(img);
                        image.sampler = crate::assets::repeat_sampler();
                        image
                    });

                PreparedSubMesh {
                    mesh,
                    material: StandardMaterial {
                        base_color: Color::WHITE,
                        alpha_mode: AlphaMode::Opaque,
                        cull_mode: None,
                        double_sided: true,
                        perceptual_roughness: 1.0,
                        reflectance: 0.0,
                        metallic: 0.0,
                        ..default()
                    },
                    texture,
                }
            }).collect(),
        }];
        progress.models = Some(models);
        progress.step = progress.step.next();
    } else if let Some(odm) = &progress.odm {
        // Existing outdoor path (unchanged)
        // ...
    }
}
```

- [ ] **Step 4: Handle indoor path in `BuildTerrain` and `BuildAtlas`**

These steps should be skipped for indoor maps. Since we jump directly from ParseMap to BuildModels for indoor, these steps won't execute. But add a safety check:

```rust
LoadingStep::BuildTerrain => {
    if progress.blv.is_some() {
        // Indoor: no terrain — skip
        progress.step = progress.step.next();
        return;
    }
    // Existing outdoor code...
}
LoadingStep::BuildAtlas => {
    if progress.blv.is_some() {
        progress.step = progress.step.next();
        return;
    }
    // Existing outdoor code...
}
```

- [ ] **Step 5: Handle indoor path in `BuildBillboards`**

For indoor maps, extract decorations from BLV instead of ODM billboards:

```rust
LoadingStep::BuildBillboards => {
    if let Some(blv) = &progress.blv {
        // Indoor decorations
        let mut start_points = Vec::new();
        let mut billboards = Vec::new();
        for dec in &blv.decorations {
            let pos = Vec3::from(openmm_data::odm::mm6_to_bevy(
                dec.position[0], dec.position[1], dec.position[2],
            ));
            let name_lower = dec.name.to_lowercase();
            if name_lower.contains("start") {
                start_points.push(StartPoint {
                    name: dec.name.clone(),
                    position: pos,
                    yaw: dec.yaw as f32 * std::f32::consts::PI / 1024.0,
                });
            }
            // Indoor decorations use declist_id — skip rendering for now
            // (we'd need BillboardManager lookup by decoration_desc_id)
        }
        progress.start_points = Some(start_points);
        progress.billboards = Some(billboards);
        progress.step = progress.step.next();
    } else if let Some(odm) = &progress.odm {
        // Existing outdoor path...
    }
}
```

- [ ] **Step 6: Handle indoor path in `Done` step**

The `Done` step needs to work for indoor maps that have no terrain mesh/texture. Use dummy/empty data:

```rust
LoadingStep::Done => {
    let is_indoor = progress.blv.is_some();
    let models = progress.models.take();

    if is_indoor {
        if let Some(models) = models {
            // Create a minimal empty terrain mesh and 1x1 white texture for indoor
            let mut empty_mesh = Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::RENDER_WORLD,
            );
            empty_mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, Vec::<[f32; 3]>::new());
            empty_mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, Vec::<[f32; 3]>::new());
            empty_mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, Vec::<[f32; 2]>::new());
            empty_mesh.insert_indices(Indices::U32(Vec::new()));

            let white_pixel = Image::new_fill(
                bevy::render::render_resource::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
                bevy::render::render_resource::TextureDimension::D2,
                &[255, 255, 255, 255],
                bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::RENDER_WORLD,
            );

            // Use a dummy Odm — indoor doesn't use it but PreparedWorld requires it.
            // TODO: Refactor PreparedWorld to be an enum (OutdoorWorld / IndoorWorld)
            // For now, we store the BLV models in the models vec and ignore terrain.
            // We need a minimal Odm to satisfy the struct.
            // Actually — let's just make PreparedWorld accept Option<Odm>.
            // See Step 7 for this change.
        }
    } else {
        // Existing outdoor path (unchanged)
    }
}
```

- [ ] **Step 7: Make `PreparedWorld` support indoor maps**

Change `PreparedWorld.map` to `Option<Odm>` since indoor maps don't have an ODM:

```rust
#[derive(Resource)]
pub struct PreparedWorld {
    pub map: Option<Odm>,  // None for indoor maps
    pub terrain_mesh: Mesh,
    pub terrain_texture: Image,
    // ... rest unchanged
}
```

Update the `Done` step's outdoor path to wrap `Some(map)`, and update `odm.rs` spawn_world to handle `Option<Odm>` (early return if `None` for terrain/boundary systems).

- [ ] **Step 8: Build and fix**

Run: `cargo check --package openmm`
Fix all compilation errors.

- [ ] **Step 9: Run the game with an indoor map**

Run: `OPENMM_6_PATH=./data/mm6/data cargo run --package openmm -- --map d01 --skip-intro true`
Expected: Game loads d01.blv and renders indoor geometry (walls, floors, ceilings).

- [ ] **Step 10: Commit**

```bash
git add openmm/src/states/loading.rs
git commit -m "feat: indoor BLV map loading pipeline with --map support"
```

---

### Task 5: Outdoor Spawn System Guards

**Files:**
- Modify: `openmm/src/game/odm.rs`

Indoor maps don't have a terrain heightmap, so terrain-following, boundary checking, and billboard spawning need to handle the indoor case.

- [ ] **Step 1: Guard `spawn_world` for indoor maps**

In `spawn_world`, skip terrain mesh spawning and boundary registration when `PreparedWorld.map` is `None`:

```rust
fn spawn_world(/* ... */) {
    let world = prepared.into_inner();

    // Spawn BSP models (works for both outdoor and indoor)
    for model in &world.models {
        // ... existing model spawning code
    }

    if let Some(ref odm) = world.map {
        // Spawn terrain mesh, set up water, sky, etc.
        // ... existing outdoor-only code
    } else {
        // Indoor: add an ambient light since there's no sun
        commands.spawn((
            PointLight {
                intensity: 500000.0,
                range: 50000.0,
                ..default()
            },
            Transform::from_xyz(0.0, 1000.0, 0.0),
            InGame,
        ));
    }
}
```

- [ ] **Step 2: Guard `check_map_boundary` for indoor maps**

The boundary check should be a no-op for indoor maps (no adjacent zones). Add an early return:

```rust
fn check_map_boundary(/* ... */) {
    let world = prepared.as_ref();
    if world.map.is_none() { return; } // Indoor — no boundaries
    // ... existing boundary code
}
```

- [ ] **Step 3: Guard `lazy_spawn` for indoor maps**

Lazy spawn of billboards and actors should still work for indoor maps, but skip terrain-based height adjustment:

For actors/monsters that reference terrain height, use their BLV-provided Z coordinate directly instead of sampling the heightmap.

- [ ] **Step 4: Build and test**

Run: `cargo check --package openmm`
Run: `OPENMM_6_PATH=./data/mm6/data cargo run --package openmm -- --map d01 --skip-intro true`
Expected: Indoor map renders, no crashes from missing terrain data.

- [ ] **Step 5: Commit**

```bash
git add openmm/src/game/odm.rs
git commit -m "fix: guard outdoor-only systems for indoor map support"
```

---

### Task 6: Player Spawn Position for Indoor Maps

**Files:**
- Modify: `openmm/src/game/player.rs`

- [ ] **Step 1: Read current player.rs spawn logic**

Check how the player spawn position is determined and ensure it works when there's no heightmap.

- [ ] **Step 2: Use start point or BLV center for indoor spawn**

If there are start points from BLV decorations, use the first one. Otherwise compute a position from the BLV bounding box (average of all vertex positions).

The player's terrain-following system needs to be disabled for indoor maps (no heightmap to sample). The player should just stay at their spawned Y position.

- [ ] **Step 3: Disable terrain following for indoor maps**

In the player movement system, check if the current map is indoor and skip heightmap sampling:

```rust
// In the player movement system:
if let Some(ref odm) = prepared_world.map {
    // Existing terrain-following code using odm.height_map
} else {
    // Indoor: keep current Y position (gravity/floor detection comes later)
}
```

- [ ] **Step 4: Build and test**

Run: `OPENMM_6_PATH=./data/mm6/data cargo run --package openmm -- --map d01 --skip-intro true`
Expected: Player spawns inside the dungeon and can look around.

- [ ] **Step 5: Commit**

```bash
git add openmm/src/game/player.rs
git commit -m "feat: player spawn and movement for indoor maps"
```

---

### Task 7: Polish and Verify

- [ ] **Step 1: Test multiple indoor maps**

Run each of these and verify geometry renders:
```bash
OPENMM_6_PATH=./data/mm6/data cargo run --package openmm -- --map sewer --skip-intro true
OPENMM_6_PATH=./data/mm6/data cargo run --package openmm -- --map pyramid --skip-intro true
OPENMM_6_PATH=./data/mm6/data cargo run --package openmm -- --map t1 --skip-intro true
```

- [ ] **Step 2: Verify outdoor maps still work**

```bash
OPENMM_6_PATH=./data/mm6/data cargo run --package openmm -- --map oute3 --skip-intro true
```

- [ ] **Step 3: Run full test suite**

Run: `OPENMM_6_PATH=./data/mm6/data cargo test`
Expected: All existing tests pass plus new BLV/DLV tests.

- [ ] **Step 4: Update CLAUDE.md**

Add BLV/DLV to the architecture docs and openmm-data crate structure sections.

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "docs: update CLAUDE.md with BLV/DLV indoor map architecture"
```
