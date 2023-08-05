use std::{env, path::Path};

mod lod;

const ENV_OMM_LOD_PATH: &str = "OMM_LOD_PATH";
const ENV_OMM_DUMP_PATH: &str = "OMM_DUMP_PATH";

fn main() {
    let lod_path = env::var(ENV_OMM_LOD_PATH).unwrap_or("./target/mm6/data".into());
    println!("lod_path: {}", lod_path);
    let lod_path = Path::new(&lod_path);
    let dump_path = env::var(ENV_OMM_DUMP_PATH).unwrap_or("./target/dump".into());
    println!("dump_path: {}", dump_path);
    let dump_path: &Path = Path::new(&dump_path);

    let bitmaps_lod = lod::Lod::open(lod_path.join("BITMAPS.LOD")).unwrap();
    let palettes = lod::palette::Palettes::try_from(&bitmaps_lod).unwrap();
    bitmaps_lod.dump(&dump_path.join("bitmaps_lod"), &palettes);

    let games_lod = lod::Lod::open(lod_path.join("games.lod")).unwrap();
    games_lod.dump(&dump_path.join("games_lod"), &palettes);

    let sprites_lod = lod::Lod::open(lod_path.join("SPRITES.LOD")).unwrap();
    sprites_lod.dump(&dump_path.join("sprites_lod"), &palettes);

    let icons_lod = lod::Lod::open(lod_path.join("icons.lod")).unwrap();
    icons_lod.dump(&dump_path.join("icons_lod"), &palettes);

    let new_lod = lod::Lod::open(lod_path.join("new.lod")).unwrap();
    new_lod.dump(&dump_path.join("new_lod"), &palettes)
}
