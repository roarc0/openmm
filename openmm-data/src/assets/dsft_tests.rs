use crate::assets::test_lod;

use super::DSFT;

#[test]
fn read_declist_data_works() {
    let Some(assets) = test_lod() else {
        return;
    };
    let dsft = DSFT::load(&assets).unwrap();
    assert_eq!(dsft.frames.len(), 6455);
    assert_eq!(dsft.frames[0].group_name(), Some("null".to_string()));
    assert_eq!(dsft.frames[1].group_name(), Some("key".to_string()));
    assert_eq!(dsft.frames[1].sprite_name(), Some("3gem7".to_string()));
    assert_eq!(dsft.frames[1017].group_name(), Some("rok1".to_string()));
    assert_eq!(dsft.frames[1017].sprite_name(), Some("rok1".to_string()));
    assert_eq!(dsft.groups.len(), 1656);
}

/// Helper: find the first DSFT frame matching a group name (case-insensitive).
fn find_frame_by_group<'a>(dsft: &'a DSFT, group: &str) -> Option<&'a super::DSFTFrame> {
    dsft.frames
        .iter()
        .find(|f| f.group_name().map(|g| g.eq_ignore_ascii_case(group)).unwrap_or(false))
}

/// Goblin variants A/B/C must resolve to different DSFT palette IDs.
/// Regression: find_with_sprite was falling back to variant A for B/C,
/// causing all goblins to get palette_id=225 (variant A's palette).
#[test]
fn goblin_variants_have_distinct_dsft_palettes() {
    let Some(assets) = test_lod() else {
        return;
    };
    let dsft = DSFT::load(&assets).unwrap();
    let monlist = crate::assets::dmonlist::MonsterList::load(&assets).unwrap();

    // Resolve goblin A, B, C through monlist + DSFT
    let gob_a = monlist.find_by_name("Goblin", 1).expect("Goblin A should exist");
    let gob_b = monlist.find_by_name("Goblin", 2).expect("Goblin B should exist");
    let gob_c = monlist.find_by_name("Goblin", 3).expect("Goblin C should exist");

    // Each variant should have a different internal name
    assert_ne!(gob_a.internal_name, gob_b.internal_name);
    assert_ne!(gob_b.internal_name, gob_c.internal_name);

    // Their standing sprite group names should differ
    let st_a = &gob_a.sprite_names[0];
    let st_b = &gob_b.sprite_names[0];
    let st_c = &gob_c.sprite_names[0];
    assert_ne!(st_a, st_b, "goblin A/B standing sprites should differ");
    assert_ne!(st_b, st_c, "goblin B/C standing sprites should differ");

    // Lookup DSFT frames for each variant's standing group
    let frame_a = find_frame_by_group(&dsft, st_a).expect("DSFT frame for goblin A");
    let frame_b = find_frame_by_group(&dsft, st_b).expect("DSFT frame for goblin B");
    let frame_c = find_frame_by_group(&dsft, st_c).expect("DSFT frame for goblin C");

    // Variant B and C must have different palette IDs than variant A
    assert_ne!(
        frame_a.palette_id, frame_b.palette_id,
        "goblin A ({}) and B ({}) should have different DSFT palette IDs",
        st_a, st_b
    );
    assert_ne!(
        frame_a.palette_id, frame_c.palette_id,
        "goblin A ({}) and C ({}) should have different DSFT palette IDs",
        st_a, st_c
    );
}

