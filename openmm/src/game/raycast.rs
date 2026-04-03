use bevy::prelude::*;

use crate::game::events::MapEvents;

/// Resolve a human-readable label for an event ID from its EVT steps.
/// Returns the first non-empty text found, scanning steps in order.
/// Recognised events: Hint, StatusText, LocationName, SpeakInHouse, OpenChest, MoveToMap.
pub fn resolve_event_name_from_evt(event_id: u16, evt: &lod::evt::EvtFile) -> Option<String> {
    let steps = evt.events.get(&event_id)?;
    for s in steps {
        let text = match &s.event {
            lod::evt::GameEvent::Hint { text, .. } if !text.is_empty() => text.clone(),
            lod::evt::GameEvent::StatusText { text, .. } if !text.is_empty() => text.clone(),
            lod::evt::GameEvent::LocationName { text, .. } if !text.is_empty() => text.clone(),
            lod::evt::GameEvent::SpeakInHouse { house_id } => format!("Building #{}", house_id),
            lod::evt::GameEvent::OpenChest { id } => format!("Chest #{}", id),
            lod::evt::GameEvent::MoveToMap { map_name, .. } => format!("Enter {}", map_name),
            _ => continue,
        };
        return Some(text);
    }
    None
}

/// Resolve a label for an event from the map's loaded event table.
/// For SpeakInHouse, looks up the house name from the loaded house table first.
/// Returns `None` when no map events are loaded or no matching event exists.
pub fn resolve_event_name(event_id: u16, map_events: &Option<Res<MapEvents>>) -> Option<String> {
    let me = map_events.as_ref()?;
    let evt = me.evt.as_ref()?;

    // For SpeakInHouse, prefer the loaded house name over the generic "Building #N"
    if let Some(steps) = evt.events.get(&event_id) {
        for s in steps {
            if let lod::evt::GameEvent::SpeakInHouse { house_id } = &s.event {
                if let Some(houses) = me.houses.as_ref()
                    && let Some(entry) = houses.houses.get(house_id)
                {
                    return Some(entry.name.clone());
                }
                return Some(format!("Building #{}", house_id));
            }
        }
    }

    resolve_event_name_from_evt(event_id, evt)
}

/// Ray-plane intersection. Returns distance `t` along ray if hit (positive = in front).
pub fn ray_plane_intersect(origin: Vec3, dir: Vec3, normal: Vec3, plane_dist: f32) -> Option<f32> {
    let denom = normal.dot(dir);
    if denom.abs() < 1e-6 {
        return None;
    }
    let t = (plane_dist - normal.dot(origin)) / denom;
    if t > 0.0 { Some(t) } else { None }
}

/// Test if a 3D point lies inside a convex/concave polygon using winding number.
/// All points assumed coplanar. Projects to the best 2D plane based on normal.
pub fn point_in_polygon(point: Vec3, vertices: &[Vec3], normal: Vec3) -> bool {
    if vertices.len() < 3 {
        return false;
    }
    let abs_n = normal.abs();
    let (ax1, ax2) = if abs_n.x >= abs_n.y && abs_n.x >= abs_n.z {
        (1usize, 2usize)
    } else if abs_n.y >= abs_n.z {
        (0, 2)
    } else {
        (0, 1)
    };
    let get = |v: Vec3, axis: usize| -> f32 {
        match axis { 0 => v.x, 1 => v.y, _ => v.z }
    };
    let px = get(point, ax1);
    let py = get(point, ax2);
    let mut winding = 0i32;
    let n = vertices.len();
    for i in 0..n {
        let v1 = vertices[i];
        let v2 = vertices[(i + 1) % n];
        let y1 = get(v1, ax2);
        let y2 = get(v2, ax2);
        if y1 <= py {
            if y2 > py {
                let x1 = get(v1, ax1);
                let x2 = get(v2, ax1);
                if (x2 - x1) * (py - y1) - (px - x1) * (y2 - y1) > 0.0 {
                    winding += 1;
                }
            }
        } else if y2 <= py {
            let x1 = get(v1, ax1);
            let x2 = get(v2, ax1);
            if (x2 - x1) * (py - y1) - (px - x1) * (y2 - y1) < 0.0 {
                winding -= 1;
            }
        }
    }
    winding != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ray_plane_hit() {
        let t = ray_plane_intersect(Vec3::new(0.0, 5.0, 0.0), Vec3::NEG_Y, Vec3::Y, 0.0);
        assert!((t.unwrap() - 5.0).abs() < 1e-4);
    }

    #[test]
    fn ray_plane_parallel_miss() {
        let t = ray_plane_intersect(Vec3::new(0.0, 1.0, 0.0), Vec3::X, Vec3::Y, 0.0);
        assert!(t.is_none());
    }

    #[test]
    fn ray_plane_behind_miss() {
        let t = ray_plane_intersect(Vec3::new(0.0, -1.0, 0.0), Vec3::NEG_Y, Vec3::Y, 0.0);
        assert!(t.is_none());
    }

    #[test]
    fn resolve_event_name_first_match_wins() {
        use std::collections::HashMap;
        use lod::evt::{EvtFile, EvtStep, GameEvent};
        let mut events: HashMap<u16, Vec<EvtStep>> = HashMap::new();
        events.insert(1, vec![
            EvtStep { step: 0, event: GameEvent::StatusText { str_id: 0, text: "status".into() } },
            EvtStep { step: 1, event: GameEvent::Hint { str_id: 0, text: "hint".into() } },
        ]);
        let evt = EvtFile { events };
        // First non-empty match wins in step order
        assert_eq!(resolve_event_name_from_evt(1, &evt), Some("status".to_string()));
    }

    #[test]
    fn resolve_event_name_hint_only() {
        use std::collections::HashMap;
        use lod::evt::{EvtFile, EvtStep, GameEvent};
        let mut events: HashMap<u16, Vec<EvtStep>> = HashMap::new();
        events.insert(2, vec![
            EvtStep { step: 0, event: GameEvent::Hint { str_id: 0, text: "hint".into() } },
        ]);
        let evt = EvtFile { events };
        assert_eq!(resolve_event_name_from_evt(2, &evt), Some("hint".to_string()));
    }

    #[test]
    fn resolve_event_name_empty_text_skipped() {
        use std::collections::HashMap;
        use lod::evt::{EvtFile, EvtStep, GameEvent};
        let mut events: HashMap<u16, Vec<EvtStep>> = HashMap::new();
        events.insert(3, vec![
            EvtStep { step: 0, event: GameEvent::Hint { str_id: 0, text: "".into() } },
            EvtStep { step: 1, event: GameEvent::StatusText { str_id: 0, text: "real".into() } },
        ]);
        let evt = EvtFile { events };
        assert_eq!(resolve_event_name_from_evt(3, &evt), Some("real".to_string()));
    }

    #[test]
    fn point_in_square_polygon() {
        let verts = vec![
            Vec3::new(-1.0, 0.0, -1.0),
            Vec3::new( 1.0, 0.0, -1.0),
            Vec3::new( 1.0, 0.0,  1.0),
            Vec3::new(-1.0, 0.0,  1.0),
        ];
        assert!(point_in_polygon(Vec3::new(0.0, 0.0, 0.0), &verts, Vec3::Y));
        assert!(!point_in_polygon(Vec3::new(2.0, 0.0, 0.0), &verts, Vec3::Y));
    }
}
