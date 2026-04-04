use super::super::global::GameData;
use super::*;
use crate::{LodManager, test_lod};

fn game_data(lod: &LodManager) -> GameData {
    GameData::new(lod).expect("GameData::new failed")
}

#[test]
fn actors_loads_oute3() {
    let Some(lod) = test_lod() else {
        return;
    };
    let gd = game_data(&lod);
    let actors = Actors::new(&lod, "oute3.odm", None, &gd).unwrap();
    assert!(!actors.get_actors().is_empty(), "oute3 should have actors");
}

#[test]
fn get_npcs_all_have_sprites() {
    let Some(lod) = test_lod() else {
        return;
    };
    let gd = game_data(&lod);
    let actors = Actors::new(&lod, "oute3.odm", None, &gd).unwrap();
    for npc in actors.get_npcs() {
        assert!(npc.is_npc());
        assert!(
            !npc.standing_sprite.is_empty(),
            "NPC '{}' should have a standing sprite",
            npc.name
        );
    }
}

#[test]
fn get_npcs_returns_only_npcs() {
    let Some(lod) = test_lod() else {
        return;
    };
    let gd = game_data(&lod);
    let actors = Actors::new(&lod, "oute3.odm", None, &gd).unwrap();
    for npc in actors.get_npcs() {
        assert!(npc.is_npc(), "get_npcs() should not return monsters");
    }
    for monster in actors.get_monsters() {
        assert!(monster.is_monster(), "get_monsters() should not return NPCs");
    }
}

#[test]
fn npc_portrait_name_format() {
    let Some(lod) = test_lod() else {
        return;
    };
    let gd = game_data(&lod);
    let actors = Actors::new(&lod, "oute3.odm", None, &gd).unwrap();
    for npc in actors.get_npcs() {
        if let Some(portrait) = &npc.portrait_name {
            assert!(
                portrait.starts_with("NPC"),
                "portrait '{}' should start with NPC",
                portrait
            );
            assert_eq!(portrait.len(), 6, "portrait '{}' should be 6 chars", portrait);
        }
    }
}

#[test]
fn state_snapshot_empty_filters_nothing() {
    let Some(lod) = test_lod() else {
        return;
    };
    let gd = game_data(&lod);
    let actors_all = Actors::new(&lod, "oute3.odm", None, &gd).unwrap();
    let all_count = actors_all.get_actors().len();
    let snapshot = MapStateSnapshot { dead_actor_ids: vec![] };
    let actors_with_state = Actors::new(&lod, "oute3.odm", Some(&snapshot), &gd).unwrap();
    assert_eq!(
        actors_with_state.get_actors().len(),
        all_count,
        "empty snapshot should not filter anything"
    );
}

#[test]
fn variant_is_precomputed() {
    let Some(lod) = test_lod() else {
        return;
    };
    let gd = game_data(&lod);
    let actors = Actors::new(&lod, "oute3.odm", None, &gd).unwrap();
    // Every actor should have variant 1, 2, or 3 — never 0 (unless there is truly only one palette).
    // The assertion checks that ALL actors have variant >= 1.
    // Actors with a unique standing_sprite will always be variant 1.
    let all_variants_nonzero = actors.get_actors().iter().all(|a| a.variant >= 1);
    assert!(
        all_variants_nonzero,
        "all actors should have variant >= 1 after pre-computation"
    );
}
