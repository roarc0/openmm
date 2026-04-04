use super::*;
use crate::test_lod;

fn make_map_info(monster_names: [&str; 3], difficulty: [u8; 3]) -> MapInfo {
    MapInfo {
        name: "Test Map".to_string(),
        filename: "test.odm".to_string(),
        monster_names: monster_names.map(|s| s.to_string()),
        monster_display_names: monster_names.map(|s| s.to_string()),
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
    assert!(info.monster_for_index(0).is_none());
    assert!(info.monster_for_index(13).is_none());
    assert!(info.monster_for_index(100).is_none());
}

#[test]
fn monster_for_index_forced_a_variant() {
    // Indices 4-6 always produce forced_variant=1 (A)
    let info = make_map_info(["Goblin", "Orc", "Troll"], [5, 5, 5]);
    assert_eq!(info.monster_for_index(4).map(|(_, _, _, v)| v), Some(1));
    assert_eq!(info.monster_for_index(5).map(|(_, _, _, v)| v), Some(1));
    assert_eq!(info.monster_for_index(6).map(|(_, _, _, v)| v), Some(1));
}

#[test]
fn monster_for_index_forced_b_variant() {
    let info = make_map_info(["Goblin", "Orc", "Troll"], [1, 1, 1]);
    assert_eq!(info.monster_for_index(7).map(|(_, _, _, v)| v), Some(2));
    assert_eq!(info.monster_for_index(8).map(|(_, _, _, v)| v), Some(2));
    assert_eq!(info.monster_for_index(9).map(|(_, _, _, v)| v), Some(2));
}

#[test]
fn monster_for_index_forced_c_variant() {
    let info = make_map_info(["Goblin", "Orc", "Troll"], [1, 1, 1]);
    assert_eq!(info.monster_for_index(10).map(|(_, _, _, v)| v), Some(3));
    assert_eq!(info.monster_for_index(11).map(|(_, _, _, v)| v), Some(3));
    assert_eq!(info.monster_for_index(12).map(|(_, _, _, v)| v), Some(3));
}

/// Random indices (1-3) return forced_variant=0; the caller must call roll_variant().
#[test]
fn monster_for_index_random_returns_zero_forced_variant() {
    let info = make_map_info(["Goblin", "Orc", "Troll"], [2, 2, 2]);
    assert_eq!(info.monster_for_index(1).map(|(_, _, _, v)| v), Some(0));
    assert_eq!(info.monster_for_index(2).map(|(_, _, _, v)| v), Some(0));
    assert_eq!(info.monster_for_index(3).map(|(_, _, _, v)| v), Some(0));
}

#[test]
fn monster_for_index_name_slot_mapping() {
    let info = make_map_info(["Goblin", "Orc", "Troll"], [2, 2, 2]);
    assert_eq!(info.monster_for_index(1).map(|(n, _, _, _)| n), Some("Goblin"));
    assert_eq!(info.monster_for_index(2).map(|(n, _, _, _)| n), Some("Orc"));
    assert_eq!(info.monster_for_index(3).map(|(n, _, _, _)| n), Some("Troll"));
    // Forced variants use the same slot mapping
    assert_eq!(info.monster_for_index(4).map(|(n, _, _, _)| n), Some("Goblin")); // slot 0
    assert_eq!(info.monster_for_index(8).map(|(n, _, _, _)| n), Some("Orc")); // slot 1
    assert_eq!(info.monster_for_index(12).map(|(n, _, _, _)| n), Some("Troll")); // slot 2
}

#[test]
fn monster_for_index_empty_name_returns_none() {
    let info = make_map_info(["Goblin", "", "Troll"], [2, 2, 2]);
    // Slot 1 (Mon2) is empty — indices 2, 5, 8, 11 all map to slot 1
    assert!(info.monster_for_index(2).is_none());
    assert!(info.monster_for_index(5).is_none());
}

#[test]
fn variant_from_roll_difficulty1() {
    // MM6 table for diff=1: A=90%, B=8%, C=2%.
    let info = make_map_info(["Goblin", "", ""], [1, 0, 0]);
    assert_eq!(info.variant_from_roll(0, 0), 1, "roll 0 → A");
    assert_eq!(info.variant_from_roll(0, 89), 1, "roll 89 → A (last A)");
    assert_eq!(info.variant_from_roll(0, 90), 2, "roll 90 → B (first B)");
    assert_eq!(info.variant_from_roll(0, 97), 2, "roll 97 → B (last B)");
    assert_eq!(info.variant_from_roll(0, 98), 3, "roll 98 → C (first C)");
    assert_eq!(info.variant_from_roll(0, 99), 3, "roll 99 → C");
}

#[test]
fn variant_from_roll_difficulty5() {
    // MM6 table for diff=5: A=10%, B=50%, C=40%.
    let info = make_map_info(["Goblin", "", ""], [5, 0, 0]);
    assert_eq!(info.variant_from_roll(0, 0), 1, "roll 0 → A");
    assert_eq!(info.variant_from_roll(0, 9), 1, "roll 9 → A (last A)");
    assert_eq!(info.variant_from_roll(0, 10), 2, "roll 10 → B");
    assert_eq!(info.variant_from_roll(0, 59), 2, "roll 59 → B (last B)");
    assert_eq!(info.variant_from_roll(0, 60), 3, "roll 60 → C");
    assert_eq!(info.variant_from_roll(0, 99), 3, "roll 99 → C");
}

#[test]
fn monster_display_name_differs_from_internal() {
    let Some(lod) = test_lod() else {
        return;
    };
    let stats = MapStats::new(&lod).unwrap();
    // New Sorpigal has PeasantM2 (internal) → "Apprentice Mage" (display)
    let ns = stats.get("oute3.odm").expect("oute3.odm missing");
    // PeasantM2 is mon2 (slot 1), index 2 or 5 or 8 or 11
    let (internal, display, _, _) = ns.monster_for_index(2).expect("mon2 missing");
    assert_eq!(internal, "PeasantM2");
    assert_eq!(display, "Apprentice Mage");
}

#[test]
fn mapstats_loads_from_lod() {
    let Some(lod) = test_lod() else {
        return;
    };
    let stats = MapStats::new(&lod).unwrap();
    assert!(!stats.maps.is_empty(), "mapstats should have map entries");
    // New Sorpigal (oute3.odm) should be present
    let ns = stats.get("oute3.odm");
    assert!(ns.is_some(), "oute3.odm should be in mapstats");
    let ns = ns.unwrap();
    assert!(!ns.name.is_empty(), "map name should not be empty");
}
