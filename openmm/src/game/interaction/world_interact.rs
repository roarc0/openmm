use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use crate::game::actors::KillActorEvent;
use crate::game::events::EventQueue;
use crate::game::events::{GENERATED_NPC_ID_BASE, MapEvents};
use crate::game::map::indoor::OccluderFaces;
use crate::game::map::spatial_index::EntitySpatialIndex;
use crate::game::player::PlayerCamera;
use crate::game::sprites::loading::SpriteSheet;
use crate::game::state::WorldState;

use super::clickable;
use super::raycast::{billboard_hit_test, point_in_polygon, ray_plane_intersect, resolve_event_name};
use super::{
    DecorationInfo, DecorationTrigger, MAX_INTERACT_RANGE, MonsterInteractable, NpcInteractable, check_interact_input,
    facing_rotation,
};
use crate::game::player::Player;
use crate::game::ui::UiState;
use crate::system::config::GameConfig;

use bevy::ecs::system::SystemParam;

#[derive(SystemParam)]
pub(crate) struct WorldInteractParams<'w, 's> {
    pub camera_query: Query<'w, 's, (&'static GlobalTransform, &'static Camera), With<PlayerCamera>>,
    pub decorations: Query<
        'w,
        's,
        (
            &'static DecorationInfo,
            &'static GlobalTransform,
            Option<&'static SpriteSheet>,
        ),
    >,
    pub npcs: Query<'w, 's, (&'static NpcInteractable, &'static GlobalTransform, &'static SpriteSheet)>,
    pub monsters: Query<
        'w,
        's,
        (
            Entity,
            &'static MonsterInteractable,
            &'static GlobalTransform,
            &'static SpriteSheet,
        ),
    >,
    pub clickable_faces: Option<Res<'w, clickable::Faces>>,
    pub occluder_faces: Option<ResMut<'w, OccluderFaces>>,
    pub map_events: Option<Res<'w, MapEvents>>,
    pub spatial: Res<'w, EntitySpatialIndex>,
    pub event_queue: ResMut<'w, EventQueue>,
    pub kill_events: Option<bevy::ecs::message::MessageWriter<'w, KillActorEvent>>,
    pub world_state: Option<Res<'w, WorldState>>,
    pub ui: ResMut<'w, UiState>,
    pub cfg: Res<'w, GameConfig>,
    pub time: Res<'w, Time>,
}

/// Detect click/interact on the nearest interactable in the world (decoration, NPC, or BSP face)
/// and push exactly one event. By finding the global nearest hit before pushing, this guarantees
/// only one UI can open per interaction — no stacking of events from overlapping targets.
pub(crate) fn world_interact_system(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    gamepads: Query<&Gamepad>,
    cursor_query: Query<&CursorOptions, With<PrimaryWindow>>,
    mut params: WorldInteractParams,
) {
    let Ok((cam_global, _)) = params.camera_query.single() else {
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
    let occluder_t = params
        .occluder_faces
        .as_mut()
        .map(|of| of.min_hit_t_max(origin, dir, params.cfg.draw_distance))
        .unwrap_or(f32::MAX);

    // Find the single nearest hit across all interactable types.
    enum Hit {
        Face(u16, Option<String>),
        /// Carries (event_id, billboard_index, display_name)
        Decoration(u16, usize, Option<String>),
        Npc(i16, String),
        Monster(Entity, String),
    }
    let mut nearest: Option<(f32, Hit)> = None;

    // BSP faces (outdoor buildings) — capped to arm's reach like all other interactables.
    if let Some(faces) = params.clickable_faces.as_ref() {
        for face in &faces.faces {
            if let Some(t) = ray_plane_intersect(origin, dir, face.normal, face.plane_dist) {
                if t > params.cfg.draw_distance {
                    continue;
                }
                let hit = origin + dir * t;
                if point_in_polygon(hit, &face.vertices, face.normal) && nearest.as_ref().is_none_or(|n| t < n.0) {
                    let name = resolve_event_name(face.event_id, &params.map_events);
                    nearest = Some((t, Hit::Face(face.event_id, name)));
                }
            }
        }
    }

    // Spatially gated pass over billboard entities. The spatial index already
    // narrows the candidate list to entities whose cell overlaps the interact
    // range, so the heavy matrix + polygon + mask work here only runs for a
    // handful of nearby entities instead of every `WorldEntity` on the map.
    for entity in params
        .spatial
        .query_radius(origin.x, origin.z, params.cfg.draw_distance)
    {
        if let Ok((info, g_tf, sheet_opt)) = params.decorations.get(entity) {
            let center = g_tf.translation();
            if origin.distance_squared(center) > params.cfg.draw_distance * params.cfg.draw_distance {
                continue;
            }
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
                center,
                facing_rotation(origin, center),
                half_w,
                half_h,
                mask,
            ) && t < occluder_t
                && t < params.cfg.draw_distance
                && nearest.as_ref().is_none_or(|n| t < n.0)
            {
                let name = resolve_event_name(info.event_id, &params.map_events).or_else(|| info.display_name.clone());
                nearest = Some((t, Hit::Decoration(info.event_id, info.billboard_index, name)));
            }
            continue;
        }

        if let Ok((info, g_tf, sheet)) = params.npcs.get(entity) {
            let center = g_tf.translation();
            if origin.distance_squared(center) > MAX_INTERACT_RANGE * MAX_INTERACT_RANGE {
                continue;
            }
            let Some(&(sw, sh)) = sheet.state_dimensions.get(sheet.current_state) else {
                continue;
            };
            if let Some(t) = billboard_hit_test(
                origin,
                dir,
                center,
                facing_rotation(origin, center),
                sw / 2.0,
                sh / 2.0,
                sheet.current_mask.as_deref(),
            ) && t < occluder_t
                && t < MAX_INTERACT_RANGE
                && nearest.as_ref().is_none_or(|n| t < n.0)
            {
                nearest = Some((t, Hit::Npc(info.npc_id, info.name.clone())));
            }
            continue;
        }

        if let Ok((_, info, g_tf, sheet)) = params.monsters.get(entity) {
            let center = g_tf.translation();
            if origin.distance_squared(center) > MAX_INTERACT_RANGE * MAX_INTERACT_RANGE {
                continue;
            }
            let Some(&(sw, sh)) = sheet.state_dimensions.get(sheet.current_state) else {
                continue;
            };
            if let Some(t) = billboard_hit_test(
                origin,
                dir,
                center,
                facing_rotation(origin, center),
                sw / 2.0,
                sh / 2.0,
                sheet.current_mask.as_deref(),
            ) && t < occluder_t
                && t < MAX_INTERACT_RANGE
                && nearest.as_ref().is_none_or(|n| t < n.0)
            {
                nearest = Some((t, Hit::Monster(entity, info.name.clone())));
            }
        }
    }

    let now = params.time.elapsed_secs_f64();
    match nearest {
        Some((dist, Hit::Face(event_id, name))) => {
            if let Some(name) = name {
                params.ui.footer.set_status(&name, 2.0, now);
            }
            if dist < MAX_INTERACT_RANGE {
                info!("World interact: hit BSP face event_id={} at dist={:.0}", event_id, dist);
                if let Some(me) = params.map_events.as_ref()
                    && let Some(evt) = me.evt.as_ref()
                {
                    params.event_queue.push_all(event_id, evt);
                }
            }
        }
        Some((dist, Hit::Decoration(event_id, billboard_idx, name))) => {
            if let Some(name) = name {
                params.ui.footer.set_status(&name, 2.0, now);
            }
            if dist < MAX_INTERACT_RANGE {
                // ChangeEvent can redirect this decoration to a different script at runtime.
                let effective_id = params
                    .world_state
                    .as_ref()
                    .and_then(|ws| ws.game_vars.event_overrides.get(&billboard_idx))
                    .copied()
                    .unwrap_or(event_id);
                if let Some(me) = params.map_events.as_ref()
                    && let Some(evt) = me.evt.as_ref()
                {
                    params.event_queue.push_all(effective_id, evt);
                }
            }
        }
        Some((dist, Hit::Npc(npc_id, name))) => {
            params.ui.footer.set_status(&name, 2.0, now);
            if dist < MAX_INTERACT_RANGE {
                let npc_id_i32 = npc_id as i32;
                // For quest NPCs (from npcdata.txt), run their event_a script if available.
                // event_a is the "speak to" script — it typically contains SpeakNPC + dialogue options.
                let ran_event = if npc_id_i32 > 0 && npc_id_i32 < GENERATED_NPC_ID_BASE {
                    if let Some(me) = params.map_events.as_ref()
                        && let Some(evt) = me.evt.as_ref()
                        && let Some(entry) = me.npc_table.as_ref().and_then(|t| t.get(npc_id_i32))
                        && entry.event_a > 0
                    {
                        params.event_queue.push_all(entry.event_a as u16, evt);
                        true
                    } else {
                        false
                    }
                } else {
                    false
                };
                if !ran_event {
                    params
                        .event_queue
                        .push_single(openmm_data::evt::GameEvent::SpeakNPC { npc_id: npc_id_i32 });
                }
            }
        }
        Some((dist, Hit::Monster(entity, name))) => {
            params.ui.footer.set_status(&name, 2.0, now);
            if dist < MAX_INTERACT_RANGE
                && let Some(mut ke) = params.kill_events
            {
                ke.write(KillActorEvent(entity));
            }
        }
        None => {}
    }
}

/// Fire EVT events when the player enters a decoration's trigger radius.
/// Only fires on the rising edge (entering range), not while staying in range.
pub(crate) fn decoration_proximity_system(
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
