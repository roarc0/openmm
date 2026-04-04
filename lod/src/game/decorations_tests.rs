use super::*;
use crate::test_lod;

fn load_oute3_decorations() -> Option<Decorations> {
    let lod = test_lod()?;
    let odm = crate::odm::Odm::new(&lod, "oute3.odm").ok()?;
    Decorations::new(&lod, &odm.billboards).ok()
}

#[test]
fn decorations_loads_oute3() {
    let Some(dec) = load_oute3_decorations() else { return; };
    assert!(!dec.entries.is_empty(), "oute3 should have decorations");
}

#[test]
fn all_entries_have_sprite_name() {
    let Some(dec) = load_oute3_decorations() else { return; };
    for entry in dec.iter() {
        assert!(
            !entry.sprite_name.is_empty(),
            "every decoration should have a sprite name"
        );
    }
}

#[test]
fn directional_entries_have_zero_dimensions() {
    let Some(dec) = load_oute3_decorations() else { return; };
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
    let Some(dec) = load_oute3_decorations() else { return; };
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
