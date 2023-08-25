use crate::{image::get_atlas, read_string, Lod};
use byteorder::{LittleEndian, ReadBytesExt};
use image::DynamicImage;
use std::{
    error::Error,
    io::{Cursor, Seek},
};

#[derive(Debug)]
pub struct DtileBin {
    data: Vec<u8>,
}

#[derive(Debug, Clone, Default)]
struct DtileData {
    name: String,
    a: u16,
    b: u16,
    c: u16,
}

impl DtileBin {
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

    pub fn table(&self, tile_data: [u16; 8]) -> TileTable {
        let mut names_table: Vec<String> = Vec::with_capacity(256);
        for i in 0..=255 {
            // This is an hardcoded decoding of the index since I can't yet make full sense of the dtile.bin
            let name = index_to_tile_name_hack(&tile_data, i as u8);
            if name != "pending" {
                //println!("idx:{}, name:{} (hack)", i, name);
                names_table.push(name);
                continue;
            }

            let index = if i >= 198 {
                // roads
                i as u16 - 198 + tile_data[7]
            } else if i < 90 {
                i as u16 //+ tile_data[1]
                         // } else if (126..198).contains(&i) {
                         //     i - 126 + tile_data[5]
            } else {
                // borders
                let n = 2 * (i as u16 - 90) / 36;
                tile_data[n as usize]
            };

            let mut dtile = self.read(index as usize).unwrap();

            if dtile.c == 0x300 {
                let group = dtile.a;
                for j in 0..4 {
                    if tile_data[j * 2] == group {
                        let index2 = tile_data[j * 2 + 1];
                        dtile = self.read(index2 as usize).unwrap();
                        break;
                    }
                }
            }
            //println!("idx:{}, name:{} (dtile)", i, dtile.name);
            names_table.push(dtile.name);
        }

        TileTable::new(names_table.try_into().unwrap())
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
        let num_rows = (total_elements as f32 / row_size as f32).ceil();
        let num_cols = if total_elements < row_size {
            total_elements
        } else {
            row_size
        };
        (num_cols as u8, num_rows as u8)
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
            if a == &"pending" {
                std::cmp::Ordering::Greater
            } else if b == &"pending" {
                std::cmp::Ordering::Less
            } else {
                a.cmp(b)
            }
        });
        set.dedup();
        set
    }

    pub fn atlas_image(&self, lod: Lod) -> DynamicImage {
        let ts: Vec<&str> = self.names_set.iter().map(|s| s.as_str()).collect();
        get_atlas(&lod, ts.as_slice(), self.size.0 as usize).unwrap()
    }
}

#[deprecated]
fn index_to_tile_name_hack(tile_data: &[u16; 8], index: u8) -> String {
    if (1..=0x34).contains(&index) {
        return "dirttyl".into();
    }

    if (0x5a..=0x65).contains(&index) {
        return get_tile_group_hack(tile_data, 0).0;
    }
    if let Some(name) = get_composed_tile_group(tile_data, 0, index, 0x66) {
        return name.1;
    }

    if (0xa2..=0xad).contains(&index) {
        return get_tile_group_hack(tile_data, 4).0;
    }
    if let Some(name) = get_composed_tile_group(tile_data, 4, index, 0xae) {
        return name.1;
    }

    if (0x7e..=0x89).contains(&index) {
        return get_tile_group_hack(tile_data, 6).0;
    }
    if let Some(name) = get_composed_tile_group(tile_data, 6, index, 0x8a) {
        return name.1;
    }

    return "pending".into();
}

#[deprecated]
fn get_tile_group_hack(tile_data: &[u16; 8], group: usize) -> (String, String) {
    let id1 = tile_data[group];
    let id2 = tile_data[group + 1];

    if id1 == 0 && id2 == 90 {
        return ("grastyl".into(), "grdrt".into());
    }
    if id1 == 1 && id2 == 342 {
        return ("snotyl".into(), "snodr".into());
    }
    if id1 == 2 && id2 == 234 {
        return ("sandtyl".into(), "sndrt".into());
    }
    if id1 == 3 && id2 == 198 {
        return ("voltyl".into(), "voldrt".into());
    }
    if id1 == 6 && id2 == 162 {
        return ("crktyl".into(), "crkdrt".into());
    }
    if id1 == 7 && id2 == 270 {
        return ("swmtyl".into(), "swmdr".into());
    }
    if id1 == 8 && id2 == 306 {
        return ("troptyl".into(), "trop".into());
    }
    if id1 == 22 && id2 == 774 {
        return ("wtrtyl".into(), "wtrdr".into());
    }
    ("pending".into(), "pending".into())
}

#[deprecated]
fn get_composed_tile_group(
    tile_data: &[u16; 8],
    group: usize,
    index: u8,
    code: u8,
) -> Option<(String, String)> {
    let suffix = get_tile_type(index, code)?;
    let mut group = get_tile_group_hack(tile_data, group);
    group.1 = format!("{}{}", group.1, suffix);
    Some(group)
}