/// Campfire DSFT frame names — regression test to verify frame sprite names resolve in LOD.
#[test]
fn campfire_dsft_frame_names_resolve() {
    let Some(assets) = test_lod() else {
        return;
    };
    let dsft = DSFT::load(&assets).unwrap();
    let ddeclist = crate::assets::ddeclist::DDecList::load(&assets).unwrap();

    let (id, item) = ddeclist
        .items
        .iter()
        .enumerate()
        .find_map(|(i, it)| {
            it.name()
                .filter(|n| n.eq_ignore_ascii_case("campfireon"))
                .map(|_| (i as u16, it))
        })
        .expect("campfireon should exist in ddeclist");

    let sft_idx = item.sft_index();
    assert!(
        sft_idx >= 0,
        "campfireon sft_index should be non-negative, got {}",
        sft_idx
    );
    println!("campfireon: ddeclist_id={}, sft_index={}", id, sft_idx);

    let mut idx = sft_idx as usize;
    let mut frame_count = 0;
    loop {
        let frame = &dsft.frames[idx];
        let sprite_name = frame.sprite_name().unwrap_or_default();
        // Verify each frame's sprite exists in the LOD
        let found = assets.game().sprite(&sprite_name).is_some();
        assert!(
            found,
            "campfireon frame {} sprite '{}' should exist in LOD",
            frame_count, sprite_name
        );
        // All campfire frames must be luminous with light_radius=256 —
        // this is the source of campfire point lights, NOT ddeclist.light_radius (which is 0).
        assert!(
            frame.is_luminous(),
            "campfireon frame {} should be luminous",
            frame_count
        );
        assert_eq!(
            frame.light_radius, 256,
            "campfireon frame {} light_radius should be 256",
            frame_count
        );
        if !frame.is_not_group_end() {
            break;
        }
        idx += 1;
        frame_count += 1;
    }
    assert_eq!(
        frame_count + 1,
        6,
        "campfireon should have 6 animation frames, got {}",
        frame_count + 1
    );
}

/// Print all ddeclist entries where ddeclist.light_radius=0 but the DSFT first frame is luminous.
/// These decorations need SelfLit even though they have no ddeclist light — their light
/// comes from the DSFT frame (like campfireon).
#[test]
fn static_luminous_decorations_with_zero_ddeclist_light_radius() {
    let Some(assets) = test_lod() else {
        return;
    };
    let dsft = DSFT::load(&assets).unwrap();
    let ddeclist = crate::assets::ddeclist::DDecList::load(&assets).unwrap();
    let mut found = vec![];
    for (id, item) in ddeclist.items.iter().enumerate() {
        if item.light_radius != 0 {
            continue;
        }
        let sft_idx = item.sft_index();
        if sft_idx < 0 {
            continue;
        }
        let Some(frame) = dsft.frames.get(sft_idx as usize) else {
            continue;
        };
        if frame.is_luminous() && frame.light_radius > 0 {
            found.push((id, item.name().unwrap_or_default(), frame.light_radius));
        }
    }
    for (id, name, lr) in &found {
        println!("  ddeclist[{}] '{}': ddeclist lr=0, DSFT luminous lr={}", id, name, lr);
    }
    // We know campfireon (id=168) is in this list — it's the canonical case.
    assert!(
        found.iter().any(|(_, n, _)| n.eq_ignore_ascii_case("campfireon")),
        "campfireon should appear as static-luminous with zero ddeclist lr"
    );
}

/// Ghost/skeleton DSFT palette_id must differ from sprite file header palette.
/// Regression: using sprite header palette offset gave wrong colors for ghosts.
#[test]
fn ghost_dsft_palette_differs_from_header() {
    let Some(assets) = test_lod() else {
        return;
    };
    let dsft = DSFT::load(&assets).unwrap();
    let monlist = crate::assets::dmonlist::MonsterList::load(&assets).unwrap();

    // Ghost variant B should have a non-zero DSFT palette_id
    let ghost_b = monlist.find_by_name("Ghost", 2).expect("Ghost B should exist");
    let st_group = &ghost_b.sprite_names[0];
    let frame = find_frame_by_group(&dsft, st_group).expect("DSFT frame for ghost B");
    assert!(
        frame.palette_id > 0,
        "ghost B DSFT palette_id should be non-zero (got {})",
        frame.palette_id
    );

    // The DSFT palette_id is the authoritative palette for variant coloring.
    // Verify we can actually load a sprite with this palette.
    let sprite_name = frame.sprite_name().expect("ghost B should have sprite name");
    let root = sprite_name.trim_end_matches(|c: char| c.is_ascii_digit());
    let root = if root.ends_with(|c: char| c >= 'a' && c <= 'f') {
        &root[..root.len() - 1]
    } else {
        root
    };
    let test_name = format!("{}a0", root);
    let img = assets.game().sprite_with_palette(&test_name, frame.palette_id as u16);
    assert!(
        img.is_some(),
        "should load ghost sprite with DSFT palette {}",
        frame.palette_id
    );
}

