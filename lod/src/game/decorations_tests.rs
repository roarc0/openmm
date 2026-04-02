use super::*;
use crate::{LodManager, get_lod_path};

fn load_oute3_decorations() -> Decorations {
    let lod = LodManager::new(get_lod_path()).unwrap();
    let odm = crate::odm::Odm::new(&lod, "oute3.odm").unwrap();
    Decorations::new(&lod, &odm.billboards).unwrap()
}

#[test]
fn decorations_loads_oute3() {
    let dec = load_oute3_decorations();
    assert!(!dec.entries.is_empty(), "oute3 should have decorations");
}

#[test]
fn all_entries_have_sprite_name() {
    let dec = load_oute3_decorations();
    for entry in dec.iter() {
        assert!(
            !entry.sprite_name.is_empty(),
            "every decoration should have a sprite name"
        );
    }
}

#[test]
fn directional_entries_have_zero_dimensions() {
    let dec = load_oute3_decorations();
    for entry in dec.iter().filter(|e| e.is_directional) {
        assert_eq!(
            entry.width, 0.0,
            "directional '{}' width should be 0.0",
            entry.sprite_name
        );
        assert_eq!(
            entry.height, 0.0,
            "directional '{}' height should be 0.0",
            entry.sprite_name
        );
    }
}

#[test]
fn non_directional_entries_have_dimensions() {
    let dec = load_oute3_decorations();
    let non_dir: Vec<_> = dec.iter().filter(|e| !e.is_directional).collect();
    assert!(!non_dir.is_empty(), "oute3 should have non-directional decorations");
    for entry in non_dir {
        assert!(
            entry.width > 0.0,
            "non-directional '{}' width should be >0",
            entry.sprite_name
        );
        assert!(
            entry.height > 0.0,
            "non-directional '{}' height should be >0",
            entry.sprite_name
        );
    }
}
