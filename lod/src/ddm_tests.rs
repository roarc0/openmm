use super::*;
use crate::{LodManager, get_lod_path};

#[test]
fn parse_new_sorpigal_actors() {
    let lod = LodManager::new(get_lod_path()).unwrap();
    let ddm = Ddm::new(&lod, "oute3.odm").unwrap();
    assert!(!ddm.actors.is_empty(), "oute3 DDM should have actors");
    for actor in &ddm.actors {
        assert!(!actor.name.is_empty(), "actor name should not be empty");
    }
}

#[test]
fn actor_monlist_id_is_zero_indexed() {
    let lod = LodManager::new(get_lod_path()).unwrap();
    let ddm = Ddm::new(&lod, "oute3.odm").unwrap();
    let monlist = crate::monlist::MonsterList::new(&lod).unwrap();
    for actor in &ddm.actors {
        assert!(
            (actor.monlist_id as usize) < monlist.monsters.len(),
            "monlist_id {} out of bounds (monlist has {} entries)",
            actor.monlist_id,
            monlist.monsters.len()
        );
    }
}

#[test]
fn actor_attributes_typed_access() {
    let lod = LodManager::new(get_lod_path()).unwrap();
    let ddm = Ddm::new(&lod, "oute3.odm").unwrap();
    // actor_attributes() should parse without panicking
    for actor in &ddm.actors {
        let _attrs = actor.actor_attributes();
    }
}

#[test]
fn parse_multiple_maps() {
    let lod = LodManager::new(get_lod_path()).unwrap();
    // These maps are known to have actors
    for map in &["oute3.odm", "oute2.odm"] {
        if let Ok(ddm) = Ddm::new(&lod, map) {
            assert!(!ddm.actors.is_empty(), "{} should have actors", map);
        }
    }
}
