use crate::{image::get_atlas, utils::read_string, LodManager};
use byteorder::{LittleEndian, ReadBytesExt};
use image::DynamicImage;
use std::{
    error::Error,
    io::{Cursor, Seek},
};

#[derive(Debug)]
pub struct Dtile {
    data: Vec<u8>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
struct DtileData {
    name: String,
    a: u16,
    b: u16,
    c: u16,
}

impl Dtile {
    pub fn new(data: &[u8]) -> Self {
        Self {
            data: data.to_vec(),
        }
    }

    pub fn count(&self) -> usize {
        let mut cursor = Cursor::new(self.data.as_slice());
        cursor.read_u32::<LittleEndian>().unwrap_or_default() as usize
    }

    fn read(&self, i: usize) -> Result<DtileData, Box<dyn Error>> {
        let mut cursor = Cursor::new(self.data.as_slice());

        cursor.seek(std::io::SeekFrom::Start((4 + i * 26) as u64))?;
        let pos = cursor.position();
        let name = read_string(&mut cursor)?;
        let name = if name.is_empty() {
            "pending".into()
        } else {
            name.to_lowercase()
        };
        cursor.seek(std::io::SeekFrom::Start(pos + 20))?;
        let a = cursor.read_u16::<LittleEndian>()?;
        let b = cursor.read_u16::<LittleEndian>()?;
        let c = cursor.read_u16::<LittleEndian>()?;
        Ok(DtileData { name, a, b, c })
    }

    pub fn table(&self, tile_data: [u16; 8]) -> Result<TileTable, Box<dyn Error>> {
        let mut names_table: Vec<String> = Vec::with_capacity(256);
        println!("{tile_data:?}");
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

            let dtile: DtileData = self.read(index as usize)?;
            names_table.push(dtile.name);
        }

        Ok(TileTable::new(names_table.try_into().unwrap()))
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
            .cloned()
            .filter(|d| !d.starts_with("drr"))
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
    use crate::{dtile::Dtile, get_lod_path, lod_data::LodData, odm::Odm, LodManager};

    #[test]
    fn read_dtile_data_works() {
        let lod_path = get_lod_path();
        let lod_manager = LodManager::new(lod_path).unwrap();

        let dtile =
            LodData::try_from(lod_manager.try_get_bytes("icons/dtile.bin").unwrap()).unwrap();
        let dtile = Dtile::new(dtile.data.as_slice());
        assert_eq!(dtile.count(), 882);

        for i in 0..882 {
            let _ = dtile.read(i).unwrap();
        }
    }

    #[test]
    fn atlas_generation_works() {
        let lod_path = get_lod_path();
        let lod_manager = LodManager::new(lod_path).unwrap();

        let map = LodData::try_from(lod_manager.try_get_bytes("games/oute3.odm").unwrap()).unwrap();
        let map = Odm::try_from(map.data.as_slice()).unwrap();
        let dtile =
            LodData::try_from(lod_manager.try_get_bytes("icons/dtile.bin").unwrap()).unwrap();
        let dtile = Dtile::new(dtile.data.as_slice());

        let tile_table = dtile.table(map.tile_data).unwrap();
        tile_table
            .atlas_image(&lod_manager)
            .unwrap()
            .save("terrain_atlas.png")
            .unwrap();
    }
}
