//! World interaction — decoration, NPC, and monster click/hover detection.
use std::sync::Arc;

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use crate::GameState;
use crate::game::events::EventQueue;
use crate::game::sprites::loading::AlphaMask;
use crate::game::ui::{OverlayImage, UiMode, UiState};

pub mod clickable;
mod hint;
pub mod raycast;
mod world_interact;

use hint::hover_hint_system;
use world_interact::{decoration_proximity_system, world_interact_system};

/// Max ray distance for all outdoor interaction (billboards, decorations, BSP faces).
/// One tile = 512 units — arm's reach, consistent with MM6 feel.
const MAX_INTERACT_RANGE: f32 = 512.0;

// --- Components & Resources ---

/// Component on billboard/decoration entities that have EVT events.
#[derive(Component)]
pub struct DecorationInfo {
    pub event_id: u16,
    pub position: Vec3,
    /// Index into the map's billboard array (for SetSprite targeting).
    pub billboard_index: usize,
    /// Declist ID used to resolve sprite name and scale.
    pub declist_id: u16,
    /// Ground Y in Bevy coords (transform.y at spawn minus original half_h).
    /// Stable across sprite swaps — used by SetSprite to reposition the billboard.
    pub ground_y: f32,
    /// World-space half-extents for static (non-SpriteSheet) decorations. Zero for directional.
    pub half_w: f32,
    pub half_h: f32,
    /// Alpha mask for pixel-accurate hit testing of static decorations. None for directional.
    pub mask: Option<Arc<AlphaMask>>,
    /// Human-readable name (e.g. "fountain").
    pub display_name: Option<String>,
}

impl DecorationInfo {
    /// Build from a decoration entry's common fields.
    /// `position` = sprite centre, `ground_y` = floor Y (dec_pos.y before half-height offset).
    pub fn from_entry(
        dec: &openmm_data::provider::decorations::DecorationEntry,
        position: Vec3,
        ground_y: f32,
        half_w: f32,
        half_h: f32,
        mask: Option<Arc<AlphaMask>>,
    ) -> Self {
        Self {
            event_id: dec.event_id as u16,
            position,
            billboard_index: dec.billboard_index,
            declist_id: dec.declist_id,
            ground_y,
            half_w,
            half_h,
            mask,
            display_name: dec.display_name.clone(),
        }
    }
}

/// Component on decoration entities that fire an EVT event when the player enters their radius.
/// Tracks whether the player was already in range to avoid re-firing every frame.
#[derive(Component)]
pub struct DecorationTrigger {
    pub event_id: u16,
    pub trigger_radius: f32,
    was_in_range: bool,
}

impl DecorationTrigger {
    pub fn new(event_id: u16, trigger_radius: f32) -> Self {
        Self {
            event_id,
            trigger_radius,
            was_in_range: false,
        }
    }
}

/// Component on NPC actor entities for hover/click interaction.
#[derive(Component)]
pub struct NpcInteractable {
    pub name: String,
    /// Quest NPC: index into npcdata.txt (1-based). Generated street NPC: GENERATED_NPC_ID_BASE + spawn index. Zero means no dialogue.
    pub npc_id: i16,
}

/// Component on monster entities for hover name display.
/// No click action yet — combat system not implemented.
#[derive(Component)]
pub struct MonsterInteractable {
    pub name: String,
}

// --- Plugin ---

pub struct InteractionPlugin;

impl Plugin for InteractionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (hover_hint_system, world_interact_system)
                .chain()
                .after(crate::game::map::spatial_index::SpatialIndexSet)
                .run_if(in_state(GameState::Game))
                .run_if(crate::game::ui::game_input_active),
        )
        .add_systems(
            Update,
            decoration_proximity_system
                .run_if(in_state(GameState::Game))
                .run_if(crate::game::ui::is_world_mode),
        )
        .add_systems(
            Update,
            interaction_input
                .run_if(in_state(GameState::Game))
                .run_if(|ui: Res<UiState>| matches!(ui.mode, UiMode::Building | UiMode::NpcDialogue | UiMode::Chest))
                .after(crate::game::player::PlayerInputSet),
        );
    }
}

// --- Helpers ---

pub(crate) fn check_interact_input(
    keys: &ButtonInput<KeyCode>,
    mouse: &ButtonInput<MouseButton>,
    gamepads: &Query<&Gamepad>,
) -> (bool, bool, bool) {
    let key = keys.just_pressed(KeyCode::KeyE) || keys.just_pressed(KeyCode::Enter);
    let click = mouse.just_pressed(MouseButton::Left);
    let gamepad = gamepads
        .iter()
        .any(|gp| gp.just_pressed(bevy::input::gamepad::GamepadButton::East));
    (key, click, gamepad)
}

fn check_exit_input(keys: &ButtonInput<KeyCode>, gamepads: &Query<&Gamepad>) -> bool {
    keys.just_pressed(KeyCode::Escape)
        || keys.just_pressed(KeyCode::KeyE)
        || keys.just_pressed(KeyCode::Enter)
        || gamepads.iter().any(|gp| {
            gp.just_pressed(bevy::input::gamepad::GamepadButton::East)
                || gp.just_pressed(bevy::input::gamepad::GamepadButton::South)
        })
}

/// Compute the Y-axis rotation that makes a billboard at `center` face `cam_origin`.
/// Matches the logic in `billboard_face_camera` for non-SpriteSheet entities.
fn facing_rotation(cam_origin: Vec3, center: Vec3) -> Quat {
    let d = cam_origin - center;
    if d.x.abs() > 0.01 || d.z.abs() > 0.01 {
        Quat::from_rotation_y(d.x.atan2(d.z))
    } else {
        Quat::IDENTITY
    }
}

// --- Systems ---

/// Handle exit input when an overlay UI is active.
/// Clears the EventQueue to discard any events that were queued alongside the now-dismissed UI.
fn interaction_input(
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut view: ResMut<UiState>,
    mut commands: Commands,
    mut cursor_query: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mut event_queue: ResMut<EventQueue>,
) {
    if check_exit_input(&keys, &gamepads) {
        event_queue.clear();
        commands.remove_resource::<OverlayImage>();
        commands.remove_resource::<crate::game::actors::npc_dialogue::NpcPortrait>();
        commands.remove_resource::<crate::game::actors::npc_dialogue::NpcProfile>();
        view.mode = UiMode::World;
        if let Ok(mut cursor) = cursor_query.single_mut() {
            cursor.grab_mode = CursorGrabMode::Confined;
            cursor.visible = false;
        }
    }
}
