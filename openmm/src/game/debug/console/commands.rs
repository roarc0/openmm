//! Per-command handlers for the developer console.
//!
//! Each handler is a free function taking only the state it actually needs.
//! Dispatch is performed by the `execute_command` match in `super`.

use bevy::pbr::wireframe::WireframeConfig;
use bevy::prelude::*;

use super::{ConsoleState, HELP_TEXT, NEEDS_RELOAD, parse_coords, parse_toggle, resolve_direction, set_filtering};
use crate::GameState;
use crate::config::GameConfig;
use crate::game::outdoor::{OdmName, PLAY_WIDTH};
use crate::game::player::SpeedMultiplier;
use crate::game::world::{GameTime, WorldState};
use crate::save::GameSave;
use crate::states::loading::LoadRequest;
use openmm_data::utils::MapName;

// --- Map loading ---

pub(super) fn cmd_reload(
    state: &mut ConsoleState,
    world: &mut WorldState,
    save_data: &mut GameSave,
    commands: &mut Commands,
    game_state: &mut NextState<GameState>,
) {
    let target = world.map.name.clone();
    world.write_to_save(save_data);
    state.push_output(format!("Reloading map: {}", target));
    state.open = false;
    commands.insert_resource(LoadRequest {
        map_name: target,
        spawn_position: None,
        spawn_yaw: None,
    });
    game_state.set(GameState::Loading);
}

pub(super) fn cmd_load(
    state: &mut ConsoleState,
    parts: &[&str],
    world: &mut WorldState,
    save_data: &mut GameSave,
    commands: &mut Commands,
    game_state: &mut NextState<GameState>,
    game_assets: &crate::GameAssets,
) {
    let arg = parts.get(1).copied().unwrap_or("");
    if arg.is_empty() {
        state.push_output("Usage: load <map|north|south|east|west> [x,z]".to_string());
        return;
    }
    let resolved = match arg {
        "north" | "n" => resolve_direction(
            &world.map.name,
            world.player.position,
            OdmName::go_north,
            0.0,
            PLAY_WIDTH,
        ),
        "south" | "s" => resolve_direction(
            &world.map.name,
            world.player.position,
            OdmName::go_south,
            0.0,
            -PLAY_WIDTH,
        ),
        "east" | "e" => resolve_direction(
            &world.map.name,
            world.player.position,
            OdmName::go_east,
            -PLAY_WIDTH,
            0.0,
        ),
        "west" | "w" => resolve_direction(
            &world.map.name,
            world.player.position,
            OdmName::go_west,
            PLAY_WIDTH,
            0.0,
        ),
        name => match MapName::try_from(name) {
            Ok(target) => {
                let filename = target.filename();
                let lod_path = format!("games/{}", filename);
                if game_assets.assets().get_bytes(&lod_path).is_err() {
                    Err(format!("Map not found: {}", filename))
                } else {
                    let pos = parts.get(2).and_then(|c| parse_coords(c));
                    Ok((target, pos.unwrap_or([0.0, 0.0, 0.0])))
                }
            }
            Err(e) => Err(format!("Invalid map name '{}': {}", name, e)),
        },
    };
    match resolved {
        Ok((target, pos)) => {
            state.push_output(format!("Loading map: {} at ({:.0}, {:.0})", target, pos[0], pos[2]));
            state.open = false;
            save_data.player.position = pos;
            if let MapName::Outdoor(ref odm) = target {
                save_data.map.map_x = odm.x;
                save_data.map.map_y = odm.y;
            }
            commands.insert_resource(LoadRequest {
                map_name: target.clone(),
                spawn_position: None,
                spawn_yaw: None,
            });
            world.map.name = target;
            game_state.set(GameState::Loading);
        }
        Err(msg) => state.push_output(msg),
    }
}

// --- Graphics ---

