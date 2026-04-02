use super::*;
use crate::{LodManager, get_lod_path};

#[test]
fn npc_name_pool_parse_and_lookup() {
    let data = b"Male\tFemale\nJohn\tJane\nBob\tAlice\n";
    let pool = NpcNamePool::parse(data).unwrap();
    assert_eq!(pool.name_for(false, 0), "John");
    assert_eq!(pool.name_for(false, 1), "Bob");
    assert_eq!(pool.name_for(true, 0), "Jane");
    assert_eq!(pool.name_for(true, 1), "Alice");
    // Wraps around
    assert_eq!(pool.name_for(false, 2), "John");
}

#[test]
fn npc_name_pool_classify_name() {
    let data = b"Male\tFemale\nJohn\tJane\nBob\tAlice\n";
    let pool = NpcNamePool::parse(data).unwrap();
    assert_eq!(pool.classify_name("John"), Some(false)); // male
    assert_eq!(pool.classify_name("Jane"), Some(true));  // female
    assert_eq!(pool.classify_name("Unknown"), None);
    // Case insensitive
    assert_eq!(pool.classify_name("JOHN"), Some(false));
}

#[test]
fn npc_name_pool_empty_pool_returns_fallback() {
    let data = b"Male\tFemale\n";
    let pool = NpcNamePool::parse(data).unwrap();
    assert_eq!(pool.name_for(false, 0), "Peasant");
    assert_eq!(pool.name_for(true, 0), "Peasant");
}

#[test]
fn street_npc_table_parse_synthetic() {
    // 2 header rows, then 2 NPC entries (tab-separated, 8+ cols)
    let data = b"# header row 1\nid\tname\tpic\tstate\tfame\trep\tmap\tprof\n\
        1\tJohn Smith\t42\t0\t0\t0\t0\t52\n\
        2\tJane Doe\t81\t0\t0\t0\t0\t53\n";
    let table = StreetNpcs::parse(data, None).unwrap();
    assert_eq!(table.npcs.len(), 2);

    let e1 = table.get(1).unwrap();
    assert_eq!(e1.name, "John Smith");
    assert_eq!(e1.portrait, 42);
    assert_eq!(e1.profession_id, 52);

    assert_eq!(table.portrait_name(1), Some("NPC042".to_string()));
    assert_eq!(table.npc_name(1), Some("John Smith"));
    assert_eq!(table.npc_name(2), Some("Jane Doe"));
}

#[test]
fn street_npc_table_get_invalid_id_returns_none() {
    let data = b"h1\nh2\n1\tFoo\t10\t0\t0\t0\t0\t52\n";
    let table = StreetNpcs::parse(data, None).unwrap();
    assert!(table.get(0).is_none());  // id must be > 0
    assert!(table.get(-1).is_none()); // negative id
    assert!(table.get(99).is_none()); // non-existent
}

#[test]
fn street_npc_table_peasant_portraits_sorted_and_unique() {
    let data = b"h1\nh2\n\
        1\tAlice\t42\t0\t0\t0\t0\t52\n\
        2\tBob\t81\t0\t0\t0\t0\t55\n\
        3\tCarol\t42\t0\t0\t0\t0\t60\n"; // portrait 42 duplicate
    let table = StreetNpcs::parse(data, None).unwrap();
    // dedup'd: [42, 81]
    assert_eq!(table.peasant_portraits, vec![42, 81]);
}

#[test]
fn street_npc_table_peasant_portrait_selection_wraps() {
    let data = b"h1\nh2\n\
        1\tAlice\t10\t0\t0\t0\t0\t52\n\
        2\tBob\t20\t0\t0\t0\t0\t55\n";
    let table = StreetNpcs::parse(data, None).unwrap();
    assert_eq!(table.peasant_portrait(0), Some(10));
    assert_eq!(table.peasant_portrait(1), Some(20));
    assert_eq!(table.peasant_portrait(2), Some(10)); // wraps
}

#[test]
fn street_npc_table_from_lod() {
    let lod = LodManager::new(get_lod_path()).unwrap();
    // npcdata.txt may be zlib-compressed in the LOD archive
    let raw = lod.get_decompressed("icons/npcdata.txt").unwrap();
    let table = StreetNpcs::parse(&raw, None).unwrap();
    assert!(!table.npcs.is_empty(), "npcdata.txt should have entries");
    // NPC id 1 should exist in MM6
    let npc1 = table.get(1).expect("NPC id 1 should exist");
    assert!(!npc1.name.is_empty(), "NPC 1 should have a name");
    assert!(npc1.portrait > 0, "NPC 1 should have a portrait");
}

#[test]
fn street_npc_table_portrait_name_format() {
    let lod = LodManager::new(get_lod_path()).unwrap();
    let raw = lod.get_decompressed("icons/npcdata.txt").unwrap();
    let table = StreetNpcs::parse(&raw, None).unwrap();
    // portrait_name should format as "NPC###" with 3-digit zero-padded number
    if let Some(name) = table.portrait_name(1) {
        assert!(name.starts_with("NPC"), "portrait name should start with NPC");
        assert_eq!(name.len(), 6, "NPC+3digits = 6 chars");
    }
}
