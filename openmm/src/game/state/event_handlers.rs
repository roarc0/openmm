//! Side-effect event handlers — each handles a specific `GameEvent` variant
//! that produces heavy side effects (Commands, image loading, map transitions).
//
// Each function handles a specific GameEvent variant that produces heavy
// side effects (Commands, image loading, map transitions, view switching).

use bevy::prelude::*;
use bevy::window::{CursorOptions, PrimaryWindow};

use openmm_data::utils::MapName;

use crate::GameState;
use crate::assets::GameAssets;
use crate::game::coords::{mm6_binary_angle_to_radians, mm6_position_to_bevy};
use crate::game::optional::OptionalWrite;
use crate::game::sound::SoundManager;
use crate::game::sound::effects::PlayUiSoundEvent;
use crate::game::sprites::material::SpriteMaterial;
use crate::game::state::ui_state::{self, UiMode, UiState};
use crate::prepare::loading::LoadRequest;

use super::events::MapEvents;
use super::scripting::{AudioParams, EventQueue, MapEntityParams, TransitionParams};

/// Show the autonote text in the footer when a note is newly acquired.
pub(super) fn show_autonote_text(id: i32, assets: &GameAssets, ui: &mut UiState, time_secs: f64) {
    if let Some(note) = assets.autonotes().and_then(|t| t.get(id as u16))
        && !note.text.is_empty()
    {
        ui.footer.set_status(&note.text, 4.0, time_secs);
    }
}

/// Queue a non-positional UI sound by its `dsounds.bin` name (e.g. `"Quest"`,
/// `"EventSFX01"`). Silent no-op if the sound manager isn't available
/// (headless build) or the name isn't in the table.
///
/// Kept generic so any event handler that needs a named jingle — pickups,
/// quest completions, UI feedback — can reach for one call instead of
/// reimplementing the `dsounds -> sound_id -> PlayUiSoundEvent` chain.
pub(super) fn play_ui_sound_named(
    name: &str,
    sound_manager: Option<&SoundManager>,
    ui_sound: &mut Option<bevy::ecs::message::MessageWriter<PlayUiSoundEvent>>,
) {
    let Some(sm) = sound_manager else {
        return;
    };
    let Some(sound_id) = sm.dsounds.get_by_name(name).map(|s| s.sound_id) else {
        warn!("ui sound '{}' not found in dsounds", name);
        return;
    };
    ui_sound.try_write(PlayUiSoundEvent { sound_id });
}

/// Handle SpeakInHouse: show building description and overlay image.
pub(super) fn handle_speak_in_house(
    house_id: u32,
    game_assets: &GameAssets,
    map_events: &Option<Res<MapEvents>>,
    images: &mut Assets<Image>,
    commands: &mut Commands,
    ui: &mut UiState,
    cursor_query: &mut Query<&mut CursorOptions, With<PrimaryWindow>>,
    time: &Time,
) {
    // Show transition/location description if one exists for this house_id.
    if let Some(desc) = game_assets
        .trans()
        .and_then(|t| t.get(house_id as u16))
        .map(|e| e.description.clone())
        .filter(|s| !s.is_empty())
    {
        ui.footer.set_status(&desc, 4.0, time.elapsed_secs_f64());
    }
    let image = map_events
        .as_ref()
        .and_then(|me| super::events::resolve_building_image(house_id, me, game_assets, images))
        .or_else(|| game_assets.load_icon("evt02", images));
    if let Some(image) = image {
        ui_state::set_overlay_mode(commands, ui, cursor_query, image, UiMode::Building);
    }
}

/// Handle OpenChest: load chest icon, play sound, show overlay.
pub(super) fn handle_open_chest(
    id: u8,
    game_assets: &GameAssets,
    images: &mut Assets<Image>,
    commands: &mut Commands,
    ui: &mut UiState,
    cursor_query: &mut Query<&mut CursorOptions, With<PrimaryWindow>>,
    audio: &mut AudioParams,
) {
    debug!("OpenChest(id={})", id);
    let icon_name = format!("chest{:02}", id);
    if let Some(image) = game_assets.load_icon(&icon_name, images) {
        // Play chest-open sound if available
        if let Some(ref sm) = audio.sound_manager
            && let Some(s) = sm.dsounds.get_by_name("openchest0101")
        {
            audio.ui_sound.try_write(PlayUiSoundEvent { sound_id: s.sound_id });
        }
        ui_state::set_overlay_mode(commands, ui, cursor_query, image, UiMode::Chest);
    }
}