pub(super) fn cmd_msaa(state: &mut ConsoleState, cfg: &mut GameConfig, arg: &str) {
    match arg {
        "msaa2" | "msaa4" | "msaa8" | "fxaa" | "smaa" | "taa" | "off" => {
            cfg.antialiasing = arg.to_string();
            state.push_output(format!("Antialiasing: {} {NEEDS_RELOAD}", arg));
        }
        _ => {
            state.push_output(format!("Current: {}", cfg.antialiasing));
            state.push_output("Usage: aa <msaa2|msaa4|msaa8|fxaa|smaa|taa|off>".to_string());
        }
    }
}

pub(super) fn cmd_tonemap(state: &mut ConsoleState, cfg: &mut GameConfig, arg: &str) {
    match arg {
        "none" | "reinhard" | "aces" | "agx" | "blender_filmic" => {
            cfg.tonemapping = arg.to_string();
            state.push_output(format!("Tonemapping: {} {NEEDS_RELOAD}", arg));
        }
        _ => {
            state.push_output(format!("Current: {}", cfg.tonemapping));
            state.push_output("Usage: tonemap <none|reinhard|aces|agx|blender_filmic>".to_string());
        }
    }
}

pub(super) fn cmd_wireframe(state: &mut ConsoleState, wireframe: &mut WireframeConfig) {
    wireframe.global = !wireframe.global;
    state.push_output(format!("Wireframe: {}", if wireframe.global { "on" } else { "off" }));
}

pub(super) fn cmd_shadows(state: &mut ConsoleState, cfg: &mut GameConfig, arg: &str) {
    cfg.shadows = parse_toggle(arg, cfg.shadows);
    state.push_output(format!("Shadows: {}", if cfg.shadows { "on" } else { "off" }));
}

pub(super) fn cmd_speed(state: &mut ConsoleState, speed_mul: &mut SpeedMultiplier, arg: &str) {
    if arg.is_empty() {
        state.push_output(format!(
            "Speed: {:.2}x (usage: speed <multiplier>, e.g. 1.2, 2, 4)",
            speed_mul.0
        ));
    } else if let Ok(v) = arg.parse::<f32>() {
        if v > 0.0 {
            speed_mul.0 = v;
            state.push_output(format!("Speed: {:.2}x", v));
        } else {
            state.push_output("Speed must be > 0".to_string());
        }
    } else {
        state.push_output("Usage: speed <multiplier>".to_string());
    }
}

pub(super) fn cmd_bloom(state: &mut ConsoleState, cfg: &mut GameConfig, arg: &str) {
    match arg {
        "on" | "1" => cfg.bloom = true,
        "off" | "0" => cfg.bloom = false,
        "" => cfg.bloom = !cfg.bloom,
        intensity => {
            if let Ok(v) = intensity.parse::<f32>() {
                cfg.bloom = true;
                cfg.bloom_intensity = v.clamp(0.0, 1.0);
            } else {
                state.push_output("Usage: bloom [on|off|0.0-1.0]".to_string());
                return;
            }
        }
    }
    state.push_output(format!(
        "Bloom: {} (intensity: {:.2}) {NEEDS_RELOAD}",
        if cfg.bloom { "on" } else { "off" },
        cfg.bloom_intensity
    ));
}

pub(super) fn cmd_ssao(state: &mut ConsoleState, cfg: &mut GameConfig, arg: &str) {
    cfg.ssao = parse_toggle(arg, cfg.ssao);
    state.push_output(format!("SSAO: {} {NEEDS_RELOAD}", if cfg.ssao { "on" } else { "off" }));
}

pub(super) fn cmd_fog(state: &mut ConsoleState, cfg: &mut GameConfig, parts: &[&str]) {
    let arg = parts.get(1).copied().unwrap_or("");
    if arg.is_empty() {
        state.push_output(format!("Fog start: {:.0}, end: {:.0}", cfg.fog_start, cfg.fog_end));
    } else if let (Ok(start), Some(Ok(end))) = (arg.parse::<f32>(), parts.get(2).map(|s| s.parse::<f32>())) {
        cfg.fog_start = start;
        cfg.fog_end = end;
        state.push_output(format!("Fog: {:.0} - {:.0}", start, end));
    } else {
        state.push_output("Usage: fog <start> <end>".to_string());
    }
}