/// Monster variants A/B/C must resolve through monlist → DSFT → LOD sprite files.
/// Each variant should have a valid DSFT entry with a sprite that exists in the LOD.
/// Table-driven so we can easily add more monsters as we discover issues.
///
/// Regressions caught:
/// - Goblin B/C getting variant A's palette (find_with_sprite fallback)
/// - Ghost/skeleton wrong palette (sprite header vs DSFT palette mismatch)
/// - Lizardman B/C failing to load (DSFT group names ≠ file names)
#[test]
fn monster_variants_resolve_through_dsft() {
    let Some(assets) = test_lod() else {
        return;
    };
    let dsft = DSFT::load(&assets).unwrap();
    let monlist = crate::assets::dmonlist::MonsterList::load(&assets).unwrap();

    // (monster_name, variants_to_test, expect_distinct_palettes)
    // expect_distinct_palettes: if true, variant B/C must have different palette_id than A
    let test_cases: &[(&str, &[u8], bool)] = &[
        ("Goblin", &[1, 2, 3], true),
        ("Ghost", &[1, 2, 3], true),
        ("Skeleton", &[1, 2, 3], true),
        ("Spider", &[1, 2, 3], false),
    ];

    for &(monster_name, variants, expect_distinct) in test_cases {
        let mut palette_ids: Vec<(u8, i16)> = Vec::new();

        for &dif in variants {
            let variant_letter = match dif {
                1 => "A",
                2 => "B",
                _ => "C",
            };
            let desc = match monlist.find_by_name(monster_name, dif) {
                Some(d) => d,
                None => {
                    // Some monsters may not have all 3 variants — skip
                    continue;
                }
            };

            // Verify the correct variant suffix was returned
            let expected_suffix = variant_letter;
            assert!(
                desc.internal_name.ends_with(expected_suffix),
                "{} difficulty {} should return variant {}, got '{}'",
                monster_name,
                dif,
                expected_suffix,
                desc.internal_name
            );

            let st_group = &desc.sprite_names[0];
            assert!(
                !st_group.is_empty(),
                "{} {} standing sprite should be non-empty",
                monster_name,
                variant_letter
            );

            // DSFT must have a frame for this group
            let frame = match find_frame_by_group(&dsft, st_group) {
                Some(f) => f,
                None => {
                    panic!(
                        "DSFT frame for {} {} group '{}' should exist",
                        monster_name, variant_letter, st_group
                    );
                }
            };

            palette_ids.push((dif, frame.palette_id));

            // The DSFT sprite_name should resolve to a real file in the LOD
            let sprite_name = frame
                .sprite_name()
                .unwrap_or_else(|| panic!("{} {} DSFT frame should have sprite name", monster_name, variant_letter));
            let root = sprite_name.trim_end_matches(|c: char| c.is_ascii_digit());
            let root = if root.ends_with(|c: char| c >= 'a' && c <= 'f') {
                &root[..root.len() - 1]
            } else {
                root
            };
            let test = format!("sprites/{}a0", root.to_lowercase());
            assert!(
                assets.get_bytes(&test).is_ok(),
                "{} {} sprite '{}' (from DSFT group '{}') should exist in LOD",
                monster_name,
                variant_letter,
                test,
                st_group
            );
        }

        // Check palette distinctness for monsters that need it
        if expect_distinct && palette_ids.len() >= 2 {
            let a_palette = palette_ids.iter().find(|(d, _)| *d == 1).map(|(_, p)| *p);
            for &(dif, pal) in &palette_ids {
                if dif > 1 {
                    if let Some(a_pal) = a_palette {
                        assert_ne!(
                            a_pal,
                            pal,
                            "{} variant {} palette_id ({}) should differ from variant A ({})",
                            monster_name,
                            match dif {
                                2 => "B",
                                _ => "C",
                            },
                            pal,
                            a_pal
                        );
                    }
                }
            }
        }
    }
}
