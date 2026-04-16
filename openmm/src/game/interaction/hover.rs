use bevy::prelude::*;

use crate::game::world::ui_state::UiState;
use crate::game::indoor::OccluderFaces;
use crate::game::player::PlayerCamera;
use crate::game::spatial_index::EntitySpatialIndex;
use crate::game::sprites::loading::SpriteSheet;
use crate::game::world::MapEvents;

use super::clickable;
use super::raycast::{billboard_hit_test, point_in_polygon, ray_plane_intersect, resolve_event_name};
use super::{DecorationInfo, MAX_INTERACT_RANGE, MonsterInteractable, NpcInteractable, facing_rotation};
use crate::config::GameConfig;

/// Show the nearest interactive object's name in the footer — pixel-accurate for all types.
pub(crate) fn hover_hint_system(
    camera_query: Query<(&GlobalTransform, &Camera), With<PlayerCamera>>,
    clickable_faces: Option<Res<clickable::Faces>>,
    mut occluder_faces: Option<ResMut<OccluderFaces>>,
    decorations: Query<(&DecorationInfo, &GlobalTransform, Option<&SpriteSheet>)>,
    npcs: Query<(&NpcInteractable, &GlobalTransform, &SpriteSheet)>,
    monsters: Query<(&MonsterInteractable, &GlobalTransform, &SpriteSheet)>,
    spatial: Res<EntitySpatialIndex>,
    map_events: Option<Res<MapEvents>>,
    mut footer: ResMut<UiState>,
    ui_hovered: Option<Res<crate::screens::runtime::ScreenUiHovered>>,
    cfg: Res<GameConfig>,
    #[cfg(feature = "perf_log")] mut perf: ResMut<crate::game::debug::perf_log::PerfCounters>,
) {
    #[cfg(feature = "perf_log")]
    let _start = crate::game::debug::perf_log::perf_start();

    let ui_hovered = ui_hovered.as_ref().map(|r| r.0).unwrap_or(false);
    let Ok((cam_global, _)) = camera_query.single() else {
        return;
    };
    let origin = cam_global.translation();
    let dir = cam_global.forward().as_vec3();
    let occluder_t = occluder_faces
        .as_mut()
        .map(|of| of.min_hit_t_max(origin, dir, cfg.draw_distance))
        .unwrap_or(f32::MAX);

    let mut nearest: Option<(f32, String)> = None;

    // BSP faces (outdoor buildings) — capped to same arm's reach as billboards.
    if let Some(faces) = clickable_faces.as_ref() {
        for face in &faces.faces {
            #[cfg(feature = "perf_log")]
            {
                perf.hover_face_tests += 1;
            }
            if let Some(t) = ray_plane_intersect(origin, dir, face.normal, face.plane_dist) {
                if t > MAX_INTERACT_RANGE {
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

    let max_range_sq = MAX_INTERACT_RANGE * MAX_INTERACT_RANGE;

    // Billboard entities — spatial index narrows to the interact cell block,
    // then the same per-entity hit test runs as before.
    for entity in spatial.query_radius(origin.x, origin.z, MAX_INTERACT_RANGE) {
        #[cfg(feature = "perf_log")]
        {
            perf.hover_candidates += 1;
        }
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
                && (nearest.is_none() || t < nearest.as_ref().unwrap().0)
            {
                if info.event_id > 0 {
                    let name = resolve_event_name(info.event_id, &map_events)
                        .or_else(|| info.display_name.clone());

                    if let Some(name) = name {
                        nearest = Some((t, name));
                    }
                }
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
                && (nearest.is_none() || t < nearest.as_ref().unwrap().0)
            {
                nearest = Some((t, info.name.clone()));
            }
            continue;
        }

        if let Ok((info, g_tf, sheet)) = monsters.get(entity) {
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
                && (nearest.is_none() || t < nearest.as_ref().unwrap().0)
            {
                nearest = Some((t, info.name.clone()));
            }
        }
    }

    match nearest {
        Some((_, name)) => footer.footer.set(&name),
        // Don't clear footer when a screen UI element is being hovered —
        // the screen system owns the hint text in that case.
        None => {
            if !ui_hovered {
                footer.footer.clear();
            }
        }
    }

    #[cfg(feature = "perf_log")]
    {
        perf.time_hover_hint_us += crate::game::debug::perf_log::perf_elapsed_us(_start);
    }
}
