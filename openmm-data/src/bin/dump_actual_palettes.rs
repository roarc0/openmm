use byteorder::{LittleEndian, ReadBytesExt};
use openmm_data::Assets;
use std::io::{Cursor, Seek};

fn main() {
    let lod_path = openmm_data::get_data_path();
    let assets = Assets::new(&lod_path).expect("failed to open LOD files");

    let sprite_names = vec![
        // Goblins (all variants share the same sprites!)
        "gobsta0", "gobwaa0", "gobata0", // Barbarians
        "bar1sta0", "bar1waa0", "bar1ata0", "bar2sta0", "bar2waa0", "bar2ata0", "bar3sta0", "bar3waa0", "bar3ata0",
    ];

    println!("Actual Sprite Palette IDs:");
    println!();

    for name in sprite_names {
        match assets.get_bytes(format!("sprites/{}", name)) {
            Ok(data) => {
                if data.len() > 28 {
                    let mut cursor = Cursor::new(data);
                    cursor.seek(std::io::SeekFrom::Start(24)).ok();
                    match cursor.read_u16::<LittleEndian>() {
                        Ok(palette_id) => {
                            println!("{:12} -> palette_id: {}", name, palette_id);
                        }
                        Err(e) => println!("{:12} -> ERROR: {}", name, e),
                    }
                } else {
                    println!("{:12} -> data too short", name);
                }
            }
            Err(_) => println!("{:12} -> NOT FOUND", name),
        }
    }
}
