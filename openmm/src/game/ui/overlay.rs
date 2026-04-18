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

/// Maps a UiMode to its overlay screen .ron id.
///
/// The base `ingame` screen is always visible (borders, frame).
/// This returns the *middle layer* screen that swaps on top of it:
/// - World → "playing" (buttons, food/gold labels, gameplay HUD)
/// - NpcDialogue → "npc_speak" (panel, portrait, dialogue text)
/// - Other modes → their respective screen .ron
fn screen_for_mode(mode: UiMode) -> &'static str {
    match mode {
        UiMode::World => "playing",
        UiMode::NpcDialogue => "npc_speak",
        UiMode::Building => "building",
        UiMode::Chest => "chest",
        UiMode::Inventory => "inventory",
        UiMode::Stats => "stats",
        UiMode::Rest => "rest",
        UiMode::Map => "map",
        UiMode::TurnBattle => "turnbattle",
    }
}

/// Tracks which overlay screen is currently shown.
/// Starts empty — first sync will show "playing".
#[derive(Resource, Default)]
pub struct ActiveOverlay(Option<String>);

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
    building_screen: Option<Res<super::BuildingScreen>>,
) {
    let desired: String = if ui.mode == UiMode::Building {
        building_screen
            .as_ref()
            .map(|bs| bs.0.clone())
            .unwrap_or_else(|| "building".to_string())
    } else {
        screen_for_mode(ui.mode).to_string()
    };

    if overlay.0.as_deref() == Some(desired.as_str()) {
        return;
    }

    // Hide previous overlay.
    if let Some(old_screen) = &overlay.0 {
        info!("overlay: hiding '{}'", old_screen);
        actions.try_write(ScreenActions {
            actions: vec![format!("HideScreen(\"{}\")", old_screen)],
        });
    }

    // Show new overlay.
    info!("overlay: showing '{}'", desired);
    actions.try_write(ScreenActions {
        actions: vec![format!("ShowScreen(\"{}\")", desired)],
    });

    overlay.0 = Some(desired);
}
