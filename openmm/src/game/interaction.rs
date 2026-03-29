use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use crate::GameState;
use crate::assets::GameAssets;
use crate::config::GameConfig;
use crate::game::InGame;
use crate::game::events::MapEvents;
use crate::game::hud::{self, FooterText};
use crate::game::player::{Player, PlayerCamera};

// --- Components & Resources ---

/// Component on BSP model parent entities that are interactive.
#[derive(Component)]
pub struct BuildingInfo {
    pub model_name: String,
    pub position: Vec3,
    pub event_ids: Vec<u16>,
}

/// The active interaction — just the background image to display.
#[derive(Resource)]
pub struct ActiveInteraction {
    pub image: Handle<Image>,
}

/// Sub-state of GameState::Game.
#[derive(Clone, Copy, Default, Eq, PartialEq, Debug, Hash, SubStates)]
#[source(GameState = GameState::Game)]
pub(crate) enum InGameState {
    #[default]
    Playing,
    Interacting,
}

/// Marker for interaction UI overlay entities.
#[derive(Component)]
struct InteractionUI;

const INTERACT_RANGE: f32 = 400.0;
const RAYCAST_RANGE: f32 = 2000.0;

// --- Plugin ---

pub struct InteractionPlugin;

impl Plugin for InteractionPlugin {
    fn build(&self, app: &mut App) {
        app.add_sub_state::<InGameState>()
            .add_systems(
                Update,
                (hover_hint_system, interact_system)
                    .chain()
                    .run_if(in_state(InGameState::Playing)),
            )
            .add_systems(OnEnter(InGameState::Interacting), show_interaction_image)
            .add_systems(
                Update,
                interaction_input.run_if(in_state(InGameState::Interacting)),
            )
            .add_systems(OnExit(InGameState::Interacting), hide_interaction_image);
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

/// Map a building type string from 2devents.txt to its background image name.
fn building_background(building_type: &str) -> &'static str {
    let lower = building_type.to_lowercase();
    if lower.contains("weapon") { return "wepntabl"; }
    if lower.contains("armor") { return "armory"; }
    if lower.contains("magic") || lower.contains("guild") || lower.contains("alchemy") { return "magshelf"; }
    if lower.contains("general") || lower.contains("store") { return "genshelf"; }
    // Taverns, temples, training, houses, stables, banks, etc. use dialogue background
    "evt02"
}

/// Resolve the interaction image from event data.
fn resolve_image(
    info: &BuildingInfo,
    map_events: &Option<Res<MapEvents>>,
    game_assets: &GameAssets,
    images: &mut Assets<Image>,
) -> Option<Handle<Image>> {
    let me = map_events.as_ref()?;
    let evt = me.evt.as_ref()?;

    for &eid in &info.event_ids {
        if let Some(actions) = evt.events.get(&eid) {
            for action in actions {
                let icon_name = match action {
                    lod::evt::EventAction::OpenChest { .. } => Some("chest01"),
                    lod::evt::EventAction::SpeakInHouse { house_id } => {
                        Some(if let Some(houses) = me.houses.as_ref() {
                            if let Some(entry) = houses.houses.get(house_id) {
                                building_background(&entry.building_type)
                            } else {
                                "evt02"
                            }
                        } else {
                            "evt02"
                        })
                    }
                    lod::evt::EventAction::MoveToMap { .. } => Some("evt02"),
                    _ => None,
                };
                if let Some(name) = icon_name {
                    let img = game_assets.lod_manager().icon(name)?;
                    let mut bevy_img = crate::assets::dynamic_to_bevy_image(img);
                    bevy_img.sampler = bevy::image::ImageSampler::nearest();
                    return Some(images.add(bevy_img));
                }
            }
        }
    }
    None
}

fn grab_cursor(cursor_query: &mut Query<&mut CursorOptions, With<PrimaryWindow>>, grab: bool) {
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

fn interact_system(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    gamepads: Query<&Gamepad>,
    player_query: Query<&Transform, With<Player>>,
    camera_query: Query<(&GlobalTransform, &Camera), With<PlayerCamera>>,
    buildings: Query<(&BuildingInfo, &GlobalTransform)>,
    map_events: Option<Res<MapEvents>>,
    game_assets: Res<GameAssets>,
    mut images: ResMut<Assets<Image>>,
    mut commands: Commands,
    mut game_state: ResMut<NextState<InGameState>>,
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

    match resolve_image(info, &map_events, &game_assets, &mut images) {
        Some(image) => {
            info!("Opening interaction for '{}' event_ids={:?}", info.model_name, info.event_ids);
            commands.insert_resource(ActiveInteraction { image });
            game_state.set(InGameState::Interacting);
        }
        None => {
            info!("No image found for '{}' event_ids={:?}", info.model_name, info.event_ids);
        }
    }
}

fn show_interaction_image(
    mut commands: Commands,
    interaction: Option<Res<ActiveInteraction>>,
    mut cursor_query: Query<&mut CursorOptions, With<PrimaryWindow>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cfg: Res<GameConfig>,
    ui_assets: Res<crate::ui_assets::UiAssets>,
) {
    let Some(interaction) = interaction else { return };
    grab_cursor(&mut cursor_query, false);

    // Position the interaction UI over just the 3D viewport area (not the HUD)
    let (vp_left, vp_top, vp_w, vp_h) = windows
        .single()
        .map(|w| hud::viewport_rect(w, &cfg, &ui_assets))
        .unwrap_or((0.0, 0.0, 640.0, 480.0));

    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(vp_left),
            top: Val::Px(vp_top),
            width: Val::Px(vp_w),
            height: Val::Px(vp_h),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        InteractionUI,
        InGame,
    )).with_children(|parent| {
        parent.spawn((
            ImageNode::new(interaction.image.clone()),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
        ));
    });
}

fn interaction_input(
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut game_state: ResMut<NextState<InGameState>>,
    mut commands: Commands,
) {
    if check_exit_input(&keys, &gamepads) {
        commands.remove_resource::<ActiveInteraction>();
        game_state.set(InGameState::Playing);
    }
}

fn hide_interaction_image(
    mut commands: Commands,
    ui_query: Query<Entity, With<InteractionUI>>,
    mut cursor_query: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    for entity in ui_query.iter() {
        commands.entity(entity).despawn();
    }
    grab_cursor(&mut cursor_query, true);
}

/// Resolve a human-readable name for a building from its event data.
fn resolve_building_name(info: &BuildingInfo, map_events: &Option<Res<MapEvents>>) -> Option<String> {
    let me = map_events.as_ref()?;
    let evt = me.evt.as_ref()?;

    for &eid in &info.event_ids {
        if let Some(actions) = evt.events.get(&eid) {
            for action in actions {
                match action {
                    lod::evt::EventAction::OpenChest { id } => {
                        return Some(format!("Chest #{}", id));
                    }
                    lod::evt::EventAction::SpeakInHouse { house_id } => {
                        if let Some(houses) = me.houses.as_ref() {
                            if let Some(entry) = houses.houses.get(house_id) {
                                return Some(entry.name.clone());
                            }
                        }
                        return Some(format!("Building #{}", house_id));
                    }
                    lod::evt::EventAction::Hint { text, .. } => {
                        if !text.is_empty() {
                            return Some(text.clone());
                        }
                    }
                    lod::evt::EventAction::MoveToMap { map_name, .. } => {
                        return Some(format!("Enter {}", map_name));
                    }
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
