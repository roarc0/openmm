use std::collections::VecDeque;

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use lod::evt::{EvtFile, GameEvent};
use lod::odm::mm6_to_bevy;

use crate::GameState;
use crate::assets::GameAssets;
use crate::game::events::MapEvents;
use crate::game::hud::{FooterText, HudView, OverlayImage};
use crate::game::map_name::MapName;
use crate::save::GameSave;
use crate::states::loading::LoadRequest;

/// Queue of game events waiting to be processed, one per frame.
#[derive(Resource, Default)]
pub struct EventQueue {
    queue: VecDeque<GameEvent>,
}

impl EventQueue {
    /// Push an event to the back of the queue.
    pub fn push(&mut self, event: GameEvent) {
        self.queue.push_back(event);
    }

    /// Push an event to the front of the queue (high priority).
    pub fn push_front(&mut self, event: GameEvent) {
        self.queue.push_front(event);
    }

    /// Pop the next event from the front.
    pub fn pop(&mut self) -> Option<GameEvent> {
        self.queue.pop_front()
    }

    /// Enqueue all actions for a given event_id from the EvtFile.
    pub fn push_all(&mut self, event_id: u16, evt: &EvtFile) {
        if let Some(actions) = evt.events.get(&event_id) {
            for action in actions {
                self.queue.push_back(action.clone());
            }
        }
    }

    /// Returns true if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

pub struct EventDispatchPlugin;

impl Plugin for EventDispatchPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EventQueue>()
            .add_systems(
                Update,
                process_events.run_if(in_state(GameState::Game)),
            );
    }
}

/// Map a building type string from 2devents.txt to its background image name.
fn building_background(building_type: &str) -> &'static str {
    let lower = building_type.to_lowercase();
    if lower.contains("weapon") {
        return "wepntabl";
    }
    if lower.contains("armor") {
        return "armory";
    }
    if lower.contains("magic") || lower.contains("guild") || lower.contains("alchemy") {
        return "magshelf";
    }
    if lower.contains("general") || lower.contains("store") {
        return "genshelf";
    }
    // Taverns, temples, training, houses, stables, banks, etc. use dialogue background
    "evt02"
}

/// Load an icon from the LOD archive as a Bevy Image handle with nearest-neighbor sampling.
fn load_icon(
    name: &str,
    game_assets: &GameAssets,
    images: &mut Assets<Image>,
) -> Option<Handle<Image>> {
    let img = game_assets.lod_manager().icon(name)?;
    let mut bevy_img = crate::assets::dynamic_to_bevy_image(img);
    bevy_img.sampler = bevy::image::ImageSampler::nearest();
    Some(images.add(bevy_img))
}

/// Resolve the background image for a building interaction.
/// Tries the house's picture_id first (e.g. "evt07"), then falls back to
/// building_background() based on type, and finally "evt02" as a last resort.
fn resolve_building_image(
    house_id: u32,
    map_events: &MapEvents,
    game_assets: &GameAssets,
    images: &mut Assets<Image>,
) -> Option<Handle<Image>> {
    if let Some(houses) = map_events.houses.as_ref() {
        if let Some(entry) = houses.houses.get(&house_id) {
            // Try picture_id-based icon first (e.g. "evt07")
            let pic_name = format!("evt{:02}", entry.picture_id);
            if let Some(handle) = load_icon(&pic_name, game_assets, images) {
                return Some(handle);
            }
            // Fall back to building type
            return load_icon(building_background(&entry.building_type), game_assets, images);
        }
    }
    // Last resort
    load_icon("evt02", game_assets, images)
}

/// Set cursor grab mode and visibility.
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

/// Process one event per frame from the EventQueue.
fn process_events(
    mut event_queue: ResMut<EventQueue>,
    map_events: Option<Res<MapEvents>>,
    game_assets: Res<GameAssets>,
    mut images: ResMut<Assets<Image>>,
    mut commands: Commands,
    mut hud_view: ResMut<HudView>,
    mut footer: ResMut<FooterText>,
    mut cursor_query: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mut save_data: ResMut<GameSave>,
    mut game_state: ResMut<NextState<GameState>>,
    mut blv_doors: Option<ResMut<crate::game::blv::BlvDoors>>,
) {
    // Don't process events while a UI overlay is blocking
    if !matches!(*hud_view, HudView::World) {
        return;
    }

    let Some(event) = event_queue.pop() else {
        return;
    };

    debug!("EventDispatch: {:?}", event);

    match event {
        GameEvent::Hint { text, .. } => {
            footer.set(&text);
        }
        GameEvent::SpeakInHouse { house_id } => {
            let image = map_events
                .as_ref()
                .and_then(|me| resolve_building_image(house_id, me, &game_assets, &mut images))
                .or_else(|| load_icon("evt02", &game_assets, &mut images));
            if let Some(image) = image {
                commands.insert_resource(OverlayImage { image });
                *hud_view = HudView::Building;
                grab_cursor(&mut cursor_query, false);
            }
        }
        GameEvent::OpenChest { id } => {
            if let Some(image) = load_icon("chest01", &game_assets, &mut images) {
                commands.insert_resource(OverlayImage { image });
                *hud_view = HudView::Chest;
                grab_cursor(&mut cursor_query, false);
            }
        }
        GameEvent::MoveToMap {
            x,
            y,
            z,
            direction,
            map_name,
        } => {
            let Ok(target) = MapName::try_from(map_name.as_str()) else {
                warn!("MoveToMap: invalid map name '{}'", map_name);
                return;
            };

            // Convert MM6 coords to Bevy
            let pos = mm6_to_bevy(x, y, z);

            // Convert direction (0-65535 range, 0=east, counter-clockwise) to yaw radians
            // MM6 direction: 0=east, 512=north, 1024=west, 1536=south (in 2048 units per circle)
            // But EVT uses 0-65535 range (65536 units per circle)
            let yaw = (direction as f32) * std::f32::consts::TAU / 65536.0;

            // Update save data
            save_data.player.position = pos;
            save_data.player.yaw = yaw;

            // Update map coordinates for outdoor maps
            if let MapName::Outdoor(ref odm) = target {
                save_data.map.map_x = odm.x;
                save_data.map.map_y = odm.y;
            }

            debug!("MoveToMap: '{}' mm6=({},{},{}) dir={} -> bevy={:?} yaw={:.1}deg", map_name, x, y, z, direction, pos, yaw.to_degrees());
            commands.insert_resource(LoadRequest {
                map_name: target,
            });
            game_state.set(GameState::Loading);
        }
        GameEvent::ChangeDoorState { door_id, action } => {
            debug!("ChangeDoorState door_id={} action={}", door_id, action);
            if let Some(ref mut doors) = blv_doors {
                crate::game::blv::trigger_door(doors, door_id as u32, action);
            }
        }
    }
}
