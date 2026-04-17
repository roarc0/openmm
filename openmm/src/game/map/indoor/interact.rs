//! Indoor interaction and touch trigger systems.

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use crate::game::events::EventQueue;
use crate::game::events::MapEvents;
use crate::game::interaction::check_interact_input;
use crate::game::interaction::raycast::{point_in_polygon, ray_plane_intersect};
use crate::game::player::PlayerCamera;

use super::types::TouchTriggerFaces;

pub(crate) const INDOOR_INTERACT_RANGE: f32 = 5120.0;

/// Detect indoor face interaction (Enter/click) and dispatch EVT events.
pub(crate) fn indoor_interact_system(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    gamepads: Query<&Gamepad>,
    camera_query: Query<(&GlobalTransform, &Camera), With<PlayerCamera>>,
    clickable: Option<Res<crate::game::interaction::clickable::Faces>>,
    map_events: Option<Res<MapEvents>>,
    mut event_queue: ResMut<EventQueue>,
    cursor_query: Query<&CursorOptions, With<PrimaryWindow>>,
) {
    let Some(clickable) = clickable else { return };
    if !clickable.is_indoor || clickable.faces.is_empty() {
        return;
    }

    let (key, click, gamepad) = check_interact_input(&keys, &mouse, &gamepads);
    if !key && !click && !gamepad {
        return;
    }

    // Don't process click if cursor isn't grabbed
    if click {
        let cursor_grabbed = cursor_query
            .single()
            .map(|c| !matches!(c.grab_mode, CursorGrabMode::None))
            .unwrap_or(true);
        if !cursor_grabbed {
            return;
        }
    }

    let Ok((cam_global, _)) = camera_query.single() else {
        return;
    };
    let ray_origin = cam_global.translation();
    let ray_dir = cam_global.forward().as_vec3();

    // Find nearest clickable face hit
    let mut nearest_hit: Option<(f32, u16)> = None;
    for face in &clickable.faces {
        if let Some(t) = ray_plane_intersect(ray_origin, ray_dir, face.normal, face.plane_dist) {
            if t > INDOOR_INTERACT_RANGE {
                continue;
            }
            let hit_point = ray_origin + ray_dir * t;
            if point_in_polygon(hit_point, &face.vertices, face.normal)
                && (nearest_hit.is_none() || t < nearest_hit.unwrap().0)
            {
                nearest_hit = Some((t, face.event_id));
            }
        }
    }

    if let Some((dist, event_id)) = nearest_hit {
        info!("Indoor interact: hit face event_id={} at dist={:.0}", event_id, dist);
        let Some(me) = map_events else {
            warn!("Indoor interact: no MapEvents resource");
            return;
        };
        let Some(evt) = me.evt.as_ref() else {
            warn!("Indoor interact: no EVT file loaded");
            return;
        };
        if let Some(steps) = evt.events.get(&event_id) {
            info!(
                "Indoor interact: dispatching {} steps for event_id={}",
                steps.len(),
                event_id
            );
        } else {
            info!("Indoor interact: no actions found for event_id={}", event_id);
        }
        event_queue.push_all(event_id, evt);
    }
}

/// Check player proximity to touch-triggered faces and dispatch events.
pub(crate) fn indoor_touch_trigger_system(
    player_query: Query<&Transform, With<crate::game::player::Player>>,
    mut touch_triggers: Option<ResMut<TouchTriggerFaces>>,
    map_events: Option<Res<MapEvents>>,
    mut event_queue: ResMut<EventQueue>,
) {
    let Some(ref mut triggers) = touch_triggers else { return };
    if triggers.faces.is_empty() {
        return;
    }
    let Ok(player_tf) = player_query.single() else { return };
    let player_pos = player_tf.translation;

    // Collect events to fire (avoids borrow conflict with fired set)
    let to_fire: Vec<u16> = triggers
        .faces
        .iter()
        .filter(|f| !triggers.fired.contains(&f.event_id))
        .filter(|f| player_pos.distance(f.center) <= f.radius)
        .map(|f| f.event_id)
        .collect();

    if let Some(me) = map_events.as_ref()
        && let Some(evt) = me.evt.as_ref()
    {
        for eid in &to_fire {
            info!("Touch trigger: event_id={}", eid);
            event_queue.push_all(*eid, evt);
        }
    }
    for eid in to_fire {
        triggers.fired.insert(eid);
    }

    // Reset fired events when player moves away (allows re-triggering)
    let still_near: std::collections::HashSet<u16> = triggers
        .faces
        .iter()
        .filter(|f| triggers.fired.contains(&f.event_id) && player_pos.distance(f.center) <= f.radius * 1.5)
        .map(|f| f.event_id)
        .collect();
    triggers.fired = still_near;
}
