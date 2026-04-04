use super::super::global::GameData;
use super::*;
use crate::{LodManager, test_lod};

fn game_data(lod: &LodManager) -> GameData {
    GameData::new(lod).expect("GameData::new failed")
}

#[test]
fn monsters_loads_oute3() {
    let Some(lod) = test_lod() else {
        return;
    };
    let gd = game_data(&lod);
    let monsters = Monsters::new(&lod, "oute3.odm", &gd).unwrap();
    assert!(!monsters.is_empty(), "oute3 should have monster spawns");
}

#[test]
fn monsters_all_have_sprites() {
    let Some(lod) = test_lod() else {
        return;
    };
    let gd = game_data(&lod);
    let monsters = Monsters::new(&lod, "oute3.odm", &gd).unwrap();
    for m in monsters.iter() {
        assert!(
            !m.standing_sprite.is_empty(),
            "every monster should have a standing sprite"
        );
    }
}

#[test]
fn monsters_variant_in_range() {
    let Some(lod) = test_lod() else {
        return;
    };
    let gd = game_data(&lod);
    let monsters = Monsters::new(&lod, "oute3.odm", &gd).unwrap();
    for m in monsters.iter() {
        assert!(
            m.variant >= 1 && m.variant <= 3,
            "variant should be 1-3, got {}",
            m.variant
        );
    }
}

#[test]
fn monsters_group_index_in_range() {
    let Some(lod) = test_lod() else {
        return;
    };
    let gd = game_data(&lod);
    let monsters = Monsters::new(&lod, "oute3.odm", &gd).unwrap();
    // group_index is 0-based within each group; must be < 100 (sanity bound).
    for m in monsters.iter() {
        assert!(
            m.group_index < 100,
            "group_index should be reasonable: {}",
            m.group_index
        );
    }
}

#[test]
fn goblin_a_resolves_standing_sprite() {
    let Some(lod) = test_lod() else {
        return;
    };
    let gd = game_data(&lod);
    let entry = resolve_entry(0, &gd, &lod);
    assert!(entry.is_some(), "GoblinA (monlist_id=0) should resolve");
    let entry = entry.unwrap();
    assert!(!entry.standing_sprite.is_empty(), "standing sprite should not be empty");
    assert!(!entry.walking_sprite.is_empty(), "walking sprite should not be empty");
}

#[test]
fn resolve_sprite_group_goblin_standing() {
    let Some(lod) = test_lod() else {
        return;
    };
    let gd = game_data(&lod);
    let goblin_a = gd.monlist.find_by_name("Goblin", 1).unwrap();
    let group = &goblin_a.sprite_names[0];
    let result = resolve_sprite_group(group, &gd.dsft, &lod);
    assert!(result.is_some(), "GoblinA standing group '{}' should resolve", group);
}

#[test]
fn resolve_sprite_group_empty_name_returns_none() {
    let Some(lod) = test_lod() else {
        return;
    };
    let gd = game_data(&lod);
    assert!(resolve_sprite_group("", &gd.dsft, &lod).is_none());
}

#[test]
fn resolve_entry_peasant_male_is_flagged() {
    let Some(lod) = test_lod() else {
        return;
    };
    let gd = game_data(&lod);
    // PeasantM1A is monlist_id 132 (from monlist tests)
    let entry = resolve_entry(132, &gd, &lod);
    assert!(entry.is_some(), "PeasantM1A should resolve");
    let entry = entry.unwrap();
    assert!(entry.is_peasant);
    assert!(!entry.is_female);
}

#[test]
fn resolve_entry_peasant_female_is_flagged() {
    let Some(lod) = test_lod() else {
        return;
    };
    let gd = game_data(&lod);
    // PeasantF1A is monlist_id 120
    let entry = resolve_entry(120, &gd, &lod);
    assert!(entry.is_some(), "PeasantF1A should resolve");
    let entry = entry.unwrap();
    assert!(entry.is_peasant);
    assert!(entry.is_female);
}

/// Regression: `to_hit_radius` bytes 6-7 in dmonlist.bin are always 0 for MM6.
/// This field is `Radius2` (MM7+ only) — absent in MM6. Attack range is derived
/// from `radius * 2` instead.
#[test]
fn goblin_to_hit_radius_is_zero_in_mm6() {
    let Some(lod) = test_lod() else {
        return;
    };
    let gd = game_data(&lod);
    let goblin = gd.monlist.find_by_name("Goblin", 1).expect("Goblin A should exist");
    assert_eq!(
        goblin.to_hit_radius, 0,
        "MM6 dmonlist bytes 6-7 (Radius2) are 0; attack range uses radius*2"
    );
    // Radius should be non-zero — this is the physical collision size used for attack reach.
    assert!(goblin.radius > 0, "Goblin should have a non-zero radius");
}

/// GoblinA has radius=56. Confirmed from raw dmonlist.bin (row 76, 0-based index 75).
/// ArcherA is monlist[0] — Goblins start later.
#[test]
fn goblin_a_known_radius() {
    let Some(lod) = test_lod() else {
        return;
    };
    let gd = game_data(&lod);
    let g = gd.monlist.find_by_name("Goblin", 1).expect("GoblinA should exist");
    assert_eq!(g.internal_name, "GoblinA");
    assert_eq!(g.radius, 56, "GoblinA radius confirmed from dmonlist.bin");
    // ArcherA (not GoblinA) is the first entry
    assert_eq!(gd.monlist.get(0).map(|m| m.internal_name.as_str()), Some("ArcherA"));
}

