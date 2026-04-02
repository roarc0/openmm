use crate::{enums::TileFlags, image::get_atlas, lod_data::LodData, utils::try_read_name, LodManager};

/// Terrain tileset types from MM6 dtile.bin.
/// Raw tile_set values: 0=grass, 1=snow, 2=sand, 3=volcanic, 4=dirt,
/// 5=water, 6=cracked swamp, 7=swamp, 8+=roads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tileset {
    Grass,
    Snow,
    Desert,
    Volcanic,
    Dirt,
    Water,
    CrackedSwamp,
    Swamp,
    Road,
}

impl std::fmt::Display for Tileset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Tileset::Grass => write!(f, "GRASS"),
            Tileset::Snow => write!(f, "SNOW"),
            Tileset::Desert => write!(f, "DESERT"),
            Tileset::Volcanic => write!(f, "VOLCANIC"),
            Tileset::Dirt => write!(f, "DIRT"),
            Tileset::Water => write!(f, "WATER"),
            Tileset::CrackedSwamp => write!(f, "CRACKED"),
            Tileset::Swamp => write!(f, "SWAMP"),
            Tileset::Road => write!(f, "ROAD"),
        }
    }
}

impl Tileset {
    /// Convert raw tile_set value from dtile.bin to Tileset enum.
    /// MM6 dtile.bin uses: 0=grass, 1=snow, 2=sand, 3=volcanic, 4=dirt, 5=water, 6=cracked, 7=swamp, 8+=roads.
    pub fn from_raw(v: i16) -> Option<Self> {
        match v {
            0 => Some(Self::Grass),
            1 => Some(Self::Snow),
            2 => Some(Self::Desert),
            3 => Some(Self::Volcanic),      // voltyl
            4 => Some(Self::Dirt),
            5 => Some(Self::Water),
            6 => Some(Self::CrackedSwamp),  // crktyl
            7 => Some(Self::Swamp),         // swmtyl
            8..=255 => Some(Self::Road),
            _ => None,
        }
    }
}
use byteorder::{LittleEndian, ReadBytesExt};
use image::DynamicImage;
use std::{
    error::Error,
    io::{Cursor, Read},
};

#[derive(Debug)]
pub struct Dtile {
    tiles: Vec<Tile>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
struct Tile {
    name: [u8; 16],
    id: i16,
    bitmap: i16,
    tile_set: i16,
    section: i16,
    attributes: TileFlags,
}

#[allow(dead_code)]
impl Tile {
    pub fn is_burn(&self) -> bool {
        self.attributes.contains(TileFlags::BURN)
    }

    pub fn is_water(&self) -> bool {
        self.attributes.contains(TileFlags::WATER)
    }

    pub fn is_block(&self) -> bool {
        self.attributes.contains(TileFlags::BLOCK)
    }

    pub fn is_repulse(&self) -> bool {
        self.attributes.contains(TileFlags::REPULSE)
    }

    pub fn is_flat(&self) -> bool {
        self.attributes.contains(TileFlags::FLAT)
    }

    pub fn is_wave(&self) -> bool {
        self.attributes.contains(TileFlags::WAVY)
    }

    pub fn is_no_draw(&self) -> bool {
        self.attributes.contains(TileFlags::DONT_DRAW)
    }

    pub fn is_water_transition(&self) -> bool {
        self.attributes.contains(TileFlags::SHORE)
    }

    pub fn is_transition(&self) -> bool {
        self.attributes.contains(TileFlags::TRANSITION)
    }

    pub fn is_scroll_down(&self) -> bool {
        self.attributes.contains(TileFlags::SCROLL_DOWN)
    }

    pub fn is_scroll_up(&self) -> bool {
        self.attributes.contains(TileFlags::SCROLL_UP)
    }

    pub fn is_scroll_left(&self) -> bool {
        self.attributes.contains(TileFlags::SCROLL_LEFT)
    }

    pub fn is_scroll_right(&self) -> bool {
        self.attributes.contains(TileFlags::SCROLL_RIGHT)
    }

    pub fn name(&self) -> Option<String> {
        try_read_name(&self.name).map(|v| if v.is_empty() { "pending".into() } else { v })
    }
}

impl Dtile {
    pub fn new(lod_manager: &LodManager) -> Result<Self, Box<dyn Error>> {
        let data = LodData::try_from(lod_manager.try_get_bytes("icons/dtile.bin")?)?;
        let data = data.data.as_slice();

        let mut cursor = Cursor::new(data);
        let tile_count = cursor.read_u32::<LittleEndian>()?;
        let mut tiles = Vec::with_capacity(tile_count as usize);
        for _ in 0..tile_count {
            let mut name = [0u8; 16];
            cursor.read_exact(&mut name)?;
            let id = cursor.read_i16::<LittleEndian>()?;
            let bitmap = cursor.read_i16::<LittleEndian>()?;
            let tile_set = cursor.read_i16::<LittleEndian>()?;
            let section = cursor.read_i16::<LittleEndian>()?;
            let attributes = TileFlags::from_bits_truncate(cursor.read_u16::<LittleEndian>()?);
            tiles.push(Tile { name, id, bitmap, tile_set, section, attributes });
        }

        Ok(Self { tiles })
    }

