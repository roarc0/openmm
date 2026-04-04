use super::super::global::GameData;
use super::*;
use crate::{LodManager, test_lod};

fn game_data(lod: &LodManager) -> GameData {
    GameData::new(lod).expect("GameData::new failed")
}

#[test]
fn monsters_loads_oute3() {
    let Some(lod) = test_lod() else { return; };
    let gd = game_data(&lod);
    let monsters = Monsters::new(&lod, "oute3.odm", &gd).unwrap();
    assert!(!monsters.is_empty(), "oute3 should have monster spawns");
}

#[test]
fn monsters_all_have_sprites() {
    let Some(lod) = test_lod() else { return; };
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
    let Some(lod) = test_lod() else { return; };
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
fn monsters_group_index_within_group_size() {
    let Some(lod) = test_lod() else { return; };
    let gd = game_data(&lod);
    let monsters = Monsters::new(&lod, "oute3.odm", &gd).unwrap();
    // group_size is 3..=5, so group_index must be < 6
    for m in monsters.iter() {
        assert!(m.group_index < 6, "group_index {} seems too large", m.group_index);
    }
}

#[test]
fn goblin_a_resolves_standing_sprite() {
    let Some(lod) = test_lod() else { return; };
    let gd = game_data(&lod);
    let entry = resolve_entry(0, &gd, &lod);
    assert!(entry.is_some(), "GoblinA (monlist_id=0) should resolve");
    let entry = entry.unwrap();
    assert!(!entry.standing_sprite.is_empty(), "standing sprite should not be empty");
    assert!(!entry.walking_sprite.is_empty(), "walking sprite should not be empty");
}

#[test]
fn resolve_sprite_group_goblin_standing() {
    let Some(lod) = test_lod() else { return; };
    let gd = game_data(&lod);
    let goblin_a = gd.monlist.find_by_name("Goblin", 1).unwrap();
    let group = &goblin_a.sprite_names[0];
    let result = resolve_sprite_group(group, &gd.dsft, &lod);
    assert!(result.is_some(), "GoblinA standing group '{}' should resolve", group);
}

#[test]
fn resolve_sprite_group_empty_name_returns_none() {
    let Some(lod) = test_lod() else { return; };
    let gd = game_data(&lod);
    assert!(resolve_sprite_group("", &gd.dsft, &lod).is_none());
}

#[test]
fn resolve_entry_peasant_male_is_flagged() {
    let Some(lod) = test_lod() else { return; };
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
    let Some(lod) = test_lod() else { return; };
    let gd = game_data(&lod);
    // PeasantF1A is monlist_id 120
    let entry = resolve_entry(120, &gd, &lod);
    assert!(entry.is_some(), "PeasantF1A should resolve");
    let entry = entry.unwrap();
    assert!(entry.is_peasant);
    assert!(entry.is_female);
}