pub(super) fn cmd_draw_distance(state: &mut ConsoleState, cfg: &mut GameConfig, arg: &str) {
    if arg.is_empty() {
        state.push_output(format!("Draw distance: {:.0}", cfg.draw_distance));
    } else if let Ok(v) = arg.parse::<f32>() {
        cfg.draw_distance = v;
        state.push_output(format!("Draw distance: {:.0}", v));
    } else {
        state.push_output("Usage: dd <distance>".to_string());
    }
}

pub(super) fn cmd_exposure(state: &mut ConsoleState, cfg: &mut GameConfig, arg: &str) {
    if arg.is_empty() {
        state.push_output(format!("Exposure: {:.2}", cfg.exposure));
    } else if let Ok(v) = arg.parse::<f32>() {
        cfg.exposure = v.clamp(-4.0, 4.0);
        state.push_output(format!("Exposure: {:.2}", cfg.exposure));
    } else {
        state.push_output("Usage: exposure <-4.0 to 4.0>".to_string());
    }
}

pub(super) fn cmd_dof(state: &mut ConsoleState, cfg: &mut GameConfig, arg: &str) {
    match arg {
        "off" | "0" => {
            cfg.depth_of_field = false;
            state.push_output("Depth of field: off".to_string());
        }
        "on" | "1" => {
            cfg.depth_of_field = true;
            state.push_output(format!(
                "Depth of field: on (distance: {:.1})",
                cfg.depth_of_field_distance
            ));
        }
        "" => {
            state.push_output(format!(
                "Depth of field: {} (distance: {:.1})",
                if cfg.depth_of_field { "on" } else { "off" },
                cfg.depth_of_field_distance
            ));
        }
        dist => {
            if let Ok(v) = dist.parse::<f32>() {
                cfg.depth_of_field = true;
                cfg.depth_of_field_distance = v.max(0.1);
                state.push_output(format!(
                    "Depth of field: on (distance: {:.1})",
                    cfg.depth_of_field_distance
                ));
            } else {
                state.push_output("Usage: dof [on|off|<distance>]".to_string());
            }
        }
    }
}

// --- Inventory / quest bits ---

pub(super) fn cmd_item(state: &mut ConsoleState, world: &mut WorldState, parts: &[&str]) {
    let sub = parts.get(1).copied().unwrap_or("");
    let item_id: Option<i32> = parts.get(2).and_then(|s| s.parse().ok());
    let count: i32 = parts.get(3).and_then(|s| s.parse().ok()).unwrap_or(1);
    match (sub, item_id) {
        ("add", Some(id)) => {
            world.game_vars.give_item(id, count);
            state.push_output(format!("Item {}: count now {}", id, world.game_vars.item_count(id)));
        }
        ("rem", Some(id)) => {
            world.game_vars.remove_item(id, count);
            state.push_output(format!("Item {}: count now {}", id, world.game_vars.item_count(id)));
        }
        _ => state.push_output("Usage: item add|rem <id> [count]".to_string()),
    }
}

pub(super) fn cmd_qbit(state: &mut ConsoleState, world: &mut WorldState, parts: &[&str]) {
    let bit: Option<i32> = parts.get(1).and_then(|s| s.parse().ok());
    let value = parts.get(2).copied();
    match bit {
        None => state.push_output("Usage: qbit <n> [true|false]".to_string()),
        Some(n) => match value {
            None => {
                let s = if world.game_vars.has_qbit(n) { "set" } else { "not set" };
                state.push_output(format!("QBit {}: {}", n, s));
            }
            Some("true" | "1" | "on") => {
                world.game_vars.set_qbit(n);
                state.push_output(format!("QBit {} set", n));
            }
            Some("false" | "0" | "off") => {
                world.game_vars.clear_qbit(n);
                state.push_output(format!("QBit {} cleared", n));
            }
            Some(v) => state.push_output(format!("Unknown value '{}'; use true or false", v)),
        },
    }
}

// --- Gameplay ---

pub(super) fn cmd_fly(state: &mut ConsoleState, world: &mut WorldState, arg: &str) {
    world.player.fly_mode = parse_toggle(arg, world.player.fly_mode);
    state.push_output(format!(
        "Fly mode: {}",
        if world.player.fly_mode { "on" } else { "off" }
    ));
}

