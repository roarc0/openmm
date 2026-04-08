//! Drop-down developer console (Quake-style, toggled with Tab).
//!
//! Renders a semi-transparent overlay at the top of the viewport with scrollable
//! output lines and a text input prompt. Uses Bevy native text for crisp rendering.

use bevy::input::ButtonState;
use bevy::input::keyboard::KeyboardInput;
use bevy::pbr::FogFalloff;
use bevy::pbr::wireframe::WireframeConfig;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowMode};

use crate::GameState;
use crate::config::GameConfig;
use crate::game::InGame;
use crate::game::debug::hud::DebugHud;
use crate::game::hud::viewport_inner_rect;
use openmm_data::utils::MapName;
use crate::game::outdoor::{OdmName, PLAY_WIDTH};
use crate::game::world_state::WorldState;
use crate::save::GameSave;
use crate::states::loading::LoadRequest;
use crate::ui_assets::UiAssets;

const FONT_SIZE: f32 = 16.0;
const MAX_OUTPUT_LINES: usize = 50;
const NEEDS_RELOAD: &str = "(run 'reload' to apply)";
const CONSOLE_HEIGHT_FRACTION: f32 = 0.6;

/// Console state resource.
#[derive(Resource, Default)]
pub struct ConsoleState {
    pub open: bool,
    pub input: String,
    history: Vec<String>,
    history_index: Option<usize>,
    saved_input: String,
    output: Vec<String>,
    generation: u64,
}

impl ConsoleState {
    fn push_output(&mut self, line: String) {
        self.output.push(line);
        if self.output.len() > MAX_OUTPUT_LINES {
            self.output.remove(0);
        }
        self.generation += 1;
    }
}

#[derive(Component)]
struct ConsoleUI;

#[derive(Component)]
struct ConsoleOutput;

#[derive(Component)]
struct ConsolePrompt;

pub struct ConsolePlugin;

impl Plugin for ConsolePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ConsoleState>().add_systems(
            Update,
            (
                toggle_console,
                console_input,
                update_console_ui,
                toggle_debug_hud,
                sync_config_to_scene,
            )
                .chain()
                .run_if(in_state(GameState::Game)),
        );
    }
}

/// Toggle console open/close with Tab.
fn toggle_console(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<ConsoleState>,
    mut commands: Commands,
    existing: Query<Entity, With<ConsoleUI>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cfg: Res<GameConfig>,
    ui_assets: Res<UiAssets>,
) {
    let tab = keys.just_pressed(KeyCode::Tab) && cfg.console;
    let escape_close = keys.just_pressed(KeyCode::Escape) && state.open;
    if !tab && !escape_close {
        return;
    }
    // Escape only closes; Tab toggles
    if escape_close && !tab {
        state.open = false;
        for entity in existing.iter() {
            commands.entity(entity).despawn();
        }
        return;
    }

    state.open = !state.open;

    if state.open {
        let Ok(window) = windows.single() else { return };
        let (left, top, vp_w, vp_h) = viewport_inner_rect(window, &cfg, &ui_assets);
        let console_h = vp_h * CONSOLE_HEIGHT_FRACTION;

        commands
            .spawn((
                Name::new("console_ui"),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(left),
                    top: Val::Px(top),
                    width: Val::Px(vp_w),
                    height: Val::Px(console_h),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::End,
                    padding: UiRect::all(Val::Px(6.0)),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.88)),
                GlobalZIndex(100),
                ConsoleUI,
                InGame,
            ))
            .with_children(|parent| {
                parent.spawn((
                    Name::new("console_output"),
                    Text::new(""),
                    TextFont {
                        font_size: FONT_SIZE,
                        ..default()
                    },
                    TextColor(Color::srgba(0.7, 0.9, 0.7, 0.9)),
                    Node {
                        flex_grow: 1.0,
                        overflow: Overflow::clip_y(),
                        ..default()
                    },
                    ConsoleOutput,
                ));
                parent.spawn((
                    Name::new("console_prompt"),
                    Text::new("> _"),
                    TextFont {
                        font_size: FONT_SIZE,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    ConsolePrompt,
                ));
            });

        state.generation += 1;
    } else {
        for entity in existing.iter() {
            commands.entity(entity).despawn();
        }
    }
}

