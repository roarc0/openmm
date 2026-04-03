use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use crate::GameState;
use crate::game::event_dispatch::EventQueue;
use crate::game::events::MapEvents;
use crate::game::hud::{FooterText, HudView, OverlayImage};
use crate::game::player::PlayerCamera;
use crate::game::raycast::{billboard_hit_test, resolve_event_name, ray_plane_intersect, point_in_polygon};
use crate::game::blv::ClickableFaces;
use crate::game::entities::sprites::SpriteSheet;

// --- Components & Resources ---

/// Component on BSP model parent entities that are interactive.
#[derive(Component)]
pub struct BuildingInfo {
    pub model_name: String,
    pub position: Vec3,
    pub event_ids: Vec<u16>,
}

/// Component on billboard/decoration entities that have EVT events.
#[derive(Component)]
pub struct DecorationInfo {
    pub event_id: u16,
    pub position: Vec3,
    /// Index into the map's billboard array (for SetSprite targeting).
    pub billboard_index: usize,
}

/// Component on NPC actor entities for hover/click interaction.
#[derive(Component)]
pub struct NpcInteractable {
    pub name: String,
    pub position: Vec3,
    /// Index into the street NPC table (Game.StreetNPC + 1). Zero means no NPC dialogue.
    pub npc_id: i16,
}

/// Component on monster entities for hover name display.
/// No click action yet — combat system not implemented.
#[derive(Component)]
pub struct MonsterInteractable {
    pub name: String,
}

const RAYCAST_RANGE: f32 = 2000.0;
/// Tangent of the targeting cone half-angle used for all entity types (~7 degrees).
/// At 500 units: ~60 unit radius. At 2000 units: ~240 unit radius.
const RAY_ANGLE_TAN: f32 = 0.12;
/// Minimum perpendicular threshold for very close objects.
const RAY_MIN_PERP: f32 = 60.0;

// --- Plugin ---

pub struct InteractionPlugin;

impl Plugin for InteractionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                hover_hint_system,
                decoration_interact_system,
                npc_interact_system,
            )
                .chain()
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

pub fn make_building_info(model_name: &str, position: Vec3, event_ids: Vec<u16>) -> BuildingInfo {
    BuildingInfo {
        model_name: model_name.to_string(),
        position,
        event_ids,
    }
}

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

/// Find the nearest entity position within the camera ray's angular cone.
/// Returns `(entity_ref, along_ray_distance)` for the closest hit.
fn raycast_nearest<T>(cam_global: &GlobalTransform, items: impl Iterator<Item = (T, Vec3)>) -> Option<(T, f32)> {
    let ray_origin = cam_global.translation();
    let ray_dir = cam_global.forward().as_vec3();
    let mut nearest: Option<(T, f32)> = None;

    for (item, position) in items {
        let to_item = position - ray_origin;
        let along_ray = to_item.dot(ray_dir);
        if !(0.0..=RAYCAST_RANGE).contains(&along_ray) {
            continue;
        }
        let closest_point = ray_origin + ray_dir * along_ray;
        let perp_dist = closest_point.distance(position);
        let threshold = (along_ray * RAY_ANGLE_TAN).max(RAY_MIN_PERP);
        if perp_dist < threshold && (nearest.is_none() || along_ray < nearest.as_ref().unwrap().1) {
            nearest = Some((item, along_ray));
        }
    }

    nearest
}

fn find_nearest_building<'a>(
    cam_global: &GlobalTransform,
    buildings: &'a Query<(&BuildingInfo, &GlobalTransform)>,
) -> Option<&'a BuildingInfo> {
    raycast_nearest(cam_global, buildings.iter().map(|(info, _)| (info, info.position))).map(|(info, _)| info)
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

// --- Systems ---

/// Handle exit input when in Building or Chest overlay views.
fn interaction_input(
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut view: ResMut<HudView>,
    mut commands: Commands,
    mut cursor_query: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if check_exit_input(&keys, &gamepads) {
        commands.remove_resource::<OverlayImage>();
        commands.remove_resource::<crate::game::hud::NpcPortrait>();
        *view = HudView::World;
        if let Ok(mut cursor) = cursor_query.single_mut() {
            cursor.grab_mode = CursorGrabMode::Confined;
            cursor.visible = false;
        }
    }
}