pub(super) fn cmd_turn_speed(state: &mut ConsoleState, cfg: &mut GameConfig, arg: &str) {
    if arg.is_empty() {
        state.push_output(format!("Turn speed: {:.0}", cfg.turn_speed));
    } else if let Ok(v) = arg.parse::<f32>() {
        cfg.turn_speed = v;
        state.push_output(format!("Turn speed: {:.0}", v));
    } else {
        state.push_output("Usage: turn_speed <deg/sec>".to_string());
    }
}

pub(super) fn cmd_sensitivity(state: &mut ConsoleState, cfg: &mut GameConfig, arg: &str) {
    if arg.is_empty() {
        state.push_output(format!(
            "Mouse sensitivity: {:.2}x {:.2}y",
            cfg.mouse_sensitivity_x, cfg.mouse_sensitivity_y
        ));
    } else if let Ok(v) = arg.parse::<f32>() {
        cfg.mouse_sensitivity_x = v;
        cfg.mouse_sensitivity_y = v;
        state.push_output(format!("Mouse sensitivity: {:.2}", v));
    } else {
        state.push_output("Usage: sens <value>".to_string());
    }
}

pub(super) fn cmd_pos(state: &mut ConsoleState, world: &WorldState) {
    let p = world.player.position;
    // MM6: X right, Y forward, Z up. Bevy: X right, Y up, Z = -Y_mm6.
    let mm6_x = p.x as i32;
    let mm6_y = (-p.z) as i32;
    let mm6_z = p.y as i32;
    state.push_output(format!("MM6:  x={} y={} z={}", mm6_x, mm6_y, mm6_z));
    state.push_output(format!("Bevy: x={:.1} y={:.1} z={:.1}", p.x, p.y, p.z));
    state.push_output(format!("Map:  {}", world.map.name));
}

// --- Window ---

pub(super) fn cmd_fullscreen(state: &mut ConsoleState, cfg: &mut GameConfig) {
    cfg.window_mode = "fullscreen".into();
    state.push_output("Fullscreen".to_string());
}

pub(super) fn cmd_borderless(state: &mut ConsoleState, cfg: &mut GameConfig) {
    cfg.window_mode = "borderless".into();
    state.push_output("Borderless fullscreen".to_string());
}

pub(super) fn cmd_windowed(state: &mut ConsoleState, cfg: &mut GameConfig) {
    cfg.window_mode = "windowed".into();
    state.push_output("Windowed mode".to_string());
}

pub(super) fn cmd_aspect(state: &mut ConsoleState, cfg: &mut GameConfig, arg: &str) {
    if arg.is_empty() {
        let display = if cfg.aspect_ratio.is_empty() {
            "auto"
        } else {
            &cfg.aspect_ratio
        };
        state.push_output(format!("Aspect ratio: {}", display));
    } else if arg == "auto" {
        cfg.aspect_ratio = "".into();
        state.push_output("Aspect ratio: auto (uses window size)".to_string());
    } else if arg.contains(':') && crate::game::rendering::viewport::parse_aspect_ratio(arg).is_some() {
        cfg.aspect_ratio = arg.to_string();
        state.push_output(format!("Aspect ratio: {}", arg));
    } else {
        state.push_output("Usage: aspect <auto|4:3|16:9|21:9>".to_string());
    }
}

pub(super) fn cmd_vsync(state: &mut ConsoleState, cfg: &mut GameConfig, arg: &str) {
    match arg {
        "on" | "auto" => cfg.vsync = "auto".into(),
        "fast" => cfg.vsync = "fast".into(),
        "off" | "0" => cfg.vsync = "off".into(),
        _ => {
            state.push_output(format!("Current: {}", cfg.vsync));
            state.push_output("Usage: vsync <auto|fast|off>".to_string());
            return;
        }
    }
    state.push_output(format!("VSync: {}", cfg.vsync));
}

