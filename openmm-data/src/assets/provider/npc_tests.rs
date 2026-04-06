use super::*;
use crate::assets::test_lod;

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
    assert!(table.get(0).is_none()); // id must be > 0
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
    let Some(assets) = test_lod() else {
        return;
    };
    let table = StreetNpcs::load(&assets).unwrap();
    assert!(!table.npcs.is_empty(), "npcdata.txt should have entries");
    let npc1 = table.get(1).expect("NPC id 1 should exist");
    assert!(!npc1.name.is_empty(), "NPC 1 should have a name");
    assert!(npc1.portrait > 0, "NPC 1 should have a portrait");
}

#[test]
fn street_npc_table_portrait_name_format() {
    let Some(assets) = test_lod() else {
        return;
    };
    let table = StreetNpcs::load(&assets).unwrap();
    if let Some(name) = table.portrait_name(1) {
        assert!(name.starts_with("NPC"), "portrait name should start with NPC");
        assert_eq!(name.len(), 6, "NPC+3digits = 6 chars");
    }
}

#[test]
fn peasant_portrait_pools_no_shared_portraits() {
    let Some(assets) = test_lod() else { return };
    let table = StreetNpcs::load(&assets).unwrap();
    let female_set: std::collections::HashSet<u32> = table.peasant_female.iter().map(|(p, _)| *p).collect();
    let male_set: std::collections::HashSet<u32> = table.peasant_male.iter().map(|(p, _)| *p).collect();
    let shared: Vec<u32> = female_set.intersection(&male_set).copied().collect();
    assert!(
        shared.is_empty(),
        "portraits appear in both female and male pools: {:?}",
        shared
    );
}

#[test]
fn peasant_portrait_pools_nonempty() {
    let Some(assets) = test_lod() else { return };
    let table = StreetNpcs::load(&assets).unwrap();
    assert!(!table.peasant_female.is_empty(), "female peasant pool empty");
    assert!(!table.peasant_male.is_empty(), "male peasant pool empty");
}

#[test]
fn peasant_portrait_no_cross_sex_in_synthetic() {
    // Portrait 10 appears for both female (Alice) and male (Bob) — should be excluded from both.
    use crate::assets::npcnames::NpcNamePools;
    let npc_data = b"h1\nh2\n\
        1\tAlice\t10\t0\t0\t0\t0\t52\n\
        2\tBob\t10\t0\t0\t0\t0\t55\n\
        3\tCarol\t20\t0\t0\t0\t0\t60\n\
        4\tDave\t30\t0\t0\t0\t0\t62\n";
    let name_data: &[u8] = b"Male\tFemale\nBob\tAlice\nDave\tCarol\n";
    let pool = NpcNamePools::try_from(name_data).unwrap();
    let table = StreetNpcs::parse(npc_data, Some(&pool)).unwrap();
    // portrait 10 shared between Alice(female) and Bob(male) — removed from both pools
    let female_portraits: Vec<u32> = table.peasant_female.iter().map(|(p, _)| *p).collect();
    let male_portraits: Vec<u32> = table.peasant_male.iter().map(|(p, _)| *p).collect();
    assert!(
        !female_portraits.contains(&10),
        "portrait 10 should be excluded from female pool"
    );
    assert!(
        !male_portraits.contains(&10),
        "portrait 10 should be excluded from male pool"
    );
    // portrait 20 (Carol, female) and 30 (Dave, male) should be in their respective pools
    assert!(female_portraits.contains(&20));
    assert!(male_portraits.contains(&30));
}
