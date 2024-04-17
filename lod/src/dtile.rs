use crate::{image::get_atlas, lod_data::LodData, utils::try_read_name, LodManager};
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
    attributes: u16,
}

#[allow(dead_code)]
impl Tile {
    pub fn is_burn(&self) -> bool {
        (self.attributes & 0x0001) != 0
    }

    pub fn is_water(&self) -> bool {
        (self.attributes & 0x0002) != 0
    }

    pub fn is_block(&self) -> bool {
        (self.attributes & 0x0004) != 0
    }

    pub fn is_repulse(&self) -> bool {
        (self.attributes & 0x0010) != 0
    }

    pub fn is_flat(&self) -> bool {
        (self.attributes & 0x0020) != 0
    }

    pub fn is_wave(&self) -> bool {
        (self.attributes & 0x0040) != 0
    }

    pub fn is_no_draw(&self) -> bool {
        (self.attributes & 0x0080) != 0
    }

    pub fn is_water_transition(&self) -> bool {
        (self.attributes & 0x0200) != 0
    }

    pub fn is_transition(&self) -> bool {
        (self.attributes & 0x0400) != 0
    }

    pub fn is_scroll_down(&self) -> bool {
        (self.attributes & 0x0800) != 0
    }

    pub fn is_scroll_up(&self) -> bool {
        (self.attributes & 0x1000) != 0
    }

    pub fn is_scroll_left(&self) -> bool {
        (self.attributes & 0x2000) != 0
    }

    pub fn is_scroll_right(&self) -> bool {
        (self.attributes & 0x4000) != 0
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
        let tile_size = std::mem::size_of::<Tile>();
        let mut tiles = Vec::new();
        for _ in 0..tile_count {
            let mut tile = Tile::default();
            cursor.read_exact(unsafe {
                std::slice::from_raw_parts_mut(&mut tile as *mut _ as *mut u8, tile_size)
            })?;
            tiles.push(tile);
        }

        Ok(Self { tiles })
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

mod tests {
    use crate::{dtile::Dtile, get_lod_path, odm::Odm, LodManager};

    #[test]
    fn read_dtile_data_works() {
        let lod_manager = LodManager::new(get_lod_path()).unwrap();
        let dtile = Dtile::new(&lod_manager).unwrap();
        assert_eq!(dtile.tiles.len(), 882);
    }

    #[test]
    fn atlas_generation_works() {
        let lod_manager = LodManager::new(get_lod_path()).unwrap();
        let map = Odm::new(&lod_manager, "oute3.odm").unwrap();
        let dtile = Dtile::new(&lod_manager).unwrap();

        let tile_table = dtile.table(map.tile_data).unwrap();
        tile_table
            .atlas_image(&lod_manager)
            .unwrap()
            .save("terrain_atlas.png")
            .unwrap();
    }
}