    /// Get the tile_set value for a raw tile_map index (0-255).
    /// Requires the per-map `tile_data` offsets to remap indices correctly.
    /// Returns the tileset enum: 0=invalid, 1=grass, 2=snow, 3=desert, 4=dirt, 5=water, 6=badlands, 7=swamp, 8=road
    pub fn tile_set(&self, tile_index: u8) -> i16 {
        self.tiles.get(tile_index as usize).map(|t| t.tile_set).unwrap_or(0)
    }

    /// Get name and tile_set for a dtile entry (for debugging).
    pub fn tile_info(&self, index: usize) -> (String, i16) {
        self.tiles.get(index)
            .map(|t| (t.name().unwrap_or("?".into()), t.tile_set))
            .unwrap_or(("OOB".into(), -1))
    }

    /// Build a tileset lookup table for tile_map values 0-255 using the per-map tile_data offsets.
    /// This applies the same remapping as `table()` to resolve the actual dtile entry.
    pub fn tileset_lookup(&self, tile_data: [u16; 8]) -> Vec<i16> {
        (0..=255u16)
            .map(|i| {
                let index = if (90..125).contains(&i) {
                    i - 90 + tile_data[1] // primary terrain
                } else if (126..161).contains(&i) {
                    i // water (no remap)
                } else if (162..197).contains(&i) {
                    i - 162 + tile_data[5] // secondary terrain
                } else if i >= 198 {
                    i - 198 + tile_data[7] // roads
                } else {
                    i // dirt/base
                };
                self.tiles.get(index as usize).map(|t| t.tile_set).unwrap_or(0)
            })
            .collect()
    }

    /// Check if a tile index is pure water (blocks movement).
    /// Returns false for water transition tiles (shore) which are passable.
    pub fn is_deep_water_tile(&self, tile_index: u8) -> bool {
        if let Some(tile) = self.tiles.get(tile_index as usize) {
            tile.is_water() && !tile.is_water_transition()
        } else {
            false
        }
    }

    /// Check if a tile index has any water (deep or transition).
    pub fn is_any_water_tile(&self, tile_index: u8) -> bool {
        if let Some(tile) = self.tiles.get(tile_index as usize) {
            tile.is_water()
        } else {
            false
        }
    }

    pub fn table(&self, tile_data: [u16; 8]) -> Option<TileTable> {
        let mut names_table: Vec<String> = Vec::with_capacity(256);
        for i in 0_u16..=255_u16 {
            let index = if (90..125).contains(&i) {
                i - 90 + tile_data[1] // primary
            } else if (126..161).contains(&i) {
                i
                //i - 126 + tile_data[3] // water
            } else if (162..197).contains(&i) {
                i - 162 + tile_data[5] // secondary
            } else if i >= 198 {
                i - 198 + tile_data[7] // roads
            } else {
                i // dirt
            };

            let tile = self.tiles.get(index as usize)?;
            names_table.push(tile.name().unwrap_or("pending".into()));
        }

        Some(TileTable::new(names_table.try_into().unwrap()))
    }
}

#[derive(Debug)]
pub struct TileTable {
    size: (u8, u8),
    names_table: [String; 256],
    names_set: Vec<String>,
    coordinates_table: [(u8, u8); 256],
}

impl TileTable {
    pub fn new(names_table: [String; 256]) -> Self {
        let names_set = Self::names_set(&names_table);
        let mut t = TileTable {
            size: Self::matrix_dimensions(names_set.len() as u8, 10),
            names_set,
            names_table,
            coordinates_table: [(0, 0); 256],
        };
        t.generate_coordinates_table();
        t
    }

    fn matrix_dimensions(total_elements: u8, row_size: u8) -> (u8, u8) {
        let num_rows = (total_elements as f32 / row_size as f32).ceil() as u8;
        let num_cols = if total_elements < row_size {
            total_elements
        } else {
            row_size
        };
        (num_cols, num_rows)
    }

    pub fn size(&self) -> (u8, u8) {
        self.size
    }

    fn index_to_coordinate(&self, i: u8) -> (u8, u8) {
        (i.rem_euclid(self.size.0), i.div_euclid(self.size.0))
    }

    pub fn name(&self, tile_index: u8) -> &str {
        self.names_table[tile_index as usize].as_str()
    }

    pub fn coordinate(&self, tile_index: u8) -> (u8, u8) {
        self.coordinates_table[tile_index as usize]
    }

    fn generate_coordinates_table(&mut self) {
        let set: Vec<(usize, &String)> = self.names_set.iter().enumerate().collect();
        for i in 0..=255 {
            self.coordinates_table[i] = self.index_to_coordinate(
                set.iter()
                    .find_map(|v| {
                        if *v.1 == self.names_table[i] {
                            Some(v.0)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(set.len() - 1) as u8,
            );
        }
    }

    fn names_set(name_table: &[String; 256]) -> Vec<String> {
        let mut set: Vec<String> = name_table
            .iter()
            .filter(|&d| !d.starts_with("drr"))
            .cloned()
            .collect();
        set.sort_by(|a, b| {
            if a == "pending" {
                std::cmp::Ordering::Greater
            } else if b == "pending" {
                std::cmp::Ordering::Less
            } else {
                a.cmp(b)
            }
        });
        set.dedup();
        set
    }

    pub fn atlas_image(&self, lod_manager: &LodManager) -> Result<DynamicImage, Box<dyn Error>> {
        let ts: Vec<&str> = self.names_set.iter().map(|s| s.as_str()).collect();
        get_atlas(lod_manager, ts.as_slice(), self.size.0 as usize)
    }
}

#[cfg(test)]
#[path = "dtile_tests.rs"]
mod tests;