#[deprecated]
fn get_tile_type(index: u8, code: u8) -> Option<&'static str> {
    let group_type = [
        "ne", "se", "nw", "sw", "e", "w", "n", "s", "xne", "xse", "xnw", "xsw",
    ];

    if index < code {
        return None;
    }

    let offset = index - code;
    if offset < 12 {
        Some(group_type[offset as usize])
    } else {
        None
    }
}

// #[deprecated]
// fn add_missing_textures(imglist: &mut Vec<String>) {
//     let textures = [
//         "wtrtyl",
//         "wtrdre",
//         "wtrdrn",
//         "wtrdrs",
//         "wtrdrw",
//         "wtrdrnw",
//         "wtrdrne",
//         "wtrdrsw",
//         "wtrdrse",
//         "wtrdrxne",
//         "wtrdrxnw",
//         "wtrdrxse",
//         "wtrdrxsw",
//         "voltyl",
//         "voldrte",
//         "voldrtn",
//         "voldrts",
//         "voldrtw",
//         "voldrtnw",
//         "voldrtne",
//         "voldrtsw",
//         "voldrtse",
//         "voldrtxne",
//         "voldrtxnw",
//         "voldrtxse",
//         "voldrtxsw",
//         "troptyl",
//         "trope",
//         "tropn",
//         "trops",
//         "tropw",
//         "tropnw",
//         "tropne",
//         "tropsw",
//         "tropse",
//         "tropxne",
//         "tropxnw",
//         "tropxse",
//         "tropxsw",
//         "snotyl",
//         "snodre",
//         "snodrn",
//         "snodrs",
//         "snodrw",
//         "snodrnw",
//         "snodrne",
//         "snodrsw",
//         "snodrse",
//         "snodrxne",
//         "snodrxnw",
//         "snodrxse",
//         "snodrxsw",
//         "sandtyl",
//         "sndrte",
//         "sndrtn",
//         "sndrts",
//         "sndrtw",
//         "sndrtnw",
//         "sndrtne",
//         "sndrtsw",
//         "sndrtse",
//         "sndrtxne",
//         "sndrtxnw",
//         "sndrtxse",
//         "sndrtxsw",
//         "swmtyl",
//         "swmdre",
//         "swmdrn",
//         "swmdrs",
//         "swmdrw",
//         "swmdrnw",
//         "swmdrne",
//         "swmdrsw",
//         "swmdrse",
//         "swmdrxne",
//         "swmdrxnw",
//         "swmdrxse",
//         "swmdrxsw",
//         "crktyl",
//         "crkdrte",
//         "crkdrtn",
//         "crkdrts",
//         "crkdrtw",
//         "crkdrtnw",
//         "crkdrtne",
//         "crkdrtsw",
//         "crkdrtse",
//         "crkdrtxne",
//         "crkdrtxnw",
//         "crkdrtxse",
//         "crkdrtxsw",
//     ];

//     for x in textures {
//         imglist.push(x.to_string());
//     }
// }

mod tests {
    use super::DtileBin;
    use crate::{dtile::DtileData, get_lod_path, image::get_atlas, odm::Odm, raw, Lod};
    use std::path::Path;

    #[test]
    fn read_dtile_data_works() {
        let lod_path = get_lod_path();
        let lod_path = Path::new(&lod_path);

        let icons_lod = Lod::open(lod_path.join("icons.lod")).unwrap();
        let dtile = raw::Raw::try_from(icons_lod.try_get_bytes("dtile.bin").unwrap()).unwrap();
        let dtile = DtileBin::new(dtile.data.as_slice());
        assert_eq!(dtile.count(), 882);

        for i in 0..882 {
            let _ = dtile.read(i).unwrap();
        }
    }

    #[test]
    fn atlas_generation_works() {
        let lod_path = get_lod_path();
        let lod_path = Path::new(&lod_path);
        let bitmaps_lod = Lod::open(lod_path.join("BITMAPS.LOD")).unwrap();
        let games_lod = Lod::open(lod_path.join("games.lod")).unwrap();
        let icons_lod = Lod::open(lod_path.join("icons.lod")).unwrap();

        let map = raw::Raw::try_from(games_lod.try_get_bytes("oute3.odm").unwrap()).unwrap();
        let map = Odm::try_from(map.data.as_slice()).unwrap();
        let dtile = raw::Raw::try_from(icons_lod.try_get_bytes("dtile.bin").unwrap()).unwrap();
        let dtile = DtileBin::new(dtile.data.as_slice());

        let tile_table = dtile.table(map.tile_data);
        tile_table
            .atlas_image(bitmaps_lod)
            .save("map_viewer/assets/terrain_atlas.png")
            .unwrap();
        println!("{:?}", tile_table.size());
        println!(
            "{:?} -> {:?}",
            tile_table.name(22),
            tile_table.coordinate(22)
        );
        println!(
            "{:?} -> {:?}",
            tile_table.name(90),
            tile_table.coordinate(90)
        );
        println!("");
    }
}
