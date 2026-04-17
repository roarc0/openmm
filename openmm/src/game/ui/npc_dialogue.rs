//! NPC dialogue screen overlay — layers npc_speak.ron on top of ingame.
//!
//! Uses the screen system's ShowScreen/HideScreen actions to layer the
//! npc_speak screen on top of ingame when UiMode::NpcDialogue activates.
//! Portrait texture is swapped dynamically after the screen loads.

use bevy::prelude::*;

use crate::game::actors::npc_dialogue::NpcPortrait;
use crate::game::optional::OptionalWrite;
use crate::screens::runtime::{RuntimeElement, ScreenActions};

use super::{UiMode, UiState};

const NPC_SPEAK_SCREEN: &str = "npc_speak";
/// Element ID of the portrait placeholder in npc_speak.ron.
const PORTRAIT_ELEMENT_ID: &str = "icons/NPC001";

/// Tracks whether the npc_speak screen overlay is currently active.
#[derive(Resource, Default)]
struct NpcDialogueActive(bool);

pub struct NpcDialoguePlugin;

impl Plugin for NpcDialoguePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NpcDialogueActive>().add_systems(
            Update,
            (show_npc_screen, hide_npc_screen, swap_npc_portrait).run_if(in_state(crate::GameState::Game)),
        );
    }
}

/// When entering NpcDialogue mode, send ShowScreen action to layer npc_speak.
fn show_npc_screen(
    ui: Res<UiState>,
    mut active: ResMut<NpcDialogueActive>,
    mut actions: Option<bevy::ecs::message::MessageWriter<ScreenActions>>,
) {
    if ui.mode != UiMode::NpcDialogue || active.0 {
        return;
    }
    active.0 = true;
    info!("NPC dialogue: showing {} screen overlay", NPC_SPEAK_SCREEN);
    actions.try_write(ScreenActions {
        actions: vec![format!("ShowScreen(\"{}\")", NPC_SPEAK_SCREEN)],
    });
}

/// When leaving NpcDialogue mode, send HideScreen action to remove npc_speak.
fn hide_npc_screen(
    ui: Res<UiState>,
    mut active: ResMut<NpcDialogueActive>,
    mut actions: Option<bevy::ecs::message::MessageWriter<ScreenActions>>,
) {
    if ui.mode == UiMode::NpcDialogue || !active.0 {
        return;
    }
    active.0 = false;
    info!("NPC dialogue: hiding {} screen overlay", NPC_SPEAK_SCREEN);
    actions.try_write(ScreenActions {
        actions: vec![format!("HideScreen(\"{}\")", NPC_SPEAK_SCREEN)],
    });
}

/// After the npc_speak screen loads, swap the placeholder portrait texture
/// with the actual NPC portrait from the NpcPortrait resource.
fn swap_npc_portrait(
    active: Res<NpcDialogueActive>,
    portrait_res: Option<Res<NpcPortrait>>,
    mut query: Query<(&RuntimeElement, &mut ImageNode)>,
) {
    if !active.0 {
        return;
    }
    let Some(portrait) = portrait_res else {
        return;
    };

    for (elem, mut image_node) in query.iter_mut() {
        if elem.screen_id != NPC_SPEAK_SCREEN || elem.element_id != PORTRAIT_ELEMENT_ID {
            continue;
        }
        if image_node.image != portrait.image {
            image_node.image = portrait.image.clone();
            info!("NPC dialogue: swapped portrait texture");
        }
    }
}
