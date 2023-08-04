use std::{env, error::Error, fs, path::Path};

mod lod;

const ENV_OMM_LOD_PATH: &str = "OMM_LOD_PATH";
const ENV_OMM_DUMP_PATH: &str = "OMM_DUMP_PATH";

fn main() {
    let lod_path = env::var(ENV_OMM_LOD_PATH).unwrap_or("./target/mm6/data".into());
    let lod_path = Path::new(&lod_path);
    let dump_path = env::var(ENV_OMM_DUMP_PATH).unwrap_or("./target/dump".into());
    let dump_path: &Path = Path::new(&dump_path);

    let bitmaps_lod = lod::Lod::open(lod_path.join("BITMAPS.LOD")).unwrap();
    let palettes = lod::palette::Palettes::try_from(&bitmaps_lod).unwrap();
    //dump_all(&bitmaps_lod, &dump_path.join("bitmaps_lod"), &palettes);

    let games_lod = lod::Lod::open(lod_path.join("games.lod")).unwrap();
    //dump_all(&games_lod, &dump_path.join("games_lod"), &palettes);

    let sprites_lod = lod::Lod::open(lod_path.join("SPRITES.LOD")).unwrap();
    //dump_all(&sprites_lod, &dump_path.join("sprites_lod"), &palettes);

    let icons_lod = lod::Lod::open(lod_path.join("icons.lod")).unwrap();
    //dump_all(&icons_lod, &dump_path.join("icons_lod"), &palettes);

    let new_lod = lod::Lod::open(lod_path.join("new.lod")).unwrap();
    dump_all(&new_lod, &dump_path.join("new_lod"), &palettes);
}

// TODO avoid reading the file multiple times
fn dump_all(lod: &lod::Lod, path: &Path, palettes: &lod::palette::Palettes) {
    fs::create_dir_all(path).unwrap();
    for file_name in lod.files() {
        if dump_image(lod, file_name, path).is_ok() {
            continue;
        }
        if dump_sprite(lod, file_name, path, palettes).is_ok() {
            continue;
        }
        if dump_raw_unpacked(lod, file_name, path).is_ok() {
            continue;
        }
        if dump_raw(lod, file_name, path).is_ok() {
            continue;
        }
        println!("Error extracting file {}", file_name);
    }
}

fn dump_image(lod: &lod::Lod, file_name: &str, path: &Path) -> Result<(), Box<dyn Error>> {
    lod.get::<lod::image::Image>(file_name)?
        .dump(path.join(format!("{}.png", file_name)))
}

fn dump_sprite(
    lod: &lod::Lod,
    file_name: &str,
    path: &Path,
    palettes: &lod::palette::Palettes,
) -> Result<(), Box<dyn Error>> {
    lod.get::<lod::sprite::Sprite>(file_name)?
        .dump(palettes, path.join(format!("{}.png", file_name)))
}

fn dump_raw(lod: &lod::Lod, file_name: &str, path: &Path) -> Result<(), Box<dyn Error>> {
    lod.get::<lod::raw::Raw>(file_name)?
        .dump(path.join(file_name))
}

fn dump_raw_unpacked(lod: &lod::Lod, file_name: &str, path: &Path) -> Result<(), Box<dyn Error>> {
    lod.get::<lod::raw_unpacked::RawUnpacked>(file_name)?
        .dump(path.join(file_name))
}
