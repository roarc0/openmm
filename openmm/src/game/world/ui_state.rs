use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use crate::game::debug::console::ConsoleState;

/// Run condition: game input is fully active — UiState mode is World and no console open.
/// Use this on all player/world input systems instead of manual per-system checks.
pub fn game_input_active(ui: Res<UiState>, console: Option<Res<ConsoleState>>) -> bool {
    matches!(ui.mode, UiMode::World) && !console.as_ref().is_some_and(|c| c.open)
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

/// Handle to the current map's overview image for the M-key fullscreen overlay.
/// `None` for indoor maps (no overview icon exists).
#[derive(Resource)]
pub struct MapOverviewImage(pub Option<Handle<Image>>);

/// Text displayed in the footer bar. Write to this resource via `UiState.footer`
/// to update the footer message (e.g. building names, hints, status text).
#[derive(Default)]
pub struct FooterText {
    text: String,
    /// Generation counter -- bumped on every change so consumers know to re-render.
    pub(crate) generation: u64,
    /// If set, this text is "locked" until the timer expires.
    /// Hover hints won't overwrite locked text.
    lock_until: Option<f64>,
}

impl FooterText {
    /// Set footer text. This is a "soft" set — won't overwrite locked (status) text.
    pub fn set(&mut self, text: &str) {
        if self.lock_until.is_some() {
            return;
        }
        if self.text != text {
            self.text = text.to_string();
            self.generation += 1;
        }
    }

    /// Set footer text that persists for `duration` seconds.
    /// Cannot be overwritten by hover hints until it expires.
    pub fn set_status(&mut self, text: &str, duration: f64, now: f64) {
        self.text = text.to_string();
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
}

/// System to tick the footer text expiration.
pub fn tick_footer_text(mut ui: ResMut<UiState>, time: Res<Time>) {
    ui.footer.tick(time.elapsed_secs_f64());
}
