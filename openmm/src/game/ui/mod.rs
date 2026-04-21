use crate::screens::PropertySource;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use crate::screens::debug::console::ConsoleState;
use crate::screens::runtime::ScreenLayers;

pub mod npc_dialogue;
pub mod overlay;
pub mod party_creation;

/// Run condition: UiMode is World and no Modal screen is active.
/// Use for systems that should pause during any overlay.
pub fn is_world_mode(ui: Res<UiState>, layers: Option<Res<ScreenLayers>>) -> bool {
    ui.mode == UiMode::World && !layers.is_some_and(|l| l.has_modal())
}

/// Run condition: game input is fully active — UiState mode is World, no modal screen
/// open, and no console open. Use this on all player/world input systems.
pub fn game_input_active(
    ui: Res<UiState>,
    console: Option<Res<ConsoleState>>,
    layers: Option<Res<ScreenLayers>>,
) -> bool {
    matches!(ui.mode, UiMode::World)
        && !layers.is_some_and(|l| l.has_modal())
        && !console.as_ref().is_some_and(|c| c.open)
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

/// Transition to a UI mode that displays a fullscreen overlay image.
/// Sets the mode, inserts the OverlayImage resource, and frees the cursor.
pub fn set_overlay_mode(
    commands: &mut Commands,
    ui: &mut UiState,
    cursor_query: &mut Query<&mut CursorOptions, With<PrimaryWindow>>,
    image: Handle<Image>,
    mode: UiMode,
) {
    commands.insert_resource(OverlayImage { image });
    set_ui_mode(ui, cursor_query, mode);
}

/// Switch to a non-World UI mode and free the cursor.
pub fn set_ui_mode(ui: &mut UiState, cursor_query: &mut Query<&mut CursorOptions, With<PrimaryWindow>>, mode: UiMode) {
    ui.mode = mode;
    grab_cursor(cursor_query, mode == UiMode::World);
}

/// Which view the HUD is currently displaying.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UiMode {
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
    /// Turn-based combat mode. Freezes movement, shows combat UI.
    TurnBattle,
}

/// Unified UI state resource.
#[derive(Resource, Default)]
pub struct UiState {
    pub mode: UiMode,
    pub footer: FooterText,
}

/// Resource holding the image to display as a fullscreen overlay in the viewport area.
/// Insert this resource (and set UiState mode to a non-World variant) to show an overlay.
#[derive(Resource)]
pub struct OverlayImage {
    pub image: Handle<Image>,
}

/// Which building screen to show when UiMode is Building.
/// Set by handle_speak_in_house based on the building_type from 2devents.txt.
#[derive(Resource, Default)]
pub struct BuildingScreen(pub String);

/// Active building's display metadata for screen element bindings.
/// Inserted by handle_speak_in_house, removed by interaction_input on exit.
#[derive(Resource, Default)]
pub struct HouseProfile {
    pub name: String,
    pub owner_name: String,
}

/// Handle to the current map's overview image for the M-key fullscreen overlay.
/// `None` for indoor maps (no overview icon exists).
#[derive(Resource)]
pub struct MapOverviewImage(pub Option<Handle<Image>>);

/// Text displayed in the footer bar. Write to this resource via `UiState.footer`
/// to update the footer message (e.g. building names, hints, status text).
#[derive(Default)]
pub struct FooterText {
    text: String,
    color: String,
    /// Generation counter -- bumped on every change so consumers know to re-render.
    pub(crate) generation: u64,
    /// If set, this text is "locked" until the timer expires.
    /// Hover hints won't overwrite locked text.
    lock_until: Option<f64>,
}

impl FooterText {
    /// Set footer text. This is a "soft" set — won't overwrite locked (status) text.
    pub fn set(&mut self, text: &str) {
        self.set_colored(text, "white");
    }

    /// Set footer text with a specific color.
    pub fn set_colored(&mut self, text: &str, color: &str) {
        if self.lock_until.is_some() {
            return;
        }
        if self.text != text || self.color != color {
            self.text = text.to_string();
            self.color = color.to_string();
            self.generation += 1;
        }
    }

    /// Set footer text that persists for `duration` seconds.
    /// Cannot be overwritten by hover hints until it expires.
    pub fn set_status(&mut self, text: &str, duration: f64, now: f64) {
        self.set_status_colored(text, "white", duration, now);
    }

    /// Set colored footer text that persists for `duration` seconds.
    pub fn set_status_colored(&mut self, text: &str, color: &str, duration: f64, now: f64) {
        self.text = text.to_string();
        self.color = color.to_string();
        self.lock_until = Some(now + duration);
        self.generation += 1;
    }

    /// Call every frame to expire locked text.
    pub fn tick(&mut self, now: f64) {
        if let Some(until) = self.lock_until
            && now >= until
        {
            self.lock_until = None;
            self.text.clear();
            self.color = "white".to_string();
            self.generation += 1;
        }
    }

    /// Clear the footer text.
    pub fn clear(&mut self) {
        self.set("");
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn color(&self) -> &str {
        &self.color
    }
}

/// System to tick the footer text expiration.
pub fn tick_footer_text(mut ui: ResMut<UiState>, time: Res<Time>) {
    ui.footer.tick(time.elapsed_secs_f64());
}

impl PropertySource for UiState {
    fn source_name(&self) -> &str {
        "ui"
    }

    fn resolve(&self, path: &str) -> Option<String> {
        match path {
            "" | "footer" => Some(self.footer.text().to_string()),
            "footer_color" => Some(self.footer.color().to_string()),
            _ => None,
        }
    }
}

impl PropertySource for HouseProfile {
    fn source_name(&self) -> &str {
        "house"
    }

    fn resolve(&self, path: &str) -> Option<String> {
        match path {
            "" | "name" => Some(self.name.clone()),
            "owner" => Some(self.owner_name.clone()),
            _ => None,
        }
    }
}
