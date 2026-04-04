use super::MonsterList;
use crate::test_lod;

/// Peasant detection and gender must be derived from monlist internal_name,
/// not from hardcoded ranges. Verifies against actual dmonlist.bin data.
#[test]
fn peasant_detection_from_monlist_data() {
    let Some(lod_manager) = test_lod() else {
        return;
    };
    let monlist = MonsterList::new(&lod_manager).unwrap();

    // PeasantF1A-C (ids 120-122) must be detected as female peasants
    for id in 120..=122u8 {
        assert!(monlist.is_peasant(id), "monlist_id={} should be peasant", id);
        assert!(monlist.is_female_peasant(id), "monlist_id={} should be female", id);
    }
    // PeasantM1A-C (ids 132-134) must be detected as male peasants
    for id in 132..=134u8 {
        assert!(monlist.is_peasant(id), "monlist_id={} should be peasant", id);
        assert!(!monlist.is_female_peasant(id), "monlist_id={} should be male", id);
    }
    // Non-peasant types (Goblin=0, Ooze=114, Rat=144) must NOT be peasants
    for id in [0u8, 114, 144] {
        assert!(!monlist.is_peasant(id), "monlist_id={} should NOT be peasant", id);
    }
}

/// Table-driven test: find_by_name must return the correct A/B/C variant.
/// Each row is (monster_name, difficulty, expected_suffix).
/// Add new rows as we discover variant resolution issues.
#[test]
fn find_by_name_returns_correct_variant() {
    let Some(lod_manager) = test_lod() else {
        return;
    };
    let monlist = MonsterList::new(&lod_manager).unwrap();

    let test_cases: &[(&str, u8, &str)] = &[
        // (monster_name, difficulty, expected_internal_name_suffix)
        ("Goblin", 1, "A"),
        ("Goblin", 2, "B"),
        ("Goblin", 3, "C"),
        ("Ghost", 1, "A"),
        ("Ghost", 2, "B"),
        ("Ghost", 3, "C"),
        ("Skeleton", 1, "A"),
        ("Skeleton", 2, "B"),
        ("Skeleton", 3, "C"),
        ("Spider", 1, "A"),
        ("Spider", 2, "B"),
    ];

    for &(name, dif, expected_suffix) in test_cases {
        let desc = monlist
            .find_by_name(name, dif)
            .unwrap_or_else(|| panic!("{} difficulty {} should exist", name, dif));
        assert!(
            desc.internal_name.ends_with(expected_suffix),
            "{} difficulty {} should return variant {}, got '{}'",
            name,
            dif,
            expected_suffix,
            desc.internal_name
        );
    }
}

/// Variants A vs B/C must have different standing sprite groups.
/// Regression: find_with_sprite fell back to variant A for B/C,
/// giving all variants the same sprite group and palette.
#[test]
fn variants_have_distinct_sprite_groups() {
    let Some(lod_manager) = test_lod() else {
        return;
    };
    let monlist = MonsterList::new(&lod_manager).unwrap();

    // (monster_name, variants that must have distinct sprite groups)
    let test_cases: &[(&str, &[u8])] = &[("Goblin", &[1, 2, 3]), ("Ghost", &[1, 2, 3]), ("Skeleton", &[1, 2, 3])];

    for &(name, variants) in test_cases {
        let descs: Vec<_> = variants
            .iter()
            .filter_map(|&dif| monlist.find_by_name(name, dif))
            .collect();

        // All requested variants should exist
        assert_eq!(
            descs.len(),
            variants.len(),
            "{} should have {} variants, found {}",
            name,
            variants.len(),
            descs.len()
        );

        // Standing sprite groups should all differ
        for i in 0..descs.len() {
            for j in (i + 1)..descs.len() {
                assert_ne!(
                    descs[i].sprite_names[0], descs[j].sprite_names[0],
                    "{}: variant '{}' and '{}' should have different standing sprite groups",
                    name, descs[i].internal_name, descs[j].internal_name
                );
            }
        }
    }
}

/// Regression: ghost walking and standing groups must resolve to the same
/// sprite root and DSFT palette_id. Before the cache-key fix, the walking
/// animation used a stale preload entry (palette_id=0 offset path) while
/// standing decoded fresh with the correct DSFT palette — causing the ghost
/// to display the wrong color during walking in outc3.
#[test]
fn ghost_walking_and_standing_palette_match() {
    let Some(lod) = test_lod() else {
        return;
    };
    let gd = crate::game::global::GameData::new(&lod).unwrap();
    for dif in 1..=3u8 {
        let desc = gd
            .monlist
            .find_by_name("Ghost", dif)
            .unwrap_or_else(|| panic!("Ghost variant {} should exist", dif));
        let st_group = &desc.sprite_names[0];
        let wa_group = &desc.sprite_names[1];
        let st = crate::game::monster::resolve_sprite_group(st_group, &gd.dsft, &lod)
            .unwrap_or_else(|| panic!("Ghost {} standing group '{}' should resolve", dif, st_group));
        let wa = crate::game::monster::resolve_sprite_group(wa_group, &gd.dsft, &lod)
            .unwrap_or_else(|| panic!("Ghost {} walking group '{}' should resolve", dif, wa_group));
        assert_eq!(
            st.0, wa.0,
            "Ghost {}: standing root '{}' should match walking root '{}'",
            dif, st.0, wa.0
        );
        assert_eq!(
            st.1, wa.1,
            "Ghost {}: standing palette_id {} should match walking palette_id {} — mismatch causes wrong color during walking animation",
            dif, st.1, wa.1
        );
        // B/C variants must have non-zero DSFT palette_id so sprite_with_palette is used
        if dif > 1 {
            assert!(
                st.1 > 0,
                "Ghost {} DSFT palette_id should be non-zero (got {}), otherwise sprite-header offset path is used instead",
                dif,
                st.1
            );
        }
    }
}
