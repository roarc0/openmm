use std::sync::Arc;

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use crate::GameState;
use crate::game::blv::{ClickableFaces, OccluderFaces};
use crate::game::entities::sprites::{AlphaMask, SpriteSheet};
use crate::game::event_dispatch::EventQueue;
use crate::game::events::{GENERATED_NPC_ID_BASE, MapEvents};
use crate::game::hud::{FooterText, HudView, OverlayImage};
use crate::game::player::{Player, PlayerCamera};
use crate::game::raycast::{billboard_hit_test, point_in_polygon, ray_plane_intersect, resolve_event_name};
use crate::game::world_state::WorldState;

// --- Components & Resources ---

/// Component on billboard/decoration entities that have EVT events.
#[derive(Component)]
pub struct DecorationInfo {
    pub event_id: u16,
    pub position: Vec3,
    /// Index into the map's billboard array (for SetSprite targeting).
    pub billboard_index: usize,
    /// Declist ID used to resolve sprite name and scale via BillboardManager::get().
    pub declist_id: u16,
    /// Ground Y in Bevy coords (transform.y at spawn minus original half_h).
    /// Stable across sprite swaps — used by SetSprite to reposition the billboard.
    pub ground_y: f32,
    /// World-space half-extents for static (non-SpriteSheet) decorations. Zero for directional.
    pub half_w: f32,
    pub half_h: f32,
    /// Alpha mask for pixel-accurate hit testing of static decorations. None for directional.
    pub mask: Option<Arc<AlphaMask>>,
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
                .run_if(in_state(GameState::Game))
                .run_if(crate::game::hud::game_input_active),
        )
        .add_systems(
            Update,
            decoration_proximity_system
                .run_if(in_state(GameState::Game))
                .run_if(resource_equals(HudView::World)),
        )
        .add_systems(
            Update,
            interaction_input
                .run_if(in_state(GameState::Game))
                .run_if(|view: Res<HudView>| matches!(*view, HudView::Building | HudView::NpcDialogue | HudView::Chest))
                .after(crate::game::player::PlayerInputSet),
        );
    }
}

// --- Helpers ---

fn check_interact_input(
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

// --- Helpers ---

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
    mut view: ResMut<HudView>,
    mut commands: Commands,
    mut cursor_query: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mut event_queue: ResMut<EventQueue>,
) {
    if check_exit_input(&keys, &gamepads) {
        event_queue.clear();
        commands.remove_resource::<OverlayImage>();
        commands.remove_resource::<crate::game::hud::NpcPortrait>();
        commands.remove_resource::<crate::game::hud::NpcProfile>();
        *view = HudView::World;
        if let Ok(mut cursor) = cursor_query.single_mut() {
            cursor.grab_mode = CursorGrabMode::Confined;
            cursor.visible = false;
        }
    }
}

