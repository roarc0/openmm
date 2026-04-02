use super::*;
use crate::{LodManager, get_lod_path};

fn make_map_info(monster_names: [&str; 3], difficulty: [u8; 3]) -> MapInfo {
    MapInfo {
        name: "Test Map".to_string(),
        filename: "test.odm".to_string(),
        monster_names: monster_names.map(|s| s.to_string()),
        difficulty,
        reset_count: 0,
        first_visit_day: 0,
        respawn_days: 7,
        lock: 0,
        trap_d20_count: 0,
        treasure_level: 0,
        encounter_chance: 10,
        encounter_chances: [50, 30, 20],
        encounter_min: [1, 1, 1],
        encounter_max: [3, 3, 3],
        music_track: 0,
    }
}

#[test]
fn parse_count_range_dash_separated() {
    assert_eq!(parse_count_range("2-4"), (2, 4));
    assert_eq!(parse_count_range(" 1-3 "), (1, 3));
    assert_eq!(parse_count_range("0-10"), (0, 10));
}

#[test]
fn parse_count_range_single_value() {
    assert_eq!(parse_count_range("3"), (3, 3));
    assert_eq!(parse_count_range("0"), (0, 0));
}

#[test]
fn parse_count_range_empty_is_zero() {
    assert_eq!(parse_count_range(""), (0, 0));
    assert_eq!(parse_count_range("  "), (0, 0));
}

#[test]
fn monster_for_index_out_of_range_returns_none() {
    let info = make_map_info(["Goblin", "Orc", "Troll"], [2, 2, 2]);
    assert!(info.monster_for_index(0, 0).is_none());
    assert!(info.monster_for_index(13, 0).is_none());
    assert!(info.monster_for_index(100, 0).is_none());
}

#[test]
fn monster_for_index_forced_a_variant() {
    // Indices 4-6 always produce variant 1 (A), regardless of seed
    let info = make_map_info(["Goblin", "Orc", "Troll"], [5, 5, 5]);
    assert_eq!(info.monster_for_index(4, 0).map(|(_, v)| v), Some(1));
    assert_eq!(info.monster_for_index(5, 99999).map(|(_, v)| v), Some(1));
    assert_eq!(info.monster_for_index(6, 42).map(|(_, v)| v), Some(1));
}

#[test]
fn monster_for_index_forced_b_variant() {
    let info = make_map_info(["Goblin", "Orc", "Troll"], [1, 1, 1]);
    assert_eq!(info.monster_for_index(7, 0).map(|(_, v)| v), Some(2));
    assert_eq!(info.monster_for_index(8, 12345).map(|(_, v)| v), Some(2));
    assert_eq!(info.monster_for_index(9, 99999).map(|(_, v)| v), Some(2));
}

#[test]
fn monster_for_index_forced_c_variant() {
    let info = make_map_info(["Goblin", "Orc", "Troll"], [1, 1, 1]);
    assert_eq!(info.monster_for_index(10, 0).map(|(_, v)| v), Some(3));
    assert_eq!(info.monster_for_index(11, 12345).map(|(_, v)| v), Some(3));
    assert_eq!(info.monster_for_index(12, 99999).map(|(_, v)| v), Some(3));
}

#[test]
fn monster_for_index_name_slot_mapping() {
    let info = make_map_info(["Goblin", "Orc", "Troll"], [2, 2, 2]);
    assert_eq!(info.monster_for_index(1, 0).map(|(n, _)| n), Some("Goblin"));
    assert_eq!(info.monster_for_index(2, 0).map(|(n, _)| n), Some("Orc"));
    assert_eq!(info.monster_for_index(3, 0).map(|(n, _)| n), Some("Troll"));
    // Forced variants use the same slot mapping
    assert_eq!(info.monster_for_index(4, 0).map(|(n, _)| n), Some("Goblin")); // slot 0
    assert_eq!(info.monster_for_index(8, 0).map(|(n, _)| n), Some("Orc"));   // slot 1
    assert_eq!(info.monster_for_index(12, 0).map(|(n, _)| n), Some("Troll")); // slot 2
}

#[test]
fn monster_for_index_empty_name_returns_none() {
    let info = make_map_info(["Goblin", "", "Troll"], [2, 2, 2]);
    // Slot 1 (Mon2) is empty — indices 2, 5, 8, 11 all map to slot 1
    assert!(info.monster_for_index(2, 0).is_none());
    assert!(info.monster_for_index(5, 0).is_none());
}

#[test]
fn monster_for_index_difficulty_0_always_a() {
    // Difficulty 0 => 100% A variant
    let info = make_map_info(["Goblin", "Orc", "Troll"], [0, 0, 0]);
    for seed in [0u32, 1, 100, 99999, u32::MAX] {
        assert_eq!(info.monster_for_index(1, seed).map(|(_, v)| v), Some(1));
    }
}

#[test]
fn mapstats_loads_from_lod() {
    let lod = LodManager::new(get_lod_path()).unwrap();
    let stats = MapStats::new(&lod).unwrap();
    assert!(!stats.maps.is_empty(), "mapstats should have map entries");
    // New Sorpigal (oute3.odm) should be present
    let ns = stats.get("oute3.odm");
    assert!(ns.is_some(), "oute3.odm should be in mapstats");
    let ns = ns.unwrap();
    assert!(!ns.name.is_empty(), "map name should not be empty");
}
