use crate::{
    dtile::{Dtile, Tileset},
    odm::Odm,
    test_lod,
};

#[test]
fn read_dtile_data_works() {
    let Some(lod_manager) = test_lod() else {
        return;
    };
    let dtile = Dtile::new(&lod_manager).unwrap();
    assert_eq!(dtile.tiles.len(), 882);
}

#[test]
fn atlas_generation_works() {
    let Some(lod_manager) = test_lod() else {
        return;
    };
    let map = Odm::new(&lod_manager, "oute3.odm").unwrap();
    let dtile = Dtile::new(&lod_manager).unwrap();

    let tile_table = dtile.table(map.tile_data).unwrap();
    tile_table
        .atlas_image(&lod_manager)
        .unwrap()
        .save("terrain_atlas.png")
        .unwrap();
}

#[test]
fn tileset_lookup_grass_map() {
    let Some(lod_manager) = test_lod() else {
        return;
    };
    let odm = Odm::new(&lod_manager, "oute3.odm").unwrap();
    let dtile = Dtile::new(&lod_manager).unwrap();
    let lookup = dtile.tileset_lookup(odm.tile_data);

    // tile_map=90 is the first primary terrain tile on oute3 (a grass map)
    assert_eq!(
        Tileset::from_raw(lookup[90]),
        Some(Tileset::Grass),
        "primary terrain tiles (90-124) should be Grass on oute3"
    );

    // tile_map=1 is a dirt tile
    assert_eq!(
        Tileset::from_raw(lookup[1]),
        Some(Tileset::Dirt),
        "base tiles (1-12) should be Dirt"
    );

    // tile_map=126 is water
    assert_eq!(
        Tileset::from_raw(lookup[126]),
        Some(Tileset::Water),
        "water tiles (126-161) should be Water"
    );
}

#[test]
fn tileset_lookup_snow_map() {
    let Some(lod_manager) = test_lod() else {
        return;
    };
    let odm = Odm::new(&lod_manager, "outc1.odm").unwrap();
    let dtile = Dtile::new(&lod_manager).unwrap();
    println!("outc1 tile_data: {:?}", odm.tile_data);

    let lookup = dtile.tileset_lookup(odm.tile_data);

    // Dump primary terrain entries
    for i in 90..95 {
        let dtile_idx = i - 90 + odm.tile_data[1] as usize;
        let info = dtile.tile_info(dtile_idx);
        println!(
            "tile_map={} -> dtile[{}] name={:?} tile_set={} -> {:?}",
            i,
            dtile_idx,
            info.0,
            info.1,
            Tileset::from_raw(lookup[i])
        );
    }

    // outc1 is a snow map — primary terrain should be Snow
    assert_eq!(
        Tileset::from_raw(lookup[90]),
        Some(Tileset::Snow),
        "primary terrain tiles (90-124) should be Snow on outc1"
    );
}

#[test]
fn from_raw_covers_all_tilesets() {
    // MM6 dtile.bin raw values
    assert_eq!(Tileset::from_raw(0), Some(Tileset::Grass));
    assert_eq!(Tileset::from_raw(1), Some(Tileset::Snow));
    assert_eq!(Tileset::from_raw(2), Some(Tileset::Desert));
    assert_eq!(Tileset::from_raw(3), Some(Tileset::Volcanic)); // voltyl
    assert_eq!(Tileset::from_raw(4), Some(Tileset::Dirt));
    assert_eq!(Tileset::from_raw(5), Some(Tileset::Water));
    assert_eq!(Tileset::from_raw(6), Some(Tileset::CrackedSwamp)); // crktyl
    assert_eq!(Tileset::from_raw(7), Some(Tileset::Swamp)); // swmtyl
    assert_eq!(Tileset::from_raw(8), Some(Tileset::Road));
    assert_eq!(Tileset::from_raw(22), Some(Tileset::Road));
    assert_eq!(Tileset::from_raw(-1), None);
}
