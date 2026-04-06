use super::*;
use crate::assets::test_lod;

#[test]
fn parse_new_sorpigal_actors() {
    let Some(assets) = test_lod() else {
        return;
    };
    let ddm = Ddm::load(&assets, "oute3.odm").unwrap();
    assert!(!ddm.actors.is_empty(), "oute3 DDM should have actors");
    for actor in &ddm.actors {
        assert!(!actor.name.is_empty(), "actor name should not be empty");
    }
}

#[test]
fn actor_monlist_id_is_zero_indexed() {
    let Some(assets) = test_lod() else {
        return;
    };
    let ddm = Ddm::load(&assets, "oute3.odm").unwrap();
    let monlist = crate::assets::dmonlist::MonsterList::load(&assets).unwrap();
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
    let Some(assets) = test_lod() else {
        return;
    };
    let ddm = Ddm::load(&assets, "oute3.odm").unwrap();
    // actor_attributes() should parse without panicking
    for actor in &ddm.actors {
        let _attrs = actor.actor_attributes();
    }
}

#[test]
fn parse_multiple_maps() {
    let Some(assets) = test_lod() else {
        return;
    };
    // These maps are known to have actors
    for map in &["oute3.odm", "oute2.odm"] {
        if let Ok(ddm) = Ddm::load(&assets, map) {
            assert!(!ddm.actors.is_empty(), "{} should have actors", map);
        }
    }
}

/// Check what DDM actors exist near the goblin spawn at MM6 (-13480,-20192).
/// If the DDM has pre-placed goblins here (including a blue one), the group
/// composition comes from DDM, not from ODM spawn generation.
#[test]
fn oute3_ddm_actors_near_goblin_spawn() {
    let Some(assets) = test_lod() else { return };
    let ml = crate::assets::dmonlist::MonsterList::load(&assets).unwrap();
    let ddm = Ddm::load(&assets, "oute3.odm").unwrap();
    let target = (-13480_i32, -20192_i32);
    let mut nearby: Vec<_> = ddm
        .actors
        .iter()
        .filter(|a| {
            let dx = a.position[0] as i32 - target.0;
            let dy = a.position[1] as i32 - target.1;
            (dx * dx + dy * dy) < 5000 * 5000
        })
        .collect();
    nearby.sort_by_key(|a| {
        let dx = a.position[0] as i32 - target.0;
        let dy = a.position[1] as i32 - target.1;
        dx * dx + dy * dy
    });
    println!("DDM actors within 5000 units of goblin spawn (-13480,-20192):");
    for a in &nearby {
        let name = ml
            .get(a.monlist_id as usize)
            .map(|m| m.internal_name.as_str())
            .unwrap_or("?");
        let dx = a.position[0] as i32 - target.0;
        let dy = a.position[1] as i32 - target.1;
        let dist = ((dx * dx + dy * dy) as f64).sqrt() as i32;
        println!(
            "  dist={:5} pos=({},{},{}) monlist_id={} internal={} name={:?}",
            dist, a.position[0], a.position[1], a.position[2], a.monlist_id, name, a.name
        );
    }
    println!("Total nearby: {}", nearby.len());
}
