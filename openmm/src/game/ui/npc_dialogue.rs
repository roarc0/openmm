//! NPC dialogue overlay details — swaps portrait texture on the npc_speak screen.
//!
//! Screen lifecycle (show/hide) is handled by `overlay.rs`. This module
//! only does mode-specific work: swapping the placeholder portrait texture.

use bevy::prelude::*;

use crate::game::actors::npc_dialogue::NpcPortrait;
use crate::screens::runtime::RuntimeElement;

use super::{UiMode, UiState};

const NPC_SPEAK_SCREEN: &str = "npc_speak";
/// Element ID of the portrait placeholder in npc_speak.ron.
const PORTRAIT_ELEMENT_ID: &str = "icons/NPC001";

pub struct NpcDialoguePlugin;

impl Plugin for NpcDialoguePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            swap_npc_portrait
                .run_if(in_state(crate::GameState::Game))
                .run_if(|ui: Res<UiState>| ui.mode == UiMode::NpcDialogue),
        );
    }
}

/// Swap the placeholder portrait texture with the actual NPC portrait.
fn swap_npc_portrait(portrait_res: Option<Res<NpcPortrait>>, mut query: Query<(&RuntimeElement, &mut ImageNode)>) {
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
