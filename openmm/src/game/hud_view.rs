use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use crate::game::debug::console::ConsoleState;

/// Run condition: game input is fully active — HudView is World and no console open.
/// Use this on all player/world input systems instead of manual per-system checks.
pub fn game_input_active(view: Res<HudView>, console: Option<Res<ConsoleState>>) -> bool {
    matches!(*view, HudView::World) && !console.as_ref().is_some_and(|c| c.open)
}

/// Set cursor grab mode and visibility. grab=true = locked/hidden (gameplay), false = free (UI).
pub fn grab_cursor(cursor_query: &mut Query<&mut CursorOptions, With<PrimaryWindow>>, grab: bool) {
    if let Ok(mut cursor) = cursor_query.single_mut() {
        if grab {
            cursor.grab_mode = CursorGrabMode::Confined;
            cursor.visible = false;
        } else {
            cursor.grab_mode = CursorGrabMode::None;
            cursor.visible = true;
        }
    }
}

/// Which view the HUD is currently displaying.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HudView {
    #[default]
    World,
    /// Entered a building interior (shop, temple, tavern, etc.).
    Building,
    /// Talking to a street NPC.
    NpcDialogue,
    Chest,
    Inventory,
    Stats,
    Rest,
    /// Fullscreen map overlay (M key). Freezes time, blocks input.
    Map,
}

/// Resource holding the image to display as a fullscreen overlay in the viewport area.
/// Insert this resource (and set HudView to a non-World variant) to show an overlay.
#[derive(Resource)]
pub struct OverlayImage {
    pub image: Handle<Image>,
}

/// Handle to the current map's overview image for the M-key fullscreen overlay.
/// `None` for indoor maps (no overview icon exists).
#[derive(Resource)]
pub struct MapOverviewImage(pub Option<Handle<Image>>);
