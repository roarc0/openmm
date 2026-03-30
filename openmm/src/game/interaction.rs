use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use crate::GameState;
use crate::game::event_dispatch::EventQueue;
use crate::game::events::MapEvents;
use crate::game::hud::{FooterText, HudView, OverlayImage};
use crate::game::player::{Player, PlayerCamera};

// --- Components & Resources ---

/// Component on BSP model parent entities that are interactive.
#[derive(Component)]
pub struct BuildingInfo {
    pub model_name: String,
    pub position: Vec3,
    pub event_ids: Vec<u16>,
}


const INTERACT_RANGE: f32 = 250.0;
const RAYCAST_RANGE: f32 = 2000.0;

// --- Plugin ---

pub struct InteractionPlugin;

impl Plugin for InteractionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (hover_hint_system, interact_system)
                .chain()
                .run_if(in_state(GameState::Game))
                .run_if(resource_equals(HudView::World)),
        )
        .add_systems(
            Update,
            interaction_input
                .run_if(in_state(GameState::Game))
                .run_if(|view: Res<HudView>| matches!(*view, HudView::Building | HudView::Chest)),
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
    let gamepad = gamepads.iter().any(|gp| gp.just_pressed(bevy::input::gamepad::GamepadButton::East));
    (key, click, gamepad)
}

fn find_nearest_building<'a>(
    player_pos: Vec3,
    cam_global: &GlobalTransform,
    buildings: &'a Query<(&BuildingInfo, &GlobalTransform)>,
    use_raycast: bool,
) -> Option<&'a BuildingInfo> {
    let mut nearest: Option<(&BuildingInfo, f32)> = None;

    for (info, _) in buildings.iter() {
        let dist = player_pos.distance(info.position);
        if dist < INTERACT_RANGE {
            if nearest.is_none() || dist < nearest.unwrap().1 {
                nearest = Some((info, dist));
            }
        }
    }

    if use_raycast {
        let ray_origin = cam_global.translation();
        let ray_dir = cam_global.forward().as_vec3();
        for (info, _) in buildings.iter() {
            let to_building = info.position - ray_origin;
            let along_ray = to_building.dot(ray_dir);
            if along_ray < 0.0 || along_ray > RAYCAST_RANGE { continue; }
            let closest_point = ray_origin + ray_dir * along_ray;
            let perp_dist = closest_point.distance(info.position);
            if perp_dist < 500.0 {
                if nearest.is_none() || along_ray < nearest.unwrap().1 {
                    nearest = Some((info, along_ray));
                }
            }
        }
    }

    nearest.map(|(info, _)| info)
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

/// Detect interaction input and push events from the building's EVT script to the queue.
fn interact_system(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    gamepads: Query<&Gamepad>,
    player_query: Query<&Transform, With<Player>>,
    camera_query: Query<(&GlobalTransform, &Camera), With<PlayerCamera>>,
    buildings: Query<(&BuildingInfo, &GlobalTransform)>,
    map_events: Option<Res<MapEvents>>,
    mut event_queue: ResMut<EventQueue>,
    cursor_query: Query<&CursorOptions, With<PrimaryWindow>>,
) {
    let Ok(player_tf) = player_query.single() else { return };
    let Ok((cam_global, _)) = camera_query.single() else { return };

    let (key, click, gamepad) = check_interact_input(&keys, &mouse, &gamepads);
    if !key && !click && !gamepad { return; }

    let cursor_grabbed = cursor_query.single()
        .map(|c| !matches!(c.grab_mode, CursorGrabMode::None)).unwrap_or(true);
    if click && !cursor_grabbed { return; }

    let use_raycast = click || gamepad;
    let Some(info) = find_nearest_building(player_tf.translation, cam_global, &buildings, use_raycast) else {
        return;
    };

    let Some(me) = map_events else { return };
    let Some(evt) = me.evt.as_ref() else { return };

    for &eid in &info.event_ids {
        event_queue.push_all(eid, evt);
    }
}

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
        if let Some(actions) = evt.events.get(&eid) {
            for action in actions {
                match action {
                    lod::evt::GameEvent::OpenChest { id } => {
                        return Some(format!("Chest #{}", id));
                    }
                    lod::evt::GameEvent::SpeakInHouse { house_id } => {
                        if let Some(houses) = me.houses.as_ref() {
                            if let Some(entry) = houses.houses.get(house_id) {
                                return Some(entry.name.clone());
                            }
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
                    lod::evt::GameEvent::Exit => {}
                }
            }
        }
    }
    None
}

/// Show the name of the nearest interactive building in the footer bar.
fn hover_hint_system(
    player_query: Query<&Transform, With<Player>>,
    buildings: Query<(&BuildingInfo, &GlobalTransform)>,
    map_events: Option<Res<MapEvents>>,
    mut footer: ResMut<FooterText>,
) {
    let Ok(player_tf) = player_query.single() else { return };

    // Proximity-only check for hover (no raycast — saves a full building scan per frame)
    if let Some(info) = find_nearest_building(player_tf.translation, &GlobalTransform::default(), &buildings, false) {
        if let Some(name) = resolve_building_name(info, &map_events) {
            footer.set(&name);
            return;
        }
    }

    // Nothing nearby — clear footer
    footer.clear();
}