pub(super) fn cmd_fps_cap(state: &mut ConsoleState, cfg: &mut GameConfig, arg: &str) {
    if arg.is_empty() {
        let cap = if cfg.fps_cap == 0 {
            "unlimited".to_string()
        } else {
            format!("{}", cfg.fps_cap)
        };
        state.push_output(format!("FPS cap: {}", cap));
    } else if let Ok(v) = arg.parse::<u32>() {
        cfg.fps_cap = v;
        let cap = if v == 0 {
            "unlimited".to_string()
        } else {
            format!("{}", v)
        };
        state.push_output(format!("FPS cap: {}", cap));
    } else {
        state.push_output("Usage: fps_cap <0=unlimited|30|60|120|...>".to_string());
    }
}

pub(super) fn cmd_render_scale(state: &mut ConsoleState, cfg: &mut GameConfig, arg: &str) {
    if arg.is_empty() {
        state.push_output(format!("Render scale: {:.2}", cfg.render_scale));
    } else if let Ok(v) = arg.parse::<f32>() {
        cfg.render_scale = v.clamp(0.1, 1.0);
        state.push_output(format!("Render scale: {:.2}", cfg.render_scale));
    } else {
        state.push_output("Usage: render_scale <0.25|0.5|0.75|1.0>".to_string());
    }
}

// --- Audio ---

pub(super) fn cmd_mute(state: &mut ConsoleState, cfg: &mut GameConfig) {
    cfg.music_volume = 0.0;
    cfg.sfx_volume = 0.0;
    state.push_output("All audio muted".to_string());
}

pub(super) fn cmd_unmute(state: &mut ConsoleState, cfg: &mut GameConfig) {
    if cfg.music_volume == 0.0 {
        cfg.music_volume = 0.5;
    }
    if cfg.sfx_volume == 0.0 {
        cfg.sfx_volume = 1.0;
    }
    state.push_output(format!(
        "Audio unmuted (music: {:.0}%, sfx: {:.0}%)",
        cfg.music_volume * 100.0,
        cfg.sfx_volume * 100.0
    ));
}

fn parse_volume(v: f32) -> f32 {
    if v > 1.0 {
        (v / 100.0).clamp(0.0, 1.0)
    } else {
        v.clamp(0.0, 1.0)
    }
}

pub(super) fn cmd_music(state: &mut ConsoleState, cfg: &mut GameConfig, arg: &str) {
    if arg.is_empty() {
        state.push_output(format!("Music volume: {:.0}%", cfg.music_volume * 100.0));
    } else if let Ok(v) = arg.parse::<f32>() {
        cfg.music_volume = parse_volume(v);
        state.push_output(format!("Music volume: {:.0}%", cfg.music_volume * 100.0));
    } else {
        state.push_output("Usage: music <0-100>".to_string());
    }
}

pub(super) fn cmd_sfx(state: &mut ConsoleState, cfg: &mut GameConfig, arg: &str) {
    if arg.is_empty() {
        state.push_output(format!("SFX volume: {:.0}%", cfg.sfx_volume * 100.0));
    } else if let Ok(v) = arg.parse::<f32>() {
        cfg.sfx_volume = parse_volume(v);
        state.push_output(format!("SFX volume: {:.0}%", cfg.sfx_volume * 100.0));
    } else {
        state.push_output("Usage: sfx <0-100>".to_string());
    }
}

pub(super) fn cmd_volume(state: &mut ConsoleState, cfg: &mut GameConfig, arg: &str) {
    if arg.is_empty() {
        state.push_output(format!(
            "Music: {:.0}%, SFX: {:.0}%",
            cfg.music_volume * 100.0,
            cfg.sfx_volume * 100.0
        ));
    } else if let Ok(v) = arg.parse::<f32>() {
        let vol = parse_volume(v);
        cfg.music_volume = vol;
        cfg.sfx_volume = vol;
        state.push_output(format!("All volume: {:.0}%", vol * 100.0));
    } else {
        state.push_output("Usage: volume <0-100>".to_string());
    }
}

// --- System ---