/// Resolve a human-readable name for a building from its event data.
fn resolve_building_name(info: &BuildingInfo, map_events: &Option<Res<MapEvents>>) -> Option<String> {
    let me = map_events.as_ref()?;
    let evt = me.evt.as_ref()?;

    for &eid in &info.event_ids {
        if let Some(steps) = evt.events.get(&eid) {
            for s in steps {
                match &s.event {
                    lod::evt::GameEvent::OpenChest { id } => {
                        return Some(format!("Chest #{}", id));
                    }
                    lod::evt::GameEvent::SpeakInHouse { house_id } => {
                        if let Some(houses) = me.houses.as_ref()
                            && let Some(entry) = houses.houses.get(house_id)
                        {
                            return Some(entry.name.clone());
                        }
                        return Some(format!("Building #{}", house_id));
                    }
                    lod::evt::GameEvent::Hint { text, .. } => {
                        if !text.is_empty() {
                            return Some(text.clone());
                        }
                    }
                    lod::evt::GameEvent::MoveToMap { map_name, .. } => {
                        return Some(format!("Enter {}", map_name));
                    }
                    lod::evt::GameEvent::ChangeDoorState { .. } => {}
                    lod::evt::GameEvent::PlaySound { .. } => {}
                    lod::evt::GameEvent::StatusText { text, .. } => {
                        if !text.is_empty() {
                            return Some(text.clone());
                        }
                    }
                    lod::evt::GameEvent::LocationName { text, .. } => {
                        if !text.is_empty() {
                            return Some(text.clone());
                        }
                    }
                    lod::evt::GameEvent::ShowMessage { .. } => {}
                    lod::evt::GameEvent::Exit => {}
                    _ => {}
                }
            }
        }
    }
    None
}

/// Detect click on a decoration and push its events. Uses billboard hit test with alpha mask.
fn decoration_interact_system(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    gamepads: Query<&Gamepad>,
    camera_query: Query<(&GlobalTransform, &Camera), With<PlayerCamera>>,
    decorations: Query<(&DecorationInfo, &GlobalTransform, &Transform, &SpriteSheet)>,
    map_events: Option<Res<MapEvents>>,
    mut event_queue: ResMut<EventQueue>,
    cursor_query: Query<&CursorOptions, With<PrimaryWindow>>,
) {
    let Ok((cam_global, _)) = camera_query.single() else { return };
    let (key, click, gamepad) = check_interact_input(&keys, &mouse, &gamepads);
    if !key && !click && !gamepad { return }
    let cursor_grabbed = cursor_query.single()
        .map(|c| !matches!(c.grab_mode, CursorGrabMode::None)).unwrap_or(true);
    if click && !cursor_grabbed { return }

    let origin = cam_global.translation();
    let dir = cam_global.forward().as_vec3();

    let mut nearest: Option<(f32, u16)> = None;
    for (info, g_tf, tf, sheet) in decorations.iter() {
        let (sw, sh) = sheet.state_dimensions[sheet.current_state];
        if let Some(t) = billboard_hit_test(
            origin, dir, g_tf.translation(), tf.rotation,
            sw / 2.0, sh / 2.0, sheet.current_mask.as_deref(),
        ) {
            if nearest.is_none() || t < nearest.unwrap().0 {
                nearest = Some((t, info.event_id));
            }
        }
    }

    let Some((_, event_id)) = nearest else { return };
    let Some(me) = map_events else { return };
    let Some(evt) = me.evt.as_ref() else { return };
    event_queue.push_all(event_id, evt);
}