/// Detect click/interact on the nearest interactable in the world (decoration, NPC, or BSP face)
/// and push exactly one event. By finding the global nearest hit before pushing, this guarantees
/// only one UI can open per interaction — no stacking of events from overlapping targets.
fn world_interact_system(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    gamepads: Query<&Gamepad>,
    camera_query: Query<(&GlobalTransform, &Camera), With<PlayerCamera>>,
    decorations: Query<(&DecorationInfo, &GlobalTransform, Option<&SpriteSheet>)>,
    npcs: Query<(&NpcInteractable, &GlobalTransform, &SpriteSheet)>,
    clickable_faces: Option<Res<ClickableFaces>>,
    occluder_faces: Option<Res<OccluderFaces>>,
    map_events: Option<Res<MapEvents>>,
    mut event_queue: ResMut<EventQueue>,
    cursor_query: Query<&CursorOptions, With<PrimaryWindow>>,
    world_state: Option<Res<WorldState>>,
) {
    let Ok((cam_global, _)) = camera_query.single() else {
        return;
    };
    let (key, click, gamepad) = check_interact_input(&keys, &mouse, &gamepads);
    if !key && !click && !gamepad {
        return;
    }
    let cursor_grabbed = cursor_query
        .single()
        .map(|c| !matches!(c.grab_mode, CursorGrabMode::None))
        .unwrap_or(true);
    if click && !cursor_grabbed {
        return;
    }

    let origin = cam_global.translation();
    let dir = cam_global.forward().as_vec3();
    let occluder_t = occluder_faces
        .as_ref()
        .map(|of| of.min_hit_t(origin, dir))
        .unwrap_or(f32::MAX);

    // Find the single nearest hit across all interactable types.
    enum Hit {
        Face(u16),
        /// Carries (event_id, billboard_index) so ChangeEvent overrides can be checked.
        Decoration(u16, usize),
        Npc(i16),
    }
    let mut nearest: Option<(f32, Hit)> = None;

    // BSP faces (buildings, doors) — not occluded by terrain walls.
    if let Some(faces) = clickable_faces.as_ref() {
        for face in &faces.faces {
            if let Some(t) = ray_plane_intersect(origin, dir, face.normal, face.plane_dist) {
                if t > crate::game::blv::INDOOR_INTERACT_RANGE {
                    continue;
                }
                let hit = origin + dir * t;
                if point_in_polygon(hit, &face.vertices, face.normal) && nearest.as_ref().is_none_or(|n| t < n.0) {
                    nearest = Some((t, Hit::Face(face.event_id)));
                }
            }
        }
    }

    for (info, g_tf, sheet_opt) in decorations.iter() {
        let (half_w, half_h, mask) = if let Some(sheet) = sheet_opt {
            let Some(&(sw, sh)) = sheet.state_dimensions.get(sheet.current_state) else {
                continue;
            };
            (sw / 2.0, sh / 2.0, sheet.current_mask.as_deref())
        } else {
            if info.half_w == 0.0 && info.half_h == 0.0 {
                continue;
            }
            (info.half_w, info.half_h, info.mask.as_deref())
        };
        if let Some(t) = billboard_hit_test(
            origin,
            dir,
            g_tf.translation(),
            facing_rotation(origin, g_tf.translation()),
            half_w,
            half_h,
            mask,
        ) && t < occluder_t
            && nearest.as_ref().is_none_or(|n| t < n.0)
        {
            nearest = Some((t, Hit::Decoration(info.event_id, info.billboard_index)));
        }
    }

    for (info, g_tf, sheet) in npcs.iter() {
        let Some(&(sw, sh)) = sheet.state_dimensions.get(sheet.current_state) else {
            continue;
        };
        if let Some(t) = billboard_hit_test(
            origin,
            dir,
            g_tf.translation(),
            facing_rotation(origin, g_tf.translation()),
            sw / 2.0,
            sh / 2.0,
            sheet.current_mask.as_deref(),
        ) && t < occluder_t
            && nearest.as_ref().is_none_or(|n| t < n.0)
        {
            nearest = Some((t, Hit::Npc(info.npc_id)));
        }
    }

    match nearest {
        Some((dist, Hit::Face(event_id))) => {
            info!("World interact: hit BSP face event_id={} at dist={:.0}", event_id, dist);
            if let Some(me) = map_events.as_ref()
                && let Some(evt) = me.evt.as_ref()
            {
                event_queue.push_all(event_id, evt);
            }
        }
        Some((_, Hit::Decoration(event_id, billboard_idx))) => {
            // ChangeEvent can redirect this decoration to a different script at runtime.
            let effective_id = world_state
                .as_ref()
                .and_then(|ws| ws.game_vars.event_overrides.get(&billboard_idx))
                .copied()
                .unwrap_or(event_id);
            if let Some(me) = map_events.as_ref()
                && let Some(evt) = me.evt.as_ref()
            {
                event_queue.push_all(effective_id, evt);
            }
        }
        Some((_, Hit::Npc(npc_id))) => {
            let npc_id_i32 = npc_id as i32;
            // For quest NPCs (from npcdata.txt), run their event_a script if available.
            // event_a is the "speak to" script — it typically contains SpeakNPC + dialogue options.
            let ran_event = if npc_id_i32 > 0 && npc_id_i32 < GENERATED_NPC_ID_BASE {
                if let Some(me) = map_events.as_ref()
                    && let Some(evt) = me.evt.as_ref()
                    && let Some(entry) = me.npc_table.as_ref().and_then(|t| t.get(npc_id_i32))
                    && entry.event_a > 0
                {
                    event_queue.push_all(entry.event_a as u16, evt);
                    true
                } else {
                    false
                }
            } else {
                false
            };
            if !ran_event {
                event_queue.push_single(lod::evt::GameEvent::SpeakNPC { npc_id: npc_id_i32 });
            }
        }
        None => {}
    }
}

/// Fire EVT events when the player enters a decoration's trigger radius.
/// Only fires on the rising edge (entering range), not while staying in range.
fn decoration_proximity_system(
    player_query: Query<&Transform, With<Player>>,
    mut triggers: Query<(&GlobalTransform, &mut DecorationTrigger)>,
    map_events: Option<Res<MapEvents>>,
    mut event_queue: ResMut<EventQueue>,
) {
    let Ok(player_tf) = player_query.single() else {
        return;
    };
    let player_pos = player_tf.translation;

    for (g_tf, mut trigger) in triggers.iter_mut() {
        let dist_sq = g_tf.translation().distance_squared(player_pos);
        let radius_sq = trigger.trigger_radius * trigger.trigger_radius;
        let in_range = dist_sq <= radius_sq;
        if in_range && !trigger.was_in_range {
            // Rising edge: player just entered the trigger radius
            if let Some(me) = map_events.as_ref()
                && let Some(evt) = me.evt.as_ref()
                && trigger.event_id > 0
            {
                event_queue.push_all(trigger.event_id, evt);
            }
        }
        trigger.was_in_range = in_range;
    }
}

