use super::common::*;
use crate::config::GameConfig;
use crate::game::player::Player;
use crate::prepare::loading::PreparedWorld;
use bevy::prelude::*;

pub fn update_map_info_text(
    throttle: Res<HudThrottle>,
    world_state: Res<crate::game::state::WorldState>,
    mut query: Query<&mut Text, With<MapNameSpan>>,
) {
    if !throttle.0.just_finished() {
        return;
    }

    let map_name = world_state.map.name.to_string().to_uppercase();
    let map_str = format!("  {}", map_name);

    for mut text in &mut query {
        if **text != map_str {
            **text = map_str.clone();
        }
    }
}

pub fn update_player_mode_text(
    throttle: Res<HudThrottle>,
    world_state: Res<crate::game::state::WorldState>,
    mut query: Query<&mut Text, With<ModeSpan>>,
) {
    if !throttle.0.just_finished() {
        return;
    }

    let mode_str = if world_state.player.fly_mode {
        "  FLY"
    } else if world_state.player.is_running {
        "  RUN"
    } else {
        "  WALK"
    };

    for mut text in &mut query {
        if **text != mode_str {
            **text = mode_str.to_string();
        }
    }
}

pub fn update_position_text(
    throttle: Res<HudThrottle>,
    cfg: Res<GameConfig>,
    _world_state: Res<crate::game::state::WorldState>,
    spawn_progress: Res<crate::game::outdoor::SpawnProgress>,
    player_query: Query<&Transform, With<Player>>,
    mut query: Query<&mut Text, With<PosSpan>>,
) {
    if !throttle.0.just_finished() {
        return;
    }

    let coords_str = if let Some(transform) = player_query.iter().next() {
        let (yaw, _, _): (f32, f32, f32) = transform.rotation.to_euler(EulerRot::YXZ);
        let spawn_str = if cfg.debug && spawn_progress.total > 0 && spawn_progress.done < spawn_progress.total {
            format!("  SPAWN: {}/{}", spawn_progress.done, spawn_progress.total)
        } else {
            String::new()
        };
        format!(
            "  X:{:.0}  Y:{:.0}  Z:{:.0}  YAW:{:.0}deg{}",
            transform.translation.x,
            transform.translation.y,
            transform.translation.z,
            yaw.to_degrees(),
            spawn_str,
        )
    } else {
        "  POS: --".to_string()
    };

    for mut text in &mut query {
        if **text != coords_str {
            **text = coords_str.clone();
        }
    }
}

pub fn update_tile_text(
    throttle: Res<HudThrottle>,
    player_query: Query<&Transform, With<Player>>,
    prepared: Res<PreparedWorld>,
    mut query: Query<(&mut Text, &mut TextColor), With<TileSpan>>,
) {
    if !throttle.0.just_finished() {
        return;
    }

    let tileset = player_query
        .iter()
        .next()
        .and_then(|tf| prepared.terrain_at(tf.translation.x, tf.translation.z));

    let tileset_str = if let Some(ts) = tileset {
        format!("  {ts}")
    } else {
        String::new()
    };

    for (mut text, mut tc) in &mut query {
        if **text != tileset_str {
            **text = tileset_str.clone();
            if let Some(ts) = tileset {
                *tc = TextColor(tileset_color(ts));
            }
        }
    }
}
