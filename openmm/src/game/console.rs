//! Drop-down developer console (Quake-style, toggled with Tab).
//!
//! Renders a semi-transparent overlay at the top of the viewport with scrollable
//! output lines and a text input prompt. Uses Bevy native text for crisp rendering.

use bevy::input::ButtonState;
use bevy::input::keyboard::KeyboardInput;
use bevy::pbr::wireframe::WireframeConfig;
use bevy::pbr::FogFalloff;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowMode};

use crate::GameState;
use crate::config::GameConfig;
use crate::game::InGame;
use crate::game::debug::{CurrentMapName, DebugConfig, DebugHud};
use crate::game::map_name::MapName;
use crate::game::odm::{OdmName, PLAY_WIDTH};
use crate::game::player::{FlyMode, Player};
use crate::game::hud::viewport_inner_rect;
use crate::save::GameSave;
use crate::states::loading::LoadRequest;
use crate::ui_assets::UiAssets;

const FONT_SIZE: f32 = 16.0;
const MAX_OUTPUT_LINES: usize = 50;
const CONSOLE_HEIGHT_FRACTION: f32 = 0.4;

/// Console state resource.
#[derive(Resource, Default)]
pub struct ConsoleState {
    pub open: bool,
    pub input: String,
    /// History of executed commands (most recent last).
    history: Vec<String>,
    /// Index into history for up/down navigation.
    history_index: Option<usize>,
    /// Saved current input when browsing history.
    saved_input: String,
    /// Lines of output (command results, errors).
    output: Vec<String>,
    /// Generation counter for re-rendering.
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

/// Marker for the console UI root node.
#[derive(Component)]
struct ConsoleUI;

/// Marker for the output text area.
#[derive(Component)]
struct ConsoleOutput;

/// Marker for the prompt text.
#[derive(Component)]
struct ConsolePrompt;

pub struct ConsolePlugin;

impl Plugin for ConsolePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ConsoleState>()
            .add_systems(
                Update,
                (toggle_console, console_input, update_console_ui, toggle_debug_hud, sync_config_to_scene)
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
    if !keys.just_pressed(KeyCode::Tab) || !cfg.console {
        return;
    }

    state.open = !state.open;

    if state.open {
        let Ok(window) = windows.single() else { return };
        let (left, top, vp_w, vp_h) = viewport_inner_rect(&window, &cfg, &ui_assets);
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
                // Scrollable output area
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

                // Prompt line
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
    mut keyboard_events: MessageReader<KeyboardInput>,
    mut exit: MessageWriter<AppExit>,
    mut current_map: ResMut<CurrentMapName>,
    player_query: Query<&Transform, With<Player>>,
    mut save_data: ResMut<GameSave>,
    mut commands: Commands,
    mut game_state: ResMut<NextState<GameState>>,
    mut cfg: ResMut<GameConfig>,
    mut fly_mode: ResMut<FlyMode>,
    mut wireframe_config: ResMut<WireframeConfig>,
    mut debug_config: ResMut<DebugConfig>,
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
                    state.history_index = None;
                    state.saved_input.clear();
                    let player_pos = player_query
                        .single()
                        .map(|t| t.translation)
                        .unwrap_or_default();
                    execute_command(
                        &cmd,
                        &mut state,
                        &mut exit,
                        &mut current_map,
                        player_pos,
                        &mut save_data,
                        &mut commands,
                        &mut game_state,
                        &mut cfg,
                        &mut fly_mode,
                        &mut wireframe_config,
                        &mut debug_config,
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
            KeyCode::Tab => {
                // Consumed by toggle, ignore
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
    state: &mut ConsoleState,
    exit: &mut MessageWriter<AppExit>,
    current_map: &mut CurrentMapName,
    player_pos: Vec3,
    save_data: &mut GameSave,
    commands: &mut Commands,
    game_state: &mut NextState<GameState>,
    cfg: &mut GameConfig,
    fly_mode: &mut FlyMode,
    wireframe_config: &mut WireframeConfig,
    debug_config: &mut DebugConfig,
) {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    let Some(&command) = parts.first() else { return };
    let arg = parts.get(1).copied().unwrap_or("");

    state.push_output(format!("> {}", cmd));

    match command {
        // --- Map loading ---
        "load" | "map" => {
            if arg.is_empty() {
                state.push_output("Usage: load <map|north|south|east|west> [x,z]".to_string());
            } else {
                let resolved = match arg {
                    "north" | "n" => resolve_direction(current_map, player_pos, OdmName::go_north, 0.0, PLAY_WIDTH),
                    "south" | "s" => resolve_direction(current_map, player_pos, OdmName::go_south, 0.0, -PLAY_WIDTH),
                    "east" | "e" => resolve_direction(current_map, player_pos, OdmName::go_east, -PLAY_WIDTH, 0.0),
                    "west" | "w" => resolve_direction(current_map, player_pos, OdmName::go_west, PLAY_WIDTH, 0.0),
                    name => {
                        match MapName::try_from(name) {
                            Ok(target) => {
                                let pos = parts.get(2).and_then(|c| parse_coords(c));
                                Ok((target, pos.unwrap_or([0.0, player_pos.y, 0.0])))
                            }
                            Err(e) => Err(format!("Invalid map name '{}': {}", name, e)),
                        }
                    }
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

                        commands.insert_resource(LoadRequest { map_name: target.clone() });
                        current_map.0 = target;
                        game_state.set(GameState::Loading);
                    }
                    Err(msg) => state.push_output(msg),
                }
            }
        }

        // --- Graphics ---
        "msaa" | "aa" => {
            match arg {
                "msaa2" | "msaa4" | "msaa8" | "fxaa" | "smaa" | "taa" | "off" => {
                    cfg.antialiasing = arg.to_string();
                    state.push_output(format!("Antialiasing: {} (applies on map reload)", arg));
                }
                _ => {
                    state.push_output(format!("Current: {}", cfg.antialiasing));
                    state.push_output("Usage: aa <msaa2|msaa4|msaa8|fxaa|smaa|taa|off>".to_string());
                }
            }
        }

        "tonemapping" | "tonemap" => {
            match arg {
                "none" | "reinhard" | "aces" | "agx" | "blender_filmic" => {
                    cfg.tonemapping = arg.to_string();
                    state.push_output(format!("Tonemapping: {} *reload", arg));
                }
                _ => {
                    state.push_output(format!("Current: {}", cfg.tonemapping));
                    state.push_output("Usage: tonemap <none|reinhard|aces|agx|blender_filmic>".to_string());
                }
            }
        }

        "wireframe" | "wf" => {
            wireframe_config.global = !wireframe_config.global;
            state.push_output(format!("Wireframe: {}", if wireframe_config.global { "on" } else { "off" }));
        }

        "shadows" => {
            cfg.shadows = match arg {
                "on" | "1" => true,
                "off" | "0" => false,
                _ => !cfg.shadows,
            };
            state.push_output(format!("Shadows: {}", if cfg.shadows { "on" } else { "off" }));
        }

        "bloom" => {
            match arg {
                "on" | "1" => cfg.bloom = true,
                "off" | "0" => cfg.bloom = false,
                _ if arg.is_empty() => cfg.bloom = !cfg.bloom,
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
            state.push_output(format!("Bloom: {} (intensity: {:.2}) *reload", if cfg.bloom { "on" } else { "off" }, cfg.bloom_intensity));
        }

        "ssao" => {
            cfg.ssao = match arg {
                "on" | "1" => true,
                "off" | "0" => false,
                _ => !cfg.ssao,
            };
            state.push_output(format!("SSAO: {} *reload", if cfg.ssao { "on" } else { "off" }));
        }

        "fog" => {
            if arg.is_empty() {
                state.push_output(format!("Fog start: {:.0}, end: {:.0}", cfg.fog_start, cfg.fog_end));
            } else if let Some((start, end)) = parse_fog_range(arg, parts.get(2).copied()) {
                cfg.fog_start = start;
                cfg.fog_end = end;
                state.push_output(format!("Fog: {:.0} - {:.0}", start, end));
            } else {
                state.push_output("Usage: fog <start> <end>".to_string());
            }
        }

        "draw_distance" | "dd" => {
            if arg.is_empty() {
                state.push_output(format!("Draw distance: {:.0}", cfg.draw_distance));
            } else if let Ok(v) = arg.parse::<f32>() {
                cfg.draw_distance = v;
                state.push_output(format!("Draw distance: {:.0}", v));
            } else {
                state.push_output("Usage: dd <distance>".to_string());
            }
        }

        "exposure" => {
            if arg.is_empty() {
                state.push_output(format!("Exposure: {:.2}", cfg.exposure));
            } else if let Ok(v) = arg.parse::<f32>() {
                cfg.exposure = v.clamp(-4.0, 4.0);
                state.push_output(format!("Exposure: {:.2}", cfg.exposure));
            } else {
                state.push_output("Usage: exposure <-4.0 to 4.0>".to_string());
            }
        }

        "dof" | "depth_of_field" => {
            match arg {
                "off" | "0" => {
                    cfg.depth_of_field = false;
                    state.push_output("Depth of field: off".to_string());
                }
                "on" | "1" => {
                    cfg.depth_of_field = true;
                    state.push_output(format!("Depth of field: on (distance: {:.1})", cfg.depth_of_field_distance));
                }
                "" => {
                    state.push_output(format!("Depth of field: {} (distance: {:.1})",
                        if cfg.depth_of_field { "on" } else { "off" }, cfg.depth_of_field_distance));
                }
                dist => {
                    if let Ok(v) = dist.parse::<f32>() {
                        cfg.depth_of_field = true;
                        cfg.depth_of_field_distance = v.max(0.1);
                        state.push_output(format!("Depth of field: on (distance: {:.1})", cfg.depth_of_field_distance));
                    } else {
                        state.push_output("Usage: dof [on|off|<distance>]".to_string());
                    }
                }
            }
        }

        // --- Gameplay ---
        "fly" => {
            fly_mode.0 = match arg {
                "on" | "1" => true,
                "off" | "0" => false,
                _ => !fly_mode.0,
            };
            state.push_output(format!("Fly mode: {}", if fly_mode.0 { "on" } else { "off" }));
        }

        "speed" => {
            if arg.is_empty() {
                state.push_output(format!("Turn speed: {:.0}, sensitivity: {:.2}x {:.2}y",
                    cfg.turn_speed, cfg.mouse_sensitivity_x, cfg.mouse_sensitivity_y));
            } else if let Ok(v) = arg.parse::<f32>() {
                cfg.turn_speed = v;
                state.push_output(format!("Turn speed: {:.0}", v));
            } else {
                state.push_output("Usage: speed <turn_speed>".to_string());
            }
        }

        "sensitivity" | "sens" => {
            if arg.is_empty() {
                state.push_output(format!("Mouse sensitivity: {:.2}x {:.2}y",
                    cfg.mouse_sensitivity_x, cfg.mouse_sensitivity_y));
            } else if let Ok(v) = arg.parse::<f32>() {
                cfg.mouse_sensitivity_x = v;
                cfg.mouse_sensitivity_y = v;
                state.push_output(format!("Mouse sensitivity: {:.2}", v));
            } else {
                state.push_output("Usage: sens <value>".to_string());
            }
        }

        "pos" => {
            state.push_output(format!("Position: ({:.0}, {:.1}, {:.0})",
                player_pos.x, player_pos.y, player_pos.z));
            state.push_output(format!("Map: {}", current_map.0));
        }

        // --- Window ---
        "fullscreen" | "fs" => {
            cfg.window_mode = "fullscreen".into();
            state.push_output("Fullscreen enabled".to_string());
        }

        "borderless" => {
            cfg.window_mode = "borderless".into();
            state.push_output("Borderless fullscreen enabled".to_string());
        }

        "windowed" | "window" => {
            cfg.window_mode = "windowed".into();
            state.push_output("Windowed mode enabled".to_string());
        }

        "aspect" | "aspect_ratio" | "ar" => {
            if arg.is_empty() {
                let display = if cfg.aspect_ratio.is_empty() { "auto" } else { &cfg.aspect_ratio };
                state.push_output(format!("Aspect ratio: {}", display));
            } else if arg == "auto" {
                cfg.aspect_ratio = "".into();
                state.push_output("Aspect ratio: auto (uses window size)".to_string());
            } else if arg.contains(':') && crate::game::hud::parse_aspect_ratio(arg).is_some() {
                cfg.aspect_ratio = arg.to_string();
                state.push_output(format!("Aspect ratio: {}", arg));
            } else {
                state.push_output("Usage: aspect <auto|4:3|16:9|21:9>".to_string());
            }
        }

        "vsync" => {
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

        "fps_cap" => {
            if arg.is_empty() {
                let cap = if cfg.fps_cap == 0 { "unlimited".to_string() } else { format!("{}", cfg.fps_cap) };
                state.push_output(format!("FPS cap: {}", cap));
            } else if let Ok(v) = arg.parse::<u32>() {
                cfg.fps_cap = v;
                let cap = if v == 0 { "unlimited".to_string() } else { format!("{}", v) };
                state.push_output(format!("FPS cap: {}", cap));
            } else {
                state.push_output("Usage: fps_cap <0=unlimited|30|60|120|...>".to_string());
            }
        }

        // --- Debug ---
        "debug" => {
            cfg.debug = match arg {
                "on" | "1" => true,
                "off" | "0" => false,
                _ => !cfg.debug,
            };
            debug_config.show_play_area = cfg.debug;
            state.push_output(format!("Debug HUD: {}", if cfg.debug { "on" } else { "off" }));
        }

        // --- System ---
        "exit" | "quit" | "q" => {
            exit.write(AppExit::from_code(0));
        }

        "clear" | "cls" => {
            state.output.clear();
            state.generation += 1;
        }

        "save_cfg" => {
            match cfg.save() {
                Ok(()) => state.push_output(format!("Config saved to {}", cfg.config_path.display())),
                Err(e) => state.push_output(e),
            }
        }

        "help" | "?" => {
            state.push_output("Map:".to_string());
            state.push_output("  load <map> [x,z] - Load map (e.g. load oute3, load d01 100,200)".to_string());
            state.push_output("  load n/s/e/w     - Adjacent map (keeps position)".to_string());
            state.push_output("  pos              - Show current position and map".to_string());
            state.push_output("Graphics:".to_string());
            state.push_output("  aa <mode>        - Set AA (msaa2/4/8|fxaa|smaa|taa|off) *reload".to_string());
            state.push_output("  tonemap <mode>   - Tonemapping *reload".to_string());
            state.push_output("  wireframe        - Toggle wireframe".to_string());
            state.push_output("  shadows [on|off] - Toggle shadows".to_string());
            state.push_output("  bloom [on|off|N] - Bloom / intensity *reload".to_string());
            state.push_output("  ssao [on|off]    - Ambient occlusion *reload".to_string());
            state.push_output("  fog <start> <end> - Set fog range".to_string());
            state.push_output("  dd <distance>    - Set draw distance".to_string());
            state.push_output("  exposure <N>     - Set exposure (-4 to 4)".to_string());
            state.push_output("  dof [on|off|N]   - Depth of field / focal distance".to_string());
            state.push_output("  (* = applies on map reload)".to_string());
            state.push_output("Gameplay:".to_string());
            state.push_output("  fly [on|off]     - Toggle fly mode".to_string());
            state.push_output("  speed <N>        - Set turn speed".to_string());
            state.push_output("  sens <N>         - Set mouse sensitivity".to_string());
            state.push_output("Window:".to_string());
            state.push_output("  fullscreen       - Fullscreen mode".to_string());
            state.push_output("  borderless       - Borderless fullscreen".to_string());
            state.push_output("  windowed         - Windowed mode".to_string());
            state.push_output("  aspect <auto|4:3|16:9> - Set aspect ratio".to_string());
            state.push_output("  vsync <auto|fast|off> - Set vsync".to_string());
            state.push_output("  fps_cap <N>      - Set FPS cap (0=unlimited)".to_string());
            state.push_output("System:".to_string());
            state.push_output("  debug [on|off]   - Toggle debug HUD".to_string());
            state.push_output("  save_cfg         - Save config to disk".to_string());
            state.push_output("  clear            - Clear console".to_string());
            state.push_output("  quit             - Exit game".to_string());
        }

        _ => {
            state.push_output(format!("Unknown command: '{}'. Type 'help'.", command));
        }
    }
}

/// Resolve a directional load — returns the target map and the player's position
/// offset into the new map's coordinate space (same logic as boundary crossing).
fn resolve_direction(
    current_map: &CurrentMapName,
    player_pos: Vec3,
    dir_fn: fn(&OdmName) -> Option<OdmName>,
    x_offset: f32,
    z_offset: f32,
) -> Result<(MapName, [f32; 3]), String> {
    match &current_map.0 {
        MapName::Outdoor(odm) => match dir_fn(odm) {
            Some(next) => {
                let pos = [player_pos.x + x_offset, player_pos.y, player_pos.z + z_offset];
                Ok((MapName::Outdoor(next), pos))
            }
            None => Err("No map in that direction.".to_string()),
        },
        MapName::Indoor(_) => Err("Directional navigation only works on outdoor maps.".to_string()),
    }
}

/// Parse "x,z" coordinate string into Bevy [x, y, z] position.
fn parse_coords(s: &str) -> Option<[f32; 3]> {
    let mut parts = s.split(',');
    let x: f32 = parts.next()?.trim().parse().ok()?;
    let z: f32 = parts.next()?.trim().parse().ok()?;
    Some([x, 0.0, z])
}

/// Parse fog range from one or two args: "start end" or "start" "end".
fn parse_fog_range(first: &str, second: Option<&str>) -> Option<(f32, f32)> {
    let start: f32 = first.parse().ok()?;
    let end: f32 = second?.parse().ok()?;
    Some((start, end))
}

/// Update console text when state changes.
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

    // Update console position on resize
    if let Ok(window) = windows.single() {
        let (left, top, vp_w, vp_h) = viewport_inner_rect(&window, &cfg, &ui_assets);
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

    // Update output text
    if let Ok(mut text) = output_q.single_mut() {
        **text = state.output.join("\n");
    }

    // Update prompt
    if let Ok(mut text) = prompt_q.single_mut() {
        **text = format!("> {}_", state.input);
    }
}

/// Hide the debug HUD (FPS counter etc.) while the console is open.
fn toggle_debug_hud(
    state: Res<ConsoleState>,
    cfg: Res<GameConfig>,
    mut debug_q: Query<&mut Visibility, With<DebugHud>>,
) {
    let vis = if state.open {
        Visibility::Hidden
    } else if cfg.debug {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut v in debug_q.iter_mut() {
        *v = vis;
    }
}

/// Sync GameConfig values to live scene components so console changes take effect immediately.
fn sync_config_to_scene(
    cfg: Res<GameConfig>,
    mut commands: Commands,
    mut sun_q: Query<&mut DirectionalLight>,
    mut fog_q: Query<&mut DistanceFog>,
    mut exposure_q: Query<&mut bevy::camera::Exposure>,
    camera_q: Query<(Entity, Option<&bevy::post_process::dof::DepthOfField>), With<Camera3d>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    // Shadows
    for mut light in sun_q.iter_mut() {
        light.shadows_enabled = cfg.shadows;
    }

    // Fog distances
    for mut fog in fog_q.iter_mut() {
        fog.falloff = FogFalloff::Linear {
            start: cfg.fog_start,
            end: cfg.fog_end,
        };
    }

    // Exposure
    for mut exp in exposure_q.iter_mut() {
        exp.ev100 = 9.7 + cfg.exposure;
    }

    // Depth of field
    for (entity, existing_dof) in camera_q.iter() {
        if cfg.depth_of_field {
            let dof = bevy::post_process::dof::DepthOfField {
                focal_distance: cfg.depth_of_field_distance,
                ..default()
            };
            commands.entity(entity).insert(dof);
        } else if existing_dof.is_some() {
            commands.entity(entity).remove::<bevy::post_process::dof::DepthOfField>();
        }
    }

    // Window mode, VSync, FPS cap
    if let Ok(mut window) = windows.single_mut() {
        // Window mode
        let target_mode = match cfg.window_mode.as_str() {
            "fullscreen" => WindowMode::Fullscreen(
                bevy::window::MonitorSelection::Current,
                bevy::window::VideoModeSelection::Current,
            ),
            "borderless" => WindowMode::BorderlessFullscreen(
                bevy::window::MonitorSelection::Current,
            ),
            _ => WindowMode::Windowed,
        };
        if std::mem::discriminant(&window.mode) != std::mem::discriminant(&target_mode) {
            window.mode = target_mode;
        }

        // VSync / FPS cap → present mode
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