pub(super) fn cmd_debug(state: &mut ConsoleState, world: &mut WorldState, cfg: &mut GameConfig, arg: &str) {
    cfg.debug = parse_toggle(arg, cfg.debug);
    world.debug.show_play_area = cfg.debug;
    world.debug.show_events = cfg.debug;
    state.push_output(format!("Debug HUD: {}", if cfg.debug { "on" } else { "off" }));
}

pub(super) fn cmd_lighting(state: &mut ConsoleState, cfg: &mut GameConfig, arg: &str) {
    if arg.is_empty() {
        state.push_output(format!("Lighting: {}", cfg.lighting));
    } else {
        match arg {
            "classic" | "enhanced" => {
                cfg.lighting = arg.to_string();
                state.push_output(format!("Lighting: {}", cfg.lighting));
            }
            _ => state.push_output("Usage: lighting [classic|enhanced]".to_string()),
        }
    }
}

pub(super) fn cmd_filtering(state: &mut ConsoleState, cfg: &mut GameConfig, arg: &str) {
    if arg.is_empty() {
        state.push_output(format!(
            "terrain={} models={} hud={}",
            cfg.terrain_filtering, cfg.models_filtering, cfg.hud_filtering
        ));
    } else {
        match arg {
            "nearest" | "linear" => {
                cfg.terrain_filtering = arg.to_string();
                cfg.models_filtering = arg.to_string();
                cfg.hud_filtering = arg.to_string();
                state.push_output(format!("All filtering: {} {NEEDS_RELOAD}", arg));
            }
            _ => state.push_output("Usage: filtering [nearest|linear]".to_string()),
        }
    }
}

pub(super) fn cmd_terrain_filtering(state: &mut ConsoleState, cfg: &mut GameConfig, arg: &str) {
    set_filtering(state, &mut cfg.terrain_filtering, "Terrain", arg);
}

pub(super) fn cmd_models_filtering(state: &mut ConsoleState, cfg: &mut GameConfig, arg: &str) {
    set_filtering(state, &mut cfg.models_filtering, "Models", arg);
}

pub(super) fn cmd_hud_filtering(state: &mut ConsoleState, cfg: &mut GameConfig, arg: &str) {
    set_filtering(state, &mut cfg.hud_filtering, "HUD", arg);
}

pub(super) fn cmd_exit(exit: &mut MessageWriter<AppExit>) {
    exit.write(AppExit::from_code(0));
}

pub(super) fn cmd_clear(state: &mut ConsoleState) {
    state.output.clear();
    state.generation += 1;
}

pub(super) fn cmd_save_cfg(state: &mut ConsoleState, cfg: &GameConfig) {
    match cfg.save() {
        Ok(()) => state.push_output(format!("Config saved to {}", cfg.config_path.display())),
        Err(e) => state.push_output(e),
    }
}

pub(super) fn cmd_time(state: &mut ConsoleState, game_time: &mut GameTime, parts: &[&str]) {
    let arg = parts.get(1).copied().unwrap_or("");
    match arg {
        "stop" | "pause" => {
            game_time.set_paused(true);
            state.push_output(format!("Time paused at {}", game_time.format_datetime()));
        }
        "start" | "resume" => {
            game_time.set_paused(false);
            state.push_output("Time resumed".to_string());
        }
        "add" => {
            let hours: f32 = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0.0);
            if hours == 0.0 {
                state.push_output("Usage: time add <hours>".to_string());
            } else {
                game_time.advance_hours(hours);
                state.push_output(format!("Advanced {hours}h → {}", game_time.format_datetime()));
            }
        }
        "" => {
            let status = if game_time.is_paused() { " (paused)" } else { "" };
            state.push_output(format!("{}{}", game_time.format_datetime(), status));
        }
        _ => state.push_output("Usage: time [stop|start|add <hours>]".to_string()),
    }
}

pub(super) fn cmd_help(state: &mut ConsoleState) {
    for line in HELP_TEXT {
        state.push_output(line.to_string());
    }
}

pub(super) fn cmd_unknown(state: &mut ConsoleState, command: &str) {
    state.push_output(format!("Unknown command: '{}'. Type 'help'.", command));
}