/// Handle MoveToMap: same-map teleport or cross-map transition.
/// Always clears the event queue and terminates the current sequence.
pub(super) fn handle_move_to_map(
    map_name: &str,
    x: i32,
    y: i32,
    z: i32,
    direction: i32,
    event_queue: &mut EventQueue,
    audio: &mut AudioParams,
    entities: &mut MapEntityParams,
    transition: &mut TransitionParams,
    world_state: &mut super::state::WorldState,
    commands: &mut Commands,
) {
    // A name with no letters (e.g. "0") means same-map teleport — just
    // reposition the player without reloading the map.
    // The original MM6 engine hardcodes playing the teleport sound here
    // (there is no PlaySound step in the EVT data for MoveToMap events).
    if !map_name.chars().any(|c| c.is_ascii_alphabetic()) {
        if let Some(ref sm) = audio.sound_manager
            && let Some(s) = sm.dsounds.get_by_name("teleport")
        {
            audio.ui_sound.try_write(PlayUiSoundEvent { sound_id: s.sound_id });
        }
        let base = Vec3::from(mm6_position_to_bevy(x, y, z));
        // Player Transform.y is at eye level (feet + eye_height), same as spawn.
        let pos = Vec3::new(base.x, base.y + entities.player_settings.eye_height, base.z);
        let yaw = mm6_binary_angle_to_radians(direction);
        if let Ok(mut tf) = entities.player.single_mut() {
            tf.translation = pos;
            tf.rotation = Quat::from_rotation_y(yaw);
            info!(
                "MoveToMap same-map teleport: pos={:?} yaw={:.1}deg",
                pos,
                yaw.to_degrees()
            );
        }
        event_queue.clear();
        return;
    }
    let Ok(target) = MapName::try_from(map_name) else {
        warn!("MoveToMap: invalid map name '{}'", map_name);
        return;
    };

    let pos = mm6_position_to_bevy(x, y, z);
    let yaw = mm6_binary_angle_to_radians(direction);

    debug!(
        "MoveToMap: '{}' mm6=({},{},{}) dir={} -> bevy={:?} yaw={:.1}deg",
        map_name,
        x,
        y,
        z,
        direction,
        pos,
        yaw.to_degrees()
    );

    if let MapName::Outdoor(ref odm) = target {
        transition.save_data.map.map_x = odm.x;
        transition.save_data.map.map_y = odm.y;
        world_state.map.map_x = odm.x;
        world_state.map.map_y = odm.y;
    }
    world_state.map.name = target.clone();

    transition.save_data.player.position = pos;
    transition.save_data.player.yaw = yaw;

    commands.insert_resource(LoadRequest {
        map_name: target,
        spawn_position: Some(pos),
        spawn_yaw: Some(yaw),
    });
    transition.game_state.set(GameState::Loading);

    // Reset map vars on map transition
    world_state.game_vars.map_vars = [0; 100];
    event_queue.clear();
}

/// Handle SetSprite: replace a decoration's sprite mesh and material.
pub(super) fn handle_set_sprite(
    decoration_id: i32,
    sprite_name: &str,
    game_assets: &GameAssets,
    images: &mut Assets<Image>,
    sprite_materials: Option<&mut Assets<SpriteMaterial>>,
    meshes: &mut Assets<Mesh>,
    entities: &mut MapEntityParams,
) {
    info!("SetSprite: deco={} sprite='{}'", decoration_id, sprite_name);
    let target_idx = decoration_id as usize;
    // Find the target entity first to get declist_id and ground_y.
    let target = entities
        .decorations
        .iter()
        .find(|(d, ..)| d.billboard_index == target_idx)
        .map(|(d, ..)| (d.declist_id, d.ground_y));
    let Some((declist_id, ground_y)) = target else {
        debug!("SetSprite: decoration {} not found", target_idx);
        return;
    };
    let _declist_id = declist_id; // needed for future directional sprite swap
    let Some(sprite_materials) = sprite_materials else {
        return;
    };
    // New materials reference the shared tint storage buffer, so
    // they pick up the current day/night tint automatically without
    // any per-material write here. Default to regular; selflit sprite
    // swaps aren't handled by the current SetSprite opcode.
    let Some((new_mat, new_mesh, _new_w, new_h)) = crate::game::sprites::loading::load_static_decoration_sprite(
        sprite_name,
        game_assets.assets(),
        images,
        sprite_materials,
        meshes,
        false,
    ) else {
        warn!("SetSprite: sprite '{}' not found in LOD", sprite_name);
        return;
    };
    for (deco_info, mut mat_handle, mut mesh_handle, mut transform) in entities.decorations.iter_mut() {
        if deco_info.billboard_index == target_idx {
            transform.translation.y = ground_y + new_h / 2.0;
            mesh_handle.0 = new_mesh.clone();
            mat_handle.0 = new_mat.clone();
            break;
        }
    }
}

/// Handle SpeakNPC: prepare NPC dialogue portrait and profile, switch to dialogue view.
pub(super) fn handle_speak_npc(
    npc_id: i32,
    game_assets: &GameAssets,
    map_events: &Option<Res<MapEvents>>,
    images: &mut Assets<Image>,
    commands: &mut Commands,
    ui: &mut UiState,
    cursor_query: &mut Query<&mut CursorOptions, With<PrimaryWindow>>,
    audio: &AudioParams,
    world_state: &super::state::WorldState,
) {
    // day_of_week: GameTime uses 0=Monday epoch; proftext uses 0=Sunday.
    // Shift by 6 to convert: Monday(0)->1, ..., Sunday(6)->0.
    let dow = audio
        .game_time
        .as_ref()
        .map(|gt| (gt.day_of_week() + 6) % 7)
        .unwrap_or(0);
    if let Some((portrait, profile)) = super::npc_dialogue::prepare_npc_dialogue(
        npc_id,
        map_events,
        game_assets,
        images,
        dow,
        &world_state.game_vars.npc_greetings,
    ) {
        commands.insert_resource(portrait);
        commands.insert_resource(profile);
        ui_state::set_ui_mode(ui, cursor_query, UiMode::NpcDialogue);
    }
}
