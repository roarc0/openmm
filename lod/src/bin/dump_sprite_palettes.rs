use lod::LodManager;
use std::io::{Cursor, Seek};
use byteorder::{LittleEndian, ReadBytesExt};

fn main() {
    let lod_path = lod::get_lod_path();
    let lod_manager = LodManager::new(&lod_path).expect("failed to open LOD files");
    
    let sprite_names = vec![
        // Goblins
        "gobsta", "gobwaa", "gobata0",
        "gbbsta", "gbbwaa", "gbbata0",
        "gcbsta", "gcbwaa", "gcbata0",
        // Barbarians
        "bar1sta", "bar1walk", "bar1ata",
        "bar2sta", "bar2walk", "bar2ata",
        "bar3sta", "bar3walk", "bar3ata",
    ];
    
    println!("Sprite Palette IDs:");
    println!();
    
    for name in sprite_names {
        match lod_manager.try_get_bytes(&format!("sprites/{}", name)) {
            Ok(data) => {
                if data.len() > 28 {
                    let mut cursor = Cursor::new(data);
                    cursor.seek(std::io::SeekFrom::Start(24)).ok();
                    match cursor.read_u16::<LittleEndian>() {
                        Ok(palette_id) => {
                            println!("{:12} -> palette_id: {}", name, palette_id);
                        }
                        Err(e) => println!("{:12} -> ERROR reading palette_id: {}", name, e),
                    }
                } else {
                    println!("{:12} -> data too short", name);
                }
            }
            Err(_) => println!("{:12} -> NOT FOUND", name),
        }
    }
}
