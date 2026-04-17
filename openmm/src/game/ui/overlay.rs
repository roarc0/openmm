//! Game UI overlay lifecycle — manages screen layers that open/close on top of ingame.
//!
//! Each `UiMode` variant maps to a screen .ron file. When the mode changes,
//! the previous overlay is hidden and the new one shown. `UiMode::World`
//! means no overlay — just the ingame HUD.
//!
//! This is the single bridge between game state (UiMode) and the screen
//! system (ShowScreen/HideScreen). Individual overlay modules (npc_dialogue,
//! building, inventory, etc.) handle mode-specific logic like portrait
//! swapping or dynamic text, but this module owns the screen lifecycle.

use bevy::prelude::*;

use crate::game::optional::OptionalWrite;
use crate::screens::runtime::ScreenActions;

use super::{UiMode, UiState};

/// Maps a UiMode to its screen .ron id. Returns None for World (no overlay).
fn screen_for_mode(mode: UiMode) -> Option<&'static str> {
    match mode {
        UiMode::World => None,
        UiMode::NpcDialogue => Some("npc_speak"),
        UiMode::Building => Some("building"),
        UiMode::Chest => Some("chest"),
        UiMode::Inventory => Some("inventory"),
        UiMode::Stats => Some("stats"),
        UiMode::Rest => Some("rest"),
        UiMode::Map => Some("map"),
    }
}

/// Tracks which overlay screen is currently shown (if any).
#[derive(Resource, Default)]
pub struct ActiveOverlay(Option<&'static str>);

pub struct OverlayPlugin;

impl Plugin for OverlayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActiveOverlay>()
            .add_systems(Update, sync_overlay.run_if(in_state(crate::GameState::Game)));
    }
}

/// Sync the active overlay screen with the current UiMode.
/// Hides the old screen and shows the new one when the mode changes.
fn sync_overlay(
    ui: Res<UiState>,
    mut overlay: ResMut<ActiveOverlay>,
    mut actions: Option<bevy::ecs::message::MessageWriter<ScreenActions>>,
) {
    let desired = screen_for_mode(ui.mode);

    if overlay.0 == desired {
        return;
    }

    // Hide previous overlay.
    if let Some(old_screen) = overlay.0 {
        info!("overlay: hiding '{}'", old_screen);
        actions.try_write(ScreenActions {
            actions: vec![format!("HideScreen(\"{}\")", old_screen)],
        });
    }

    // Show new overlay.
    if let Some(new_screen) = desired {
        info!("overlay: showing '{}'", new_screen);
        actions.try_write(ScreenActions {
            actions: vec![format!("ShowScreen(\"{}\")", new_screen)],
        });
    }

    overlay.0 = desired;
}
