//! Screen runtime: renders .ron screen definitions as composable Bevy UI layers.
//!
//! Multiple screens can be visible simultaneously (e.g. HUD + building UI).
//! - `LoadScreen("x")` — replaces ALL screens with a single new one
//! - `ShowScreen("x")` — adds a screen layer on top of existing ones
//! - `HideScreen("x")` — removes a specific screen layer

use std::collections::HashMap;

use bevy::ecs::message::Message;
use bevy::prelude::*;
use openmm_data::assets::SmkDecoder;

use super::Screen;
use crate::GameState;

pub struct ScreenRuntimePlugin;

impl Plugin for ScreenRuntimePlugin {
    fn build(&self, app: &mut App) {
        use super::elements::{text_update, update_screen_crosshair};
        use super::interaction::{
            click_flash_tick, hover_actions, process_pending_actions, pulse_animate, pulse_hover, screen_click,
            screen_hover, screen_keys, text_hover,
        };
        use super::setup::{game_screen_setup, loading_screen_setup, menu_screen_setup, screen_teardown};
        use super::video::video_tick;

        let screen_states = in_state(GameState::Menu)
            .or(in_state(GameState::Game))
            .or(in_state(GameState::Loading));

        app.add_plugins(super::bindings::BindingsPlugin)
            .add_message::<ScreenActions>()
            .init_resource::<ScreenLayers>()
            .init_resource::<ScreenUiHovered>()
            // Menu state: load "menu" screen.
            .add_systems(OnEnter(GameState::Menu), menu_screen_setup)
            .add_systems(OnExit(GameState::Menu), screen_teardown)
            // Loading state: load "loading" screen.
            .add_systems(OnEnter(GameState::Loading), loading_screen_setup)
            .add_systems(OnExit(GameState::Loading), screen_teardown)
            // Game state: load "ingame" screen as HUD overlay (no extra camera).
            .add_systems(OnEnter(GameState::Game), game_screen_setup)
            .add_systems(OnExit(GameState::Game), screen_teardown)
            // Interaction and text systems run in Menu, Loading, and Game states.
            .add_systems(
                Update,
                (
                    screen_hover,
                    hover_actions,
                    pulse_hover,
                    pulse_animate,
                    screen_click,
                    screen_keys,
                    video_tick,
                    text_update,
                    click_flash_tick,
                    process_pending_actions,
                    update_screen_crosshair,
                    text_hover,
                )
                    .run_if(screen_states),
            );
    }
}
// ── Components & resources ──────────────────────────────────────────────────

/// Marks the screen-system crosshair entity.
#[derive(Component)]
pub(super) struct ScreenCrosshair;

/// Tags an entity as belonging to a specific screen layer.
#[derive(Component, Clone)]
pub(crate) struct ScreenLayer(pub(crate) String);

/// Maps a Bevy entity to a screen element index within its layer.
#[derive(Component)]
pub(crate) struct RuntimeElement {
    pub(crate) screen_id: String,
    pub(crate) index: usize,
    /// The RON element `id` field — for ShowSprite/HideSprite lookup.
    pub(crate) element_id: String,
}

#[derive(Component)]
pub(super) struct HoverOverlay;

#[derive(Component)]
pub(super) struct ScreenMusic(pub(super) String);

#[derive(Component)]
pub(crate) struct ClickFlash {
    pub(crate) timer: Timer,
    pub(crate) pending_actions: Vec<String>,
}

/// Pre-loaded "clicked" state texture + the default texture handle for restoration.
#[derive(Component)]
pub(crate) struct ClickedTexture {
    pub(crate) clicked: Handle<Image>,
    /// None when element has no default texture (restore to hidden).
    pub(crate) default: Option<Handle<Image>>,
}

/// Frame animation — cycles through numbered textures at a given FPS.
#[derive(Component)]
pub(crate) struct FrameAnimation {
    pub(crate) handles: Vec<Handle<Image>>,
    pub(crate) fps: f32,
    pub(crate) elapsed: f32,
    pub(crate) current_frame: usize,
}

/// Marks a text element with its data source binding.
#[derive(Component)]
pub(super) struct RuntimeText {
    pub(super) source: String,
    pub(super) text: String,
    pub(super) font: String,
    pub(super) font_size: f32,
    pub(super) color: [u8; 4],
    pub(super) base_color: [u8; 4],
    pub(super) hover_color: Option<[u8; 4]>,
    pub(super) align: String,
    /// Bounding box in reference pixels: (x, y, w, h).
    pub(super) bounds: (f32, f32, f32, f32),
    /// Last rendered text — skip re-render if unchanged.
    pub(super) last_text: String,
    /// Last rendered color — skip re-render if unchanged.
    pub(super) last_color: [u8; 4],
}

/// Element starts hidden (from `hidden: true` in RON). Restored to Hidden on unhover.
#[derive(Component)]
pub(super) struct HiddenByDefault;

/// Runtime state for an inline SMK video.
#[derive(Component)]
pub(super) struct InlineVideo {
    pub(super) decoder: SmkDecoder,
    pub(super) image_handle: Handle<Image>,
    pub(super) frame_timer: f32,
    pub(super) spf: f32,
    pub(super) looping: bool,
    pub(super) skippable: bool,
    pub(super) on_end: Vec<String>,
    pub(super) smk_bytes: Vec<u8>,
    pub(super) finished: bool,
    pub(super) life_timer: f32,
}

/// Element has PulseSprite() in on_hover — eligible for pulse animation.
#[derive(Component)]
pub(super) struct Pulsable;

/// Currently pulsing (hover active). Accumulates time for sine wave.
#[derive(Component)]
pub(super) struct Pulsing {
    pub(super) elapsed: f32,
}

/// All active screen layers, keyed by screen id.
#[derive(Resource, Default)]
pub(super) struct ScreenLayers {
    pub(super) screens: HashMap<String, Screen>,
}

/// Queued actions from click handlers, processed next frame.
#[derive(Message, Clone)]
pub(crate) struct ScreenActions {
    pub(crate) actions: Vec<String>,
}

/// Tracks whether a screen UI element is currently hovered.
/// When true, the world interaction system skips footer clearing.
#[derive(Resource, Default)]
pub struct ScreenUiHovered(pub bool);
