use super::*;
use crate::raw::test_lod;

fn load_oute3_decorations() -> Option<Decorations> {
    let lod = test_lod()?;
    let odm = crate::raw::odm::Odm::load(&lod, "oute3.odm").ok()?;
    Decorations::load(&lod, &odm.billboards).ok()
}

#[test]
fn decorations_loads_oute3() {
    let Some(dec) = load_oute3_decorations() else {
        return;
    };
    assert!(!dec.entries.is_empty(), "oute3 should have decorations");
}

#[test]
fn all_entries_have_sprite_name() {
    let Some(dec) = load_oute3_decorations() else {
        return;
    };
    for entry in dec.iter() {
        assert!(
            !entry.sprite_name.is_empty(),
            "every decoration should have a sprite name"
        );
    }
}

#[test]
fn directional_entries_have_zero_dimensions() {
    let Some(dec) = load_oute3_decorations() else {
        return;
    };
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
    let Some(dec) = load_oute3_decorations() else {
        return;
    };
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

#[test]
fn d01_torches_have_light_radius() {
    use crate::{blv::Blv, test_lod};
    let Some(lod) = test_lod() else { return; };
    let blv = match Blv::load(&lod, "d01.blv") {
        Ok(b) => b,
        Err(_) => return,
    };
    let decs = Decorations::from_blv(&lod, &blv.decorations).unwrap();
    let torches: Vec<_> = decs.iter().filter(|e| e.sprite_name.starts_with("torch")).collect();
    assert!(!torches.is_empty(), "d01 should have torch decorations");
    for t in &torches {
        assert!(t.light_radius > 0, "torch '{}' should have light_radius > 0", t.sprite_name);
    }
}

#[test]
fn campfire_has_zero_light_radius_but_exists_in_blv_lights() {
    // Campfires have light_radius=0 in ddeclist — their illumination comes from
    // the BLV static lights list (designer-placed point lights), not the decoration radius.
    use crate::{blv::Blv, test_lod};
    let Some(lod) = test_lod() else { return; };
    let blv = match Blv::load(&lod, "zddb01.blv") {
        Ok(b) => b,
        Err(_) => return,
    };
    let decs = Decorations::from_blv(&lod, &blv.decorations).unwrap();
    let campfires: Vec<_> = decs.iter().filter(|e| e.sprite_name == "campfireon").collect();
    assert!(!campfires.is_empty(), "zddb01 should have campfireon decorations");
    for c in &campfires {
        assert_eq!(c.light_radius, 0, "campfireon has no decoration light_radius (uses BLV lights instead)");
    }
    // The BLV lights list is non-empty — those provide the campfire illumination.
    assert!(!blv.lights.is_empty(), "zddb01 should have BLV static lights for campfire illumination");
    assert!(blv.lights.iter().any(|l| l.brightness >= 640), "at least one bright light (campfire) expected");
}