/// Handle keyboard input when console is open.
fn console_input(
    mut state: ResMut<ConsoleState>,
    keys: Res<ButtonInput<KeyCode>>,
    mut keyboard_events: MessageReader<KeyboardInput>,
    mut exit: MessageWriter<AppExit>,
    mut world_state: ResMut<WorldState>,
    mut save_data: ResMut<GameSave>,
    mut commands: Commands,
    mut game_state: ResMut<NextState<GameState>>,
    mut cfg: ResMut<GameConfig>,
    mut wireframe_config: ResMut<WireframeConfig>,
    game_assets: Res<crate::GameAssets>,
    mut game_time: ResMut<crate::game::game_time::GameTime>,
) {
    if !state.open {
        return;
    }

    for event in keyboard_events.read() {
        if event.state != ButtonState::Pressed {
            continue;
        }

        match event.key_code {
            KeyCode::Enter => {
                let cmd = state.input.trim().to_string();
                if !cmd.is_empty() {
                    state.history.push(cmd.clone());
                    if state.history.len() > 100 {
                        state.history.remove(0);
                    }
                    state.history_index = None;
                    state.saved_input.clear();
                    execute_command(
                        &cmd,
                        &mut state,
                        &mut exit,
                        &mut world_state,
                        &mut save_data,
                        &mut commands,
                        &mut game_state,
                        &mut cfg,
                        &mut wireframe_config,
                        &game_assets,
                        &mut game_time,
                    );
                    state.input.clear();
                }
            }
            KeyCode::Backspace => {
                state.input.pop();
                state.generation += 1;
            }
            KeyCode::ArrowUp => {
                if !state.history.is_empty() {
                    let idx = match state.history_index {
                        None => {
                            state.saved_input = state.input.clone();
                            state.history.len() - 1
                        }
                        Some(i) => i.saturating_sub(1),
                    };
                    state.history_index = Some(idx);
                    state.input = state.history[idx].clone();
                    state.generation += 1;
                }
            }
            KeyCode::ArrowDown => {
                if let Some(idx) = state.history_index {
                    if idx + 1 < state.history.len() {
                        let new_idx = idx + 1;
                        state.history_index = Some(new_idx);
                        state.input = state.history[new_idx].clone();
                    } else {
                        state.history_index = None;
                        state.input = state.saved_input.clone();
                    }
                    state.generation += 1;
                }
            }
            KeyCode::Tab => {}
            // Readline-style shortcuts
            k if (keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight)) => {
                let changed = match k {
                    // Ctrl+U: kill line (clear all)
                    KeyCode::KeyU => {
                        state.input.clear();
                        true
                    }
                    // Ctrl+W: kill word (delete last word)
                    KeyCode::KeyW => {
                        let trimmed = state.input.trim_end().len();
                        state.input.truncate(trimmed);
                        if let Some(pos) = state.input.rfind(' ') {
                            state.input.truncate(pos + 1);
                        } else {
                            state.input.clear();
                        }
                        true
                    }
                    // Ctrl+A: move to start (clear — no cursor in this console)
                    KeyCode::KeyA => {
                        state.input.clear();
                        true
                    }
                    // Ctrl+K: kill to end (same as clear since no cursor)
                    KeyCode::KeyK => {
                        state.input.clear();
                        true
                    }
                    _ => false,
                };
                if changed {
                    state.generation += 1;
                }
            }
            _ => {
                if let Some(ref text) = event.text {
                    let s = text.as_str();
                    if !s.is_empty() && s.chars().all(|c| !c.is_control()) {
                        state.input.push_str(s);
                        state.generation += 1;
                    }
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_command(
    cmd: &str,
    ctx_state: &mut ConsoleState,
    ctx_exit: &mut MessageWriter<AppExit>,
    ctx_world: &mut WorldState,
    ctx_save_data: &mut GameSave,
    ctx_commands: &mut Commands,
    ctx_game_state: &mut NextState<GameState>,
    ctx_cfg: &mut GameConfig,
    ctx_wireframe_config: &mut WireframeConfig,
    ctx_game_assets: &crate::GameAssets,
    ctx_game_time: &mut crate::game::game_time::GameTime,
) {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    let Some(&command) = parts.first() else { return };
    let arg = parts.get(1).copied().unwrap_or("");

    ctx_state.push_output(format!("> {}", cmd));

    match command {
        // --- Map loading ---
        "reload" => {
            let target = ctx_world.map.name.clone();
            ctx_world.write_to_save(ctx_save_data);
            ctx_state.push_output(format!("Reloading map: {}", target));
            ctx_state.open = false;
            ctx_commands.insert_resource(LoadRequest {
                map_name: target,
                spawn_position: None,
                spawn_yaw: None,
            });
            ctx_game_state.set(GameState::Loading);
        }
        "load" | "map" => {
            if arg.is_empty() {
                ctx_state.push_output("Usage: load <map|north|south|east|west> [x,z]".to_string());
                return;
            }
            let resolved = match arg {
                "north" | "n" => resolve_direction(
                    &ctx_world.map.name,
                    ctx_world.player.position,
                    OdmName::go_north,
                    0.0,
                    PLAY_WIDTH,
                ),
                "south" | "s" => resolve_direction(
                    &ctx_world.map.name,
                    ctx_world.player.position,
                    OdmName::go_south,
                    0.0,
                    -PLAY_WIDTH,
                ),
                "east" | "e" => resolve_direction(
                    &ctx_world.map.name,
                    ctx_world.player.position,
                    OdmName::go_east,
                    -PLAY_WIDTH,
                    0.0,
                ),
                "west" | "w" => resolve_direction(
                    &ctx_world.map.name,
                    ctx_world.player.position,
                    OdmName::go_west,
                    PLAY_WIDTH,
                    0.0,
                ),
                name => match MapName::try_from(name) {
                    Ok(target) => {
                        let filename = target.filename();
                        let lod_path = format!("games/{}", filename);
                        if ctx_game_assets.assets().get_bytes(&lod_path).is_err() {
                            Err(format!("Map not found: {}", filename))
                        } else {
                            let pos = parts.get(2).and_then(|c| parse_coords(c));
                            // Default to [0,0,0] so spawn_player uses the map's start point
                            Ok((target, pos.unwrap_or([0.0, 0.0, 0.0])))
                        }
                    }
                    Err(e) => Err(format!("Invalid map name '{}': {}", name, e)),
                },
            };
            match resolved {
                Ok((target, pos)) => {
                    ctx_state.push_output(format!("Loading map: {} at ({:.0}, {:.0})", target, pos[0], pos[2]));
                    ctx_state.open = false;
                    ctx_save_data.player.position = pos;
                    if let MapName::Outdoor(ref odm) = target {
                        ctx_save_data.map.map_x = odm.x;
                        ctx_save_data.map.map_y = odm.y;
                    }
                    ctx_commands.insert_resource(LoadRequest {
                        map_name: target.clone(),
                        spawn_position: None,
                        spawn_yaw: None,
                    });
                    ctx_world.map.name = target;
                    ctx_game_state.set(GameState::Loading);
                }
                Err(msg) => ctx_state.push_output(msg),
            }
        }

        // --- Graphics ---
        "msaa" | "aa" => match arg {
            "msaa2" | "msaa4" | "msaa8" | "fxaa" | "smaa" | "taa" | "off" => {
                ctx_cfg.antialiasing = arg.to_string();
                ctx_state.push_output(format!("Antialiasing: {} {NEEDS_RELOAD}", arg));
            }
            _ => {
                ctx_state.push_output(format!("Current: {}", ctx_cfg.antialiasing));
                ctx_state.push_output("Usage: aa <msaa2|msaa4|msaa8|fxaa|smaa|taa|off>".to_string());
            }
        },
        "tonemapping" | "tonemap" => match arg {
            "none" | "reinhard" | "aces" | "agx" | "blender_filmic" => {
                ctx_cfg.tonemapping = arg.to_string();
                ctx_state.push_output(format!("Tonemapping: {} {NEEDS_RELOAD}", arg));
            }
            _ => {
                ctx_state.push_output(format!("Current: {}", ctx_cfg.tonemapping));
                ctx_state.push_output("Usage: tonemap <none|reinhard|aces|agx|blender_filmic>".to_string());
            }
        },
        "wireframe" | "wf" => {
            ctx_wireframe_config.global = !ctx_wireframe_config.global;
            ctx_state.push_output(format!(
                "Wireframe: {}",
                if ctx_wireframe_config.global { "on" } else { "off" }
            ));
        }
        "shadows" => {
            ctx_cfg.shadows = parse_toggle(arg, ctx_cfg.shadows);
            ctx_state.push_output(format!("Shadows: {}", if ctx_cfg.shadows { "on" } else { "off" }));
        }
        "bloom" => {
            match arg {
                "on" | "1" => ctx_cfg.bloom = true,
                "off" | "0" => ctx_cfg.bloom = false,
                "" => ctx_cfg.bloom = !ctx_cfg.bloom,
                intensity => {
                    if let Ok(v) = intensity.parse::<f32>() {
                        ctx_cfg.bloom = true;
                        ctx_cfg.bloom_intensity = v.clamp(0.0, 1.0);
                    } else {
                        ctx_state.push_output("Usage: bloom [on|off|0.0-1.0]".to_string());
                        return;
                    }
                }
            }
            ctx_state.push_output(format!(
                "Bloom: {} (intensity: {:.2}) {NEEDS_RELOAD}",
                if ctx_cfg.bloom { "on" } else { "off" },
                ctx_cfg.bloom_intensity
            ));
        }
        "ssao" => {
            ctx_cfg.ssao = parse_toggle(arg, ctx_cfg.ssao);
            ctx_state.push_output(format!(
                "SSAO: {} {NEEDS_RELOAD}",
                if ctx_cfg.ssao { "on" } else { "off" }
            ));
        }
        "fog" => {
            if arg.is_empty() {
                ctx_state.push_output(format!(
                    "Fog start: {:.0}, end: {:.0}",
                    ctx_cfg.fog_start, ctx_cfg.fog_end
                ));
            } else if let (Ok(start), Some(Ok(end))) = (arg.parse::<f32>(), parts.get(2).map(|s| s.parse::<f32>())) {
                ctx_cfg.fog_start = start;
                ctx_cfg.fog_end = end;
                ctx_state.push_output(format!("Fog: {:.0} - {:.0}", start, end));
            } else {
                ctx_state.push_output("Usage: fog <start> <end>".to_string());
            }
        }
        "draw_distance" | "dd" => {
            if arg.is_empty() {
                ctx_state.push_output(format!("Draw distance: {:.0}", ctx_cfg.draw_distance));
            } else if let Ok(v) = arg.parse::<f32>() {
                ctx_cfg.draw_distance = v;
                ctx_state.push_output(format!("Draw distance: {:.0}", v));
            } else {
                ctx_state.push_output("Usage: dd <distance>".to_string());
            }
        }
        "exposure" => {
            if arg.is_empty() {
                ctx_state.push_output(format!("Exposure: {:.2}", ctx_cfg.exposure));
            } else if let Ok(v) = arg.parse::<f32>() {
                ctx_cfg.exposure = v.clamp(-4.0, 4.0);
                ctx_state.push_output(format!("Exposure: {:.2}", ctx_cfg.exposure));
            } else {
                ctx_state.push_output("Usage: exposure <-4.0 to 4.0>".to_string());
            }
        }
        "dof" | "depth_of_field" => match arg {
            "off" | "0" => {
                ctx_cfg.depth_of_field = false;
                ctx_state.push_output("Depth of field: off".to_string());
            }
            "on" | "1" => {
                ctx_cfg.depth_of_field = true;
                ctx_state.push_output(format!(
                    "Depth of field: on (distance: {:.1})",
                    ctx_cfg.depth_of_field_distance
                ));
            }
            "" => {
                ctx_state.push_output(format!(
                    "Depth of field: {} (distance: {:.1})",
                    if ctx_cfg.depth_of_field { "on" } else { "off" },
                    ctx_cfg.depth_of_field_distance
                ));
            }
            dist => {
                if let Ok(v) = dist.parse::<f32>() {
                    ctx_cfg.depth_of_field = true;
                    ctx_cfg.depth_of_field_distance = v.max(0.1);
                    ctx_state.push_output(format!(
                        "Depth of field: on (distance: {:.1})",
                        ctx_cfg.depth_of_field_distance
                    ));
                } else {
                    ctx_state.push_output("Usage: dof [on|off|<distance>]".to_string());
                }
            }
        },

        // --- Gameplay ---
        // --- Inventory ---
        "item" => {
            let sub = arg;
            let item_id: Option<i32> = parts.get(2).and_then(|s| s.parse().ok());
            let count: i32 = parts.get(3).and_then(|s| s.parse().ok()).unwrap_or(1);
            match (sub, item_id) {
                ("add", Some(id)) => {
                    ctx_world.game_vars.give_item(id, count);
                    ctx_state.push_output(format!("Item {}: count now {}", id, ctx_world.game_vars.item_count(id)));
                }
                ("rem", Some(id)) => {
                    ctx_world.game_vars.remove_item(id, count);
                    ctx_state.push_output(format!("Item {}: count now {}", id, ctx_world.game_vars.item_count(id)));
                }
                _ => ctx_state.push_output("Usage: item add|rem <id> [count]".to_string()),
            }
        }

        // --- Quest bits ---
        "qbit" => {
            let bit: Option<i32> = parts.get(1).and_then(|s| s.parse().ok());
            let value = parts.get(2).copied();
            match bit {
                None => ctx_state.push_output("Usage: qbit <n> [true|false]".to_string()),
                Some(n) => match value {
                    None => {
                        let state = if ctx_world.game_vars.has_qbit(n) {
                            "set"
                        } else {
                            "not set"
                        };
                        ctx_state.push_output(format!("QBit {}: {}", n, state));
                    }
                    Some("true" | "1" | "on") => {
                        ctx_world.game_vars.set_qbit(n);
                        ctx_state.push_output(format!("QBit {} set", n));
                    }
                    Some("false" | "0" | "off") => {
                        ctx_world.game_vars.clear_qbit(n);
                        ctx_state.push_output(format!("QBit {} cleared", n));
                    }
                    Some(v) => ctx_state.push_output(format!("Unknown value '{}'; use true or false", v)),
                },
            }
        }

        // --- Gameplay ---
        "fly" => {
            ctx_world.player.fly_mode = parse_toggle(arg, ctx_world.player.fly_mode);
            ctx_state.push_output(format!(
                "Fly mode: {}",
                if ctx_world.player.fly_mode { "on" } else { "off" }
            ));
        }
        "speed" => {
            if arg.is_empty() {
                ctx_state.push_output(format!("Turn speed: {:.0}", ctx_cfg.turn_speed));
            } else if let Ok(v) = arg.parse::<f32>() {
                ctx_cfg.turn_speed = v;
                ctx_state.push_output(format!("Turn speed: {:.0}", v));
            } else {
                ctx_state.push_output("Usage: speed <turn_speed>".to_string());
            }
        }
        "sensitivity" | "sens" => {
            if arg.is_empty() {
                ctx_state.push_output(format!(
                    "Mouse sensitivity: {:.2}x {:.2}y",
                    ctx_cfg.mouse_sensitivity_x, ctx_cfg.mouse_sensitivity_y
                ));
            } else if let Ok(v) = arg.parse::<f32>() {
                ctx_cfg.mouse_sensitivity_x = v;
                ctx_cfg.mouse_sensitivity_y = v;
                ctx_state.push_output(format!("Mouse sensitivity: {:.2}", v));
            } else {
                ctx_state.push_output("Usage: sens <value>".to_string());
            }
        }
        "pos" => {
            let p = ctx_world.player.position;
            // MM6: X right, Y forward, Z up. Bevy: X right, Y up, Z = -Y_mm6.
            let mm6_x = p.x as i32;
            let mm6_y = (-p.z) as i32;
            let mm6_z = p.y as i32;
            ctx_state.push_output(format!("MM6:  x={} y={} z={}", mm6_x, mm6_y, mm6_z));
            ctx_state.push_output(format!("Bevy: x={:.1} y={:.1} z={:.1}", p.x, p.y, p.z));
            ctx_state.push_output(format!("Map:  {}", ctx_world.map.name));
        }

        // --- Window ---
        "fullscreen" | "fs" => {
            ctx_cfg.window_mode = "fullscreen".into();
            ctx_state.push_output("Fullscreen".to_string());
        }
        "borderless" => {
            ctx_cfg.window_mode = "borderless".into();
            ctx_state.push_output("Borderless fullscreen".to_string());
        }
        "windowed" | "window" => {
            ctx_cfg.window_mode = "windowed".into();
            ctx_state.push_output("Windowed mode".to_string());
        }
        "aspect" | "aspect_ratio" | "ar" => {
            if arg.is_empty() {
                let display = if ctx_cfg.aspect_ratio.is_empty() {
                    "auto"
                } else {
                    &ctx_cfg.aspect_ratio
                };
                ctx_state.push_output(format!("Aspect ratio: {}", display));
            } else if arg == "auto" {
                ctx_cfg.aspect_ratio = "".into();
                ctx_state.push_output("Aspect ratio: auto (uses window size)".to_string());
            } else if arg.contains(':') && crate::game::hud::parse_aspect_ratio(arg).is_some() {
                ctx_cfg.aspect_ratio = arg.to_string();
                ctx_state.push_output(format!("Aspect ratio: {}", arg));
            } else {
                ctx_state.push_output("Usage: aspect <auto|4:3|16:9|21:9>".to_string());
            }
        }
        "vsync" => {
            match arg {
                "on" | "auto" => ctx_cfg.vsync = "auto".into(),
                "fast" => ctx_cfg.vsync = "fast".into(),
                "off" | "0" => ctx_cfg.vsync = "off".into(),
                _ => {
                    ctx_state.push_output(format!("Current: {}", ctx_cfg.vsync));
                    ctx_state.push_output("Usage: vsync <auto|fast|off>".to_string());
                    return;
                }
            }
            ctx_state.push_output(format!("VSync: {}", ctx_cfg.vsync));
        }
        "fps_cap" => {
            if arg.is_empty() {
                let cap = if ctx_cfg.fps_cap == 0 {
                    "unlimited".to_string()
                } else {
                    format!("{}", ctx_cfg.fps_cap)
                };
                ctx_state.push_output(format!("FPS cap: {}", cap));
            } else if let Ok(v) = arg.parse::<u32>() {
                ctx_cfg.fps_cap = v;
                let cap = if v == 0 {
                    "unlimited".to_string()
                } else {
                    format!("{}", v)
                };
                ctx_state.push_output(format!("FPS cap: {}", cap));
            } else {
                ctx_state.push_output("Usage: fps_cap <0=unlimited|30|60|120|...>".to_string());
            }
        }

        // --- Audio ---
        "mute" => {
            ctx_cfg.music_volume = 0.0;
            ctx_cfg.sfx_volume = 0.0;
            ctx_state.push_output("All audio muted".to_string());
        }
        "unmute" => {
            if ctx_cfg.music_volume == 0.0 {
                ctx_cfg.music_volume = 0.5;
            }
            if ctx_cfg.sfx_volume == 0.0 {
                ctx_cfg.sfx_volume = 1.0;
            }
            ctx_state.push_output(format!(
                "Audio unmuted (music: {:.0}%, sfx: {:.0}%)",
                ctx_cfg.music_volume * 100.0,
                ctx_cfg.sfx_volume * 100.0
            ));
        }
        "music" => {
            if arg.is_empty() {
                ctx_state.push_output(format!("Music volume: {:.0}%", ctx_cfg.music_volume * 100.0));
            } else if let Ok(v) = arg.parse::<f32>() {
                // Accept both 0-1 range and 0-100 range
                ctx_cfg.music_volume = if v > 1.0 {
                    (v / 100.0).clamp(0.0, 1.0)
                } else {
                    v.clamp(0.0, 1.0)
                };
                ctx_state.push_output(format!("Music volume: {:.0}%", ctx_cfg.music_volume * 100.0));
            } else {
                ctx_state.push_output("Usage: music <0-100>".to_string());
            }
        }
        "sfx" => {
            if arg.is_empty() {
                ctx_state.push_output(format!("SFX volume: {:.0}%", ctx_cfg.sfx_volume * 100.0));
            } else if let Ok(v) = arg.parse::<f32>() {
                ctx_cfg.sfx_volume = if v > 1.0 {
                    (v / 100.0).clamp(0.0, 1.0)
                } else {
                    v.clamp(0.0, 1.0)
                };
                ctx_state.push_output(format!("SFX volume: {:.0}%", ctx_cfg.sfx_volume * 100.0));
            } else {
                ctx_state.push_output("Usage: sfx <0-100>".to_string());
            }
        }
        "volume" | "vol" => {
            if arg.is_empty() {
                ctx_state.push_output(format!(
                    "Music: {:.0}%, SFX: {:.0}%",
                    ctx_cfg.music_volume * 100.0,
                    ctx_cfg.sfx_volume * 100.0
                ));
            } else if let Ok(v) = arg.parse::<f32>() {
                let vol = if v > 1.0 {
                    (v / 100.0).clamp(0.0, 1.0)
                } else {
                    v.clamp(0.0, 1.0)
                };
                ctx_cfg.music_volume = vol;
                ctx_cfg.sfx_volume = vol;
                ctx_state.push_output(format!("All volume: {:.0}%", vol * 100.0));
            } else {
                ctx_state.push_output("Usage: volume <0-100>".to_string());
            }
        }

        // --- System ---
        "debug" => {
            ctx_cfg.debug = parse_toggle(arg, ctx_cfg.debug);
            ctx_world.debug.show_play_area = ctx_cfg.debug;
            ctx_world.debug.show_events = ctx_cfg.debug;
            ctx_state.push_output(format!("Debug HUD: {}", if ctx_cfg.debug { "on" } else { "off" }));
        }
        "lighting" => {
            if arg.is_empty() {
                ctx_state.push_output(format!("Lighting: {}", ctx_cfg.lighting));
            } else {
                match arg {
                    "classic" | "enhanced" => {
                        ctx_cfg.lighting = arg.to_string();
                        ctx_state.push_output(format!("Lighting: {}", ctx_cfg.lighting));
                    }
                    _ => ctx_state.push_output("Usage: lighting [classic|enhanced]".to_string()),
                }
            }
        }
        "filtering" => {
            if arg.is_empty() {
                ctx_state.push_output(format!(
                    "terrain={} models={} hud={}",
                    ctx_cfg.terrain_filtering, ctx_cfg.models_filtering, ctx_cfg.hud_filtering
                ));
            } else {
                match arg {
                    "nearest" | "linear" => {
                        ctx_cfg.terrain_filtering = arg.to_string();
                        ctx_cfg.models_filtering = arg.to_string();
                        ctx_cfg.hud_filtering = arg.to_string();
                        ctx_state.push_output(format!("All filtering: {} {NEEDS_RELOAD}", arg));
                    }
                    _ => ctx_state.push_output("Usage: filtering [nearest|linear]".to_string()),
                }
            }
        }
        "terrain_filtering" | "tf" => {
            set_filtering(ctx_state, &mut ctx_cfg.terrain_filtering, "Terrain", arg);
        }
        "models_filtering" | "mf" => {
            set_filtering(ctx_state, &mut ctx_cfg.models_filtering, "Models", arg);
        }
        "hud_filtering" | "hf" => {
            set_filtering(ctx_state, &mut ctx_cfg.hud_filtering, "HUD", arg);
        }
        "exit" | "quit" | "q" => {
            ctx_exit.write(AppExit::from_code(0));
        }
        "clear" | "cls" => {
            ctx_state.output.clear();
            ctx_state.generation += 1;
        }
        "save_cfg" => match ctx_cfg.save() {
            Ok(()) => ctx_state.push_output(format!("Config saved to {}", ctx_cfg.config_path.display())),
            Err(e) => ctx_state.push_output(e),
        },
        "time" => match arg {
            "stop" | "pause" => {
                ctx_game_time.set_paused(true);
                ctx_state.push_output(format!("Time paused at {}", ctx_game_time.format_datetime()));
            }
            "start" | "resume" => {
                ctx_game_time.set_paused(false);
                ctx_state.push_output("Time resumed".to_string());
            }
            "add" => {
                let hours: f32 = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0.0);
                if hours == 0.0 {
                    ctx_state.push_output("Usage: time add <hours>".to_string());
                } else {
                    ctx_game_time.advance_hours(hours);
                    ctx_state.push_output(format!("Advanced {hours}h → {}", ctx_game_time.format_datetime()));
                }
            }
            "" => {
                let status = if ctx_game_time.is_paused() { " (paused)" } else { "" };
                ctx_state.push_output(format!("{}{}", ctx_game_time.format_datetime(), status));
            }
            _ => ctx_state.push_output("Usage: time [stop|start|add <hours>]".to_string()),
        },
        "help" | "?" => {
            for line in HELP_TEXT {
                ctx_state.push_output(line.to_string());
            }
        }
        _ => {
            ctx_state.push_output(format!("Unknown command: '{}'. Type 'help'.", command));
        }
    }
}

// --- Command handlers ---

fn set_filtering(state: &mut ConsoleState, field: &mut String, label: &str, arg: &str) {
    if arg.is_empty() {
        state.push_output(format!("{} filtering: {}", label, field));
    } else {
        match arg {
            "nearest" | "linear" => {
                *field = arg.to_string();
                state.push_output(format!("{} filtering: {} {NEEDS_RELOAD}", label, arg));
            }
            _ => state.push_output(format!("Usage: {} [nearest|linear]", label.to_lowercase())),
        }
    }
}

fn parse_toggle(arg: &str, current: bool) -> bool {
    match arg {
        "on" | "1" => true,
        "off" | "0" => false,
        _ => !current,
    }
}

const HELP_TEXT: &[&str] = &[
    "Map:",
    "  load <map> [x,z] - Load map (e.g. load oute3, load d01 100,200)",
    "  load n/s/e/w     - Adjacent map (keeps position)",
    "  reload           - Reload current map",
    "  pos              - Show current position and map",
    "Graphics:",
    "  aa <mode>        - Set AA (msaa2/4/8|fxaa|smaa|taa|off) *",
    "  tonemap <mode>   - Tonemapping *",
    "  wireframe        - Toggle wireframe",
    "  shadows [on|off] - Toggle shadows",
    "  bloom [on|off|N] - Bloom / intensity *",
    "  ssao [on|off]    - Ambient occlusion *",
    "  fog <start> <end> - Set fog range",
    "  dd <distance>    - Set draw distance",
    "  lighting <mode>  - classic or enhanced",
    "  filtering <mode> - nearest or linear (all) *",
    "  tf/mf/hf <mode>  - terrain/models/hud filter *",
    "  exposure <N>     - Set exposure (-4 to 4)",
    "  dof [on|off|N]   - Depth of field / focal distance",
    "  (* = run 'reload' to apply)",
    "Gameplay:",
    "  item add <id> [count] - Give item to party (default count 1)",
    "  item rem <id> [count] - Remove item from party (default count 1)",
    "  qbit <n> [on|off]     - Check or set/clear quest bit n",
    "  fly [on|off]     - Toggle fly mode",
    "  speed <N>        - Set turn speed",
    "  sens <N>         - Set mouse sensitivity",
    "  time             - Show current in-game date/time",
    "  time stop        - Pause the game clock",
    "  time start       - Resume the game clock",
    "  time add <N>     - Skip forward N in-game hours",
    "Audio:",
    "  music <0-100>    - Set music volume",
    "  sfx <0-100>      - Set sound effects volume",
    "  volume <0-100>   - Set all audio volume",
    "  mute             - Mute all audio",
    "  unmute           - Restore audio volume",
    "Window:",
    "  fullscreen       - Fullscreen mode",
    "  borderless       - Borderless fullscreen",
    "  windowed         - Windowed mode",
    "  aspect <auto|4:3|16:9> - Set aspect ratio",
    "  vsync <auto|fast|off> - Set vsync",
    "  fps_cap <N>      - Set FPS cap (0=unlimited)",
    "System:",
    "  debug [on|off]   - Toggle debug HUD",
    "  save_cfg         - Save config to disk",
    "  clear            - Clear console",
    "  quit             - Exit game",
];

// --- Helpers ---

fn resolve_direction(
    current_map: &MapName,
    player_pos: Vec3,
    dir_fn: fn(&OdmName) -> Option<OdmName>,
    x_offset: f32,
    z_offset: f32,
) -> Result<(MapName, [f32; 3]), String> {
    match current_map {
        MapName::Outdoor(odm) => match dir_fn(odm) {
            Some(next) => Ok((
                MapName::Outdoor(next),
                [player_pos.x + x_offset, player_pos.y, player_pos.z + z_offset],
            )),
            None => Err("No map in that direction.".to_string()),
        },
        MapName::Indoor(_) => Err("Directional navigation only works on outdoor maps.".to_string()),
    }
}

fn parse_coords(s: &str) -> Option<[f32; 3]> {
    let mut parts = s.split(',');
    let x: f32 = parts.next()?.trim().parse().ok()?;
    let z: f32 = parts.next()?.trim().parse().ok()?;
    Some([x, 0.0, z])
}

// --- UI update ---

fn update_console_ui(
    state: Res<ConsoleState>,
    mut last_gen: Local<u64>,
    mut output_q: Query<&mut Text, (With<ConsoleOutput>, Without<ConsolePrompt>)>,
    mut prompt_q: Query<&mut Text, (With<ConsolePrompt>, Without<ConsoleOutput>)>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cfg: Res<GameConfig>,
    ui_assets: Res<UiAssets>,
    mut console_q: Query<&mut Node, (With<ConsoleUI>, Without<ConsoleOutput>, Without<ConsolePrompt>)>,
) {
    if !state.open {
        return;
    }

    if let Ok(window) = windows.single() {
        let (left, top, vp_w, vp_h) = viewport_inner_rect(window, &cfg, &ui_assets);
        let console_h = vp_h * CONSOLE_HEIGHT_FRACTION;
        for mut node in console_q.iter_mut() {
            node.left = Val::Px(left);
            node.top = Val::Px(top);
            node.width = Val::Px(vp_w);
            node.height = Val::Px(console_h);
        }
    }

    if state.generation == *last_gen {
        return;
    }
    *last_gen = state.generation;

    if let Ok(mut text) = output_q.single_mut() {
        **text = state.output.join("\n");
    }
    if let Ok(mut text) = prompt_q.single_mut() {
        **text = format!("> {}_", state.input);
    }
}

/// Hide debug HUD while console is open. Only runs when console or config state changes.
fn toggle_debug_hud(
    state: Res<ConsoleState>,
    cfg: Res<GameConfig>,
    mut debug_q: Query<&mut Visibility, With<DebugHud>>,
) {
    if !state.is_changed() && !cfg.is_changed() {
        return;
    }
    let vis = if state.open || !cfg.debug {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    for mut v in debug_q.iter_mut() {
        *v = vis;
    }
}

/// Sync GameConfig to live scene components. Only runs when config changes.
fn sync_config_to_scene(
    cfg: Res<GameConfig>,
    mut commands: Commands,
    mut sun_q: Query<&mut DirectionalLight>,
    mut fog_q: Query<&mut DistanceFog>,
    mut exposure_q: Query<&mut bevy::camera::Exposure>,
    camera_q: Query<(Entity, Option<&bevy::post_process::dof::DepthOfField>), With<Camera3d>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    if !cfg.is_changed() {
        return;
    }

    for mut light in sun_q.iter_mut() {
        light.shadows_enabled = cfg.shadows;
    }

    for mut fog in fog_q.iter_mut() {
        fog.falloff = FogFalloff::Linear {
            start: cfg.fog_start,
            end: cfg.fog_end,
        };
    }

    for mut exp in exposure_q.iter_mut() {
        exp.ev100 = 9.7 + cfg.exposure;
    }

    for (entity, existing_dof) in camera_q.iter() {
        if let Some(dof) = crate::engine::camera_dof(&cfg) {
            commands.entity(entity).insert(dof);
        } else if existing_dof.is_some() {
            commands
                .entity(entity)
                .remove::<bevy::post_process::dof::DepthOfField>();
        }
    }

    if let Ok(mut window) = windows.single_mut() {
        let target_mode = match cfg.window_mode.as_str() {
            "fullscreen" => WindowMode::Fullscreen(
                bevy::window::MonitorSelection::Current,
                bevy::window::VideoModeSelection::Current,
            ),
            "borderless" => WindowMode::BorderlessFullscreen(bevy::window::MonitorSelection::Current),
            _ => WindowMode::Windowed,
        };
        if std::mem::discriminant(&window.mode) != std::mem::discriminant(&target_mode) {
            window.mode = target_mode;
        }

        let present_mode = if cfg.fps_cap == 0 {
            bevy::window::PresentMode::Immediate
        } else {
            match cfg.vsync.as_str() {
                "fast" => bevy::window::PresentMode::Mailbox,
                "off" => bevy::window::PresentMode::Immediate,
                _ => bevy::window::PresentMode::AutoVsync,
            }
        };
        window.present_mode = present_mode;
    }
}