/// Detect click on an NPC and push a SpeakNPC event. Uses billboard hit test with alpha mask.
fn npc_interact_system(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    gamepads: Query<&Gamepad>,
    camera_query: Query<(&GlobalTransform, &Camera), With<PlayerCamera>>,
    npcs: Query<(&NpcInteractable, &GlobalTransform, &Transform, &SpriteSheet)>,
    mut event_queue: ResMut<EventQueue>,
    cursor_query: Query<&CursorOptions, With<PrimaryWindow>>,
) {
    let Ok((cam_global, _)) = camera_query.single() else { return };
    let (key, click, gamepad) = check_interact_input(&keys, &mouse, &gamepads);
    if !key && !click && !gamepad { return }
    let cursor_grabbed = cursor_query.single()
        .map(|c| !matches!(c.grab_mode, CursorGrabMode::None)).unwrap_or(true);
    if click && !cursor_grabbed { return }

    let origin = cam_global.translation();
    let dir = cam_global.forward().as_vec3();

    let mut nearest: Option<(f32, i16)> = None;
    for (info, g_tf, tf, sheet) in npcs.iter() {
        let (sw, sh) = sheet.state_dimensions[sheet.current_state];
        if let Some(t) = billboard_hit_test(
            origin, dir, g_tf.translation(), tf.rotation,
            sw / 2.0, sh / 2.0, sheet.current_mask.as_deref(),
        ) {
            if nearest.is_none() || t < nearest.unwrap().0 {
                nearest = Some((t, info.npc_id));
            }
        }
    }

    let Some((_, npc_id)) = nearest else { return };
    event_queue.push_single(lod::evt::GameEvent::SpeakNPC { npc_id: npc_id as i32 });
}

/// Show the nearest interactive object's name in the footer — pixel-accurate for all types.
fn hover_hint_system(
    camera_query: Query<(&GlobalTransform, &Camera), With<PlayerCamera>>,
    clickable_faces: Option<Res<ClickableFaces>>,
    decorations: Query<(&DecorationInfo, &GlobalTransform, &Transform, &SpriteSheet)>,
    npcs: Query<(&NpcInteractable, &GlobalTransform, &Transform, &SpriteSheet)>,
    monsters: Query<(&MonsterInteractable, &GlobalTransform, &Transform, &SpriteSheet)>,
    map_events: Option<Res<MapEvents>>,
    mut footer: ResMut<FooterText>,
) {
    let Ok((cam_global, _)) = camera_query.single() else { return };
    let origin = cam_global.translation();
    let dir = cam_global.forward().as_vec3();

    let mut nearest: Option<(f32, String)> = None;

    // BSP faces (outdoor and indoor) via ClickableFaces
    if let Some(faces) = clickable_faces.as_ref() {
        for face in &faces.faces {
            if let Some(t) = ray_plane_intersect(origin, dir, face.normal, face.plane_dist) {
                if t > crate::game::blv::INDOOR_INTERACT_RANGE {
                    continue;
                }
                let hit = origin + dir * t;
                if point_in_polygon(hit, &face.vertices, face.normal) {
                    if let Some(name) = resolve_event_name(face.event_id, &map_events) {
                        if nearest.is_none() || t < nearest.as_ref().unwrap().0 {
                            nearest = Some((t, name));
                        }
                    }
                }
            }
        }
    }

    // Decorations
    for (info, g_tf, tf, sheet) in decorations.iter() {
        let (sw, sh) = sheet.state_dimensions[sheet.current_state];
        if let Some(t) = billboard_hit_test(
            origin, dir, g_tf.translation(), tf.rotation,
            sw / 2.0, sh / 2.0, sheet.current_mask.as_deref(),
        ) {
            if nearest.is_none() || t < nearest.as_ref().unwrap().0 {
                if let Some(name) = resolve_event_name(info.event_id, &map_events) {
                    nearest = Some((t, name));
                }
            }
        }
    }

    // NPCs
    for (info, g_tf, tf, sheet) in npcs.iter() {
        let (sw, sh) = sheet.state_dimensions[sheet.current_state];
        if let Some(t) = billboard_hit_test(
            origin, dir, g_tf.translation(), tf.rotation,
            sw / 2.0, sh / 2.0, sheet.current_mask.as_deref(),
        ) {
            if nearest.is_none() || t < nearest.as_ref().unwrap().0 {
                nearest = Some((t, info.name.clone()));
            }
        }
    }

    // Monsters
    for (info, g_tf, tf, sheet) in monsters.iter() {
        let (sw, sh) = sheet.state_dimensions[sheet.current_state];
        if let Some(t) = billboard_hit_test(
            origin, dir, g_tf.translation(), tf.rotation,
            sw / 2.0, sh / 2.0, sheet.current_mask.as_deref(),
        ) {
            if nearest.is_none() || t < nearest.as_ref().unwrap().0 {
                nearest = Some((t, info.name.clone()));
            }
        }
    }

    match nearest {
        Some((_, name)) => footer.set(&name),
        None => footer.clear(),
    }
}