/// All three Goblin variants exist in dmonlist.bin and have the same radius.
#[test]
fn goblin_abc_variants_exist_with_same_radius() {
    let Some(lod) = test_lod() else {
        return;
    };
    let gd = game_data(&lod);
    let a = gd.monlist.find_by_name("Goblin", 1).expect("GoblinA");
    let b = gd.monlist.find_by_name("Goblin", 2).expect("GoblinB");
    let c = gd.monlist.find_by_name("Goblin", 3).expect("GoblinC");
    assert_eq!(a.radius, b.radius, "GoblinA and GoblinB should have same radius");
    assert_eq!(a.radius, c.radius, "GoblinA and GoblinC should have same radius");
}

/// Per-variant display names come from monsters.txt, not the mapstats base name.
#[test]
fn monster_display_names_are_per_variant() {
    let Some(lod) = test_lod() else {
        return;
    };
    let gd = game_data(&lod);
    let monsters = Monsters::new(&lod, "oute3.odm", &gd).unwrap();
    // All PeasantM2 in oute3 with variant B should show "Journeyman Mage", not "Apprentice Mage"
    for m in monsters.iter() {
        if m.standing_sprite.starts_with("peas") {
            if m.variant == 2 {
                assert_eq!(
                    m.name, "Journeyman Mage",
                    "PeasantM2 B variant should be Journeyman Mage"
                );
            }
            if m.variant == 3 {
                assert_eq!(m.name, "Mage", "PeasantM2 C variant should be Mage");
            }
        }
    }
}

/// Expected: oute3 goblin group near (-13480,-20192) = 1 blue goblin + 4 green goblins.
/// Confirmed in original game (one run). In MM6 all 5 members independently roll variant from the
/// difficulty-1 table (A=90%, B=8%, C=2%), so the exact composition is non-deterministic per run.
/// Our deterministic pos-seeded implementation gives 5 goblins (correct size) but all variant A
/// (pos_seed%100 for each member falls in the 90% band). Keeping ignored until we decide whether
/// to reproduce the exact original RNG chain or accept "statistically equivalent" spawns.
#[test]
#[ignore = "exact variant composition non-reproducible without original LCG state — group size=5 is correct"]
fn oute3_goblin_spawn_near_player_position() {
    let Some(lod) = test_lod() else { return };
    let gd = game_data(&lod);
    let monsters = Monsters::new(&lod, "oute3.odm", &gd).unwrap();

    // Find monsters whose spawn_position matches the known goblin spawn at (-13480,-20192,0).
    let group: Vec<_> = monsters
        .iter()
        .filter(|m| m.spawn_position[0] == -13480 && m.spawn_position[1] == -20192)
        .collect();

    assert_eq!(
        group.len(),
        5,
        "goblin group near (-13480,-20192) must have 5 members, got {}",
        group.len()
    );

    let champions: Vec<_> = group.iter().filter(|m| m.group_index == 0).collect();
    let minions: Vec<_> = group.iter().filter(|m| m.group_index > 0).collect();

    assert_eq!(champions.len(), 1, "exactly 1 champion");
    assert_eq!(
        champions[0].variant, 3,
        "champion must be variant C (blue goblin), got {}",
        champions[0].variant
    );

    assert_eq!(minions.len(), 4, "exactly 4 minions");
    for m in &minions {
        assert_eq!(
            m.variant, 1,
            "minion must be variant A (green goblin), got {}",
            m.variant
        );
    }
}

/// Forced-variant spawn points (monster_index 4-12) always produce exactly 1 monster.
/// Confirmed from MM6.exe fcn_00455910: ebx=1 at function start, never updated for
/// forced cases (they jump to label_4, bypassing the Rand() group-size calculation).
#[test]
fn oute3_forced_variant_spawns_produce_one_monster() {
    let Some(lod) = test_lod() else {
        return;
    };
    let gd = game_data(&lod);
    let odm = crate::odm::Odm::new(&lod, "oute3.odm").unwrap();
    let monsters = Monsters::new(&lod, "oute3.odm", &gd).unwrap();

    // For each forced-variant spawn point, count how many monsters share its position.
    for sp in odm.spawn_points.iter().filter(|sp| sp.spawn_type == 3) {
        let cfg = gd.mapstats.get("oute3.odm").unwrap();
        if let Some((_, _, _, fv)) = cfg.monster_for_index(sp.monster_index) {
            if fv != 0 {
                let count = monsters.iter().filter(|m| m.spawn_position == sp.position).count();
                assert_eq!(
                    count, 1,
                    "forced-variant spawn at {:?} should produce exactly 1 monster, got {}",
                    sp.position, count
                );
            }
        }
    }
}

/// Spawn positions may be shared within a group; verify spawn points → monsters mapping.
#[test]
fn oute3_spawn_count_at_least_spawn_points() {
    let Some(lod) = test_lod() else {
        return;
    };
    let gd = game_data(&lod);
    let odm = crate::odm::Odm::new(&lod, "oute3.odm").unwrap();
    let monster_spawn_count = odm.spawn_points.iter().filter(|sp| sp.spawn_type == 3).count();
    let monsters = Monsters::new(&lod, "oute3.odm", &gd).unwrap();
    assert!(
        monsters.len() >= monster_spawn_count,
        "each ODM spawn point produces ≥1 monster; got {} spawns, {} monsters",
        monster_spawn_count,
        monsters.len()
    );
}