/// Show the nearest interactive object's name in the footer — pixel-accurate for all types.
fn hover_hint_system(
    camera_query: Query<(&GlobalTransform, &Camera), With<PlayerCamera>>,
    clickable_faces: Option<Res<ClickableFaces>>,
    occluder_faces: Option<Res<OccluderFaces>>,
    decorations: Query<(&DecorationInfo, &GlobalTransform, Option<&SpriteSheet>)>,
    npcs: Query<(&NpcInteractable, &GlobalTransform, &SpriteSheet)>,
    monsters: Query<(&MonsterInteractable, &GlobalTransform, &SpriteSheet)>,
    map_events: Option<Res<MapEvents>>,
    mut footer: ResMut<FooterText>,
) {
    let Ok((cam_global, _)) = camera_query.single() else {
        return;
    };
    let origin = cam_global.translation();
    let dir = cam_global.forward().as_vec3();
    let occluder_t = occluder_faces
        .as_ref()
        .map(|of| of.min_hit_t(origin, dir))
        .unwrap_or(f32::MAX);

    let mut nearest: Option<(f32, String)> = None;

    // BSP faces (outdoor and indoor) via ClickableFaces — these are part of buildings so
    // they are always in front of the occluder boundary; no occlusion check needed here.
    if let Some(faces) = clickable_faces.as_ref() {
        for face in &faces.faces {
            if let Some(t) = ray_plane_intersect(origin, dir, face.normal, face.plane_dist) {
                if t > crate::game::blv::INDOOR_INTERACT_RANGE {
                    continue;
                }
                let hit = origin + dir * t;
                if point_in_polygon(hit, &face.vertices, face.normal)
                    && let Some(name) = resolve_event_name(face.event_id, &map_events)
                    && (nearest.is_none() || t < nearest.as_ref().unwrap().0)
                {
                    nearest = Some((t, name));
                }
            }
        }
    }

    // Decorations — skip if behind a solid wall.
    for (info, g_tf, sheet_opt) in decorations.iter() {
        let (half_w, half_h, mask) = if let Some(sheet) = sheet_opt {
            let Some(&(sw, sh)) = sheet.state_dimensions.get(sheet.current_state) else {
                continue;
            };
            (sw / 2.0, sh / 2.0, sheet.current_mask.as_deref())
        } else {
            if info.half_w == 0.0 && info.half_h == 0.0 {
                continue;
            }
            (info.half_w, info.half_h, info.mask.as_deref())
        };
        if let Some(t) = billboard_hit_test(
            origin,
            dir,
            g_tf.translation(),
            facing_rotation(origin, g_tf.translation()),
            half_w,
            half_h,
            mask,
        ) && t < occluder_t
            && (nearest.is_none() || t < nearest.as_ref().unwrap().0)
            && let Some(name) = resolve_event_name(info.event_id, &map_events)
        {
            nearest = Some((t, name));
        }
    }

    // NPCs — skip if behind a solid wall.
    for (info, g_tf, sheet) in npcs.iter() {
        let Some(&(sw, sh)) = sheet.state_dimensions.get(sheet.current_state) else {
            continue;
        };
        if let Some(t) = billboard_hit_test(
            origin,
            dir,
            g_tf.translation(),
            facing_rotation(origin, g_tf.translation()),
            sw / 2.0,
            sh / 2.0,
            sheet.current_mask.as_deref(),
        ) && t < occluder_t
            && (nearest.is_none() || t < nearest.as_ref().unwrap().0)
        {
            nearest = Some((t, info.name.clone()));
        }
    }

    // Monsters — skip if behind a solid wall.
    for (info, g_tf, sheet) in monsters.iter() {
        let Some(&(sw, sh)) = sheet.state_dimensions.get(sheet.current_state) else {
            continue;
        };
        if let Some(t) = billboard_hit_test(
            origin,
            dir,
            g_tf.translation(),
            facing_rotation(origin, g_tf.translation()),
            sw / 2.0,
            sh / 2.0,
            sheet.current_mask.as_deref(),
        ) && t < occluder_t
            && (nearest.is_none() || t < nearest.as_ref().unwrap().0)
        {
            nearest = Some((t, info.name.clone()));
        }
    }

    match nearest {
        Some((_, name)) => footer.set(&name),
        None => footer.clear(),
    }
}
