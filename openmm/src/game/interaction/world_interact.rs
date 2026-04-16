use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use crate::game::actors::KillActorEvent;
use crate::game::indoor::OccluderFaces;
use crate::game::player::PlayerCamera;
use crate::game::spatial_index::EntitySpatialIndex;
use crate::game::sprites::loading::SpriteSheet;
use crate::game::world::EventQueue;
use crate::game::world::WorldState;
use crate::game::world::{GENERATED_NPC_ID_BASE, MapEvents};

use super::clickable;
use super::raycast::{billboard_hit_test, point_in_polygon, ray_plane_intersect};
use super::{
    DecorationInfo, DecorationTrigger, MAX_INTERACT_RANGE, MonsterInteractable, NpcInteractable, check_interact_input,
    facing_rotation,
};
use crate::game::player::Player;

/// Detect click/interact on the nearest interactable in the world (decoration, NPC, or BSP face)
/// and push exactly one event. By finding the global nearest hit before pushing, this guarantees
/// only one UI can open per interaction — no stacking of events from overlapping targets.
pub(crate) fn world_interact_system(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    gamepads: Query<&Gamepad>,
    camera_query: Query<(&GlobalTransform, &Camera), With<PlayerCamera>>,
    decorations: Query<(&DecorationInfo, &GlobalTransform, Option<&SpriteSheet>)>,
    npcs: Query<(&NpcInteractable, &GlobalTransform, &SpriteSheet)>,
    monsters: Query<(Entity, &MonsterInteractable, &GlobalTransform, &SpriteSheet)>,
    clickable_faces: Option<Res<clickable::Faces>>,
    mut occluder_faces: Option<ResMut<OccluderFaces>>,
    map_events: Option<Res<MapEvents>>,
    spatial: Res<EntitySpatialIndex>,
    mut event_queue: ResMut<EventQueue>,
    kill_events: Option<bevy::ecs::message::MessageWriter<KillActorEvent>>,
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
        .as_mut()
        .map(|of| of.min_hit_t_max(origin, dir, MAX_INTERACT_RANGE))
        .unwrap_or(f32::MAX);

    let max_range_sq = MAX_INTERACT_RANGE * MAX_INTERACT_RANGE;

    // Find the single nearest hit across all interactable types.
    enum Hit {
        Face(u16),
        /// Carries (event_id, billboard_index) so ChangeEvent overrides can be checked.
        Decoration(u16, usize),
        Npc(i16),
        Monster(Entity),
    }
    let mut nearest: Option<(f32, Hit)> = None;

    // BSP faces (outdoor buildings) — capped to arm's reach like all other interactables.
    if let Some(faces) = clickable_faces.as_ref() {
        for face in &faces.faces {
            if let Some(t) = ray_plane_intersect(origin, dir, face.normal, face.plane_dist) {
                if t > MAX_INTERACT_RANGE {
                    continue;
                }
                let hit = origin + dir * t;
                if point_in_polygon(hit, &face.vertices, face.normal) && nearest.as_ref().is_none_or(|n| t < n.0) {
                    nearest = Some((t, Hit::Face(face.event_id)));
                }
            }
        }
    }

    // Spatially gated pass over billboard entities. The spatial index already
    // narrows the candidate list to entities whose cell overlaps the interact
    // range, so the heavy matrix + polygon + mask work here only runs for a
    // handful of nearby entities instead of every `WorldEntity` on the map.
    for entity in spatial.query_radius(origin.x, origin.z, MAX_INTERACT_RANGE) {
        if let Ok((info, g_tf, sheet_opt)) = decorations.get(entity) {
            let center = g_tf.translation();
            if origin.distance_squared(center) > max_range_sq {
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
                && t < MAX_INTERACT_RANGE
                && nearest.as_ref().is_none_or(|n| t < n.0)
            {
                nearest = Some((t, Hit::Decoration(info.event_id, info.billboard_index)));
            }
            continue;
        }

        if let Ok((info, g_tf, sheet)) = npcs.get(entity) {
            let center = g_tf.translation();
            if origin.distance_squared(center) > max_range_sq {
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
                nearest = Some((t, Hit::Npc(info.npc_id)));
            }
            continue;
        }

        if let Ok((_, _info, g_tf, sheet)) = monsters.get(entity) {
            let center = g_tf.translation();
            if origin.distance_squared(center) > max_range_sq {
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
                nearest = Some((t, Hit::Monster(entity)));
            }
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
                event_queue.push_single(openmm_data::evt::GameEvent::SpeakNPC { npc_id: npc_id_i32 });
            }
        }
        Some((_, Hit::Monster(entity))) => {
            if let Some(mut ke) = kill_events {
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
