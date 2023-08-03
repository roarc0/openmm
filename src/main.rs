use std::error::Error;

mod image;
mod lod;
mod odm6;
mod palette;
mod sprite;
mod utils;

fn main() {
    let bitmaps_lod = lod::Lod::open("/home/roarc/BITMAPS.LOD").unwrap();
    println!("{:?}", bitmaps_lod.files());

    let palettes = palette::Palettes::get_palettes(&bitmaps_lod).unwrap();

    let games_lod = lod::Lod::open("/home/roarc/games.lod").unwrap();

    let map_name = "Outa1.odm";
    let o = games_lod.get::<odm6::Odm6>(map_name).unwrap();

    //println!("{:?}", odm);

    // for f in lod_files {
    //     match dump_image_to_file(&lod, f) {
    //         Ok(()) => println!("Saving {}", f),
    //         Err(e) => println!("Error saving {}: {}", f, e),
    //     }
    // }

    let sprites_lod = lod::Lod::open("/home/roarc/SPRITES.LOD").unwrap();
    let s = sprites_lod.get::<sprite::Sprite>("flower06").unwrap();
    match s.to_png_file("flower06.png", &palettes) {
        Ok(()) => println!("ok"),
        Err(e) => println!("ko: {}", e),
    }
}
