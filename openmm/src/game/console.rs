//! Drop-down developer console (Quake-style, toggled with Tab).
//!
//! Renders a semi-transparent overlay at the top of the viewport with scrollable
//! output lines and a text input prompt. Uses Bevy native text for crisp rendering.

use bevy::input::ButtonState;
use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowMode};

use lod::evt::GameEvent;

use crate::GameState;
use crate::game::InGame;
use crate::game::debug::{CurrentMapName, DebugHud};
use crate::game::event_dispatch::EventQueue;
use crate::game::map_name::MapName;
use crate::game::odm::OdmName;
use crate::game::hud::viewport_inner_rect;
use crate::config::GameConfig;
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
    /// Index into history for up/down navigation (-1 = current input).
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
                (toggle_console, console_input, update_console_ui, toggle_debug_hud)
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
    if !keys.just_pressed(KeyCode::Tab) {
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
    mut event_queue: ResMut<EventQueue>,
    mut camera_msaa: Query<&mut Msaa, With<crate::game::player::PlayerCamera>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut exit: MessageWriter<AppExit>,
    current_map: Res<CurrentMapName>,
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
                    execute_command(
                        &cmd,
                        &mut state,
                        &mut event_queue,
                        &mut camera_msaa,
                        &mut windows,
                        &mut exit,
                        &current_map,
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

fn execute_command(
    cmd: &str,
    state: &mut ConsoleState,
    event_queue: &mut EventQueue,
    camera_msaa: &mut Query<&mut Msaa, With<crate::game::player::PlayerCamera>>,
    windows: &mut Query<&mut Window, With<PrimaryWindow>>,
    exit: &mut MessageWriter<AppExit>,
    current_map: &CurrentMapName,
) {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    let Some(&command) = parts.first() else { return };
    let arg = parts.get(1).copied().unwrap_or("");

    state.push_output(format!("> {}", cmd));

    match command {
        "load" | "map" => {
            if arg.is_empty() {
                state.push_output("Usage: load <map|north|south|east|west>".to_string());
            } else {
                // Check for directional loading or validate map name
                let resolved = match arg {
                    "north" | "n" => resolve_direction(current_map, OdmName::go_north),
                    "south" | "s" => resolve_direction(current_map, OdmName::go_south),
                    "east" | "e" => resolve_direction(current_map, OdmName::go_east),
                    "west" | "w" => resolve_direction(current_map, OdmName::go_west),
                    name => match MapName::try_from(name) {
                        Ok(_) => Ok(name.to_string()),
                        Err(e) => Err(format!("Invalid map name '{}': {}", name, e)),
                    },
                };

                match resolved {
                    Ok(map_name) => {
                        state.push_output(format!("Loading map: {}", map_name));
                        state.open = false;
                        event_queue.push(GameEvent::MoveToMap {
                            x: 0,
                            y: 0,
                            z: 0,
                            direction: 0,
                            map_name,
                        });
                    }
                    Err(msg) => state.push_output(msg),
                }
            }
        }

        "msaa" => {
            if let Ok(mut msaa) = camera_msaa.single_mut() {
                match arg {
                    "off" | "0" => {
                        *msaa = Msaa::Off;
                        state.push_output("MSAA: off".to_string());
                    }
                    "2" => {
                        *msaa = Msaa::Sample2;
                        state.push_output("MSAA: 2x".to_string());
                    }
                    "4" => {
                        *msaa = Msaa::Sample4;
                        state.push_output("MSAA: 4x".to_string());
                    }
                    "8" => {
                        *msaa = Msaa::Sample8;
                        state.push_output("MSAA: 8x".to_string());
                    }
                    _ => {
                        state.push_output(format!("Current: {:?}", *msaa));
                        state.push_output("Usage: msaa <off|2|4|8>".to_string());
                    }
                }
            } else {
                state.push_output("No camera found".to_string());
            }
        }

        "fullscreen" | "fs" => {
            if let Ok(mut window) = windows.single_mut() {
                window.mode = WindowMode::Fullscreen(
                    bevy::window::MonitorSelection::Current,
                    bevy::window::VideoModeSelection::Current,
                );
                state.push_output("Fullscreen enabled".to_string());
            }
        }

        "borderless" => {
            if let Ok(mut window) = windows.single_mut() {
                window.mode = WindowMode::BorderlessFullscreen(
                    bevy::window::MonitorSelection::Current,
                );
                state.push_output("Borderless fullscreen enabled".to_string());
            }
        }

        "windowed" | "window" => {
            if let Ok(mut window) = windows.single_mut() {
                window.mode = WindowMode::Windowed;
                state.push_output("Windowed mode enabled".to_string());
            }
        }

        "exit" | "quit" => {
            exit.write(AppExit::from_code(0));
        }

        "help" => {
            state.push_output("Commands:".to_string());
            state.push_output("  load <map>       - Load map (e.g. load oute3, load d01)".to_string());
            state.push_output("  load north/south/east/west - Move to adjacent outdoor map".to_string());
            state.push_output("  msaa <off|2|4|8> - Set anti-aliasing".to_string());
            state.push_output("  fullscreen       - Fullscreen mode".to_string());
            state.push_output("  borderless       - Borderless fullscreen".to_string());
            state.push_output("  windowed         - Windowed mode".to_string());
            state.push_output("  exit             - Quit the game".to_string());
        }

        _ => {
            state.push_output(format!("Unknown command: '{}'. Type 'help'.", command));
        }
    }
}

/// Resolve a directional load (north/south/east/west) from the current outdoor map.
fn resolve_direction(
    current_map: &CurrentMapName,
    dir_fn: fn(&OdmName) -> Option<OdmName>,
) -> Result<String, String> {
    match &current_map.0 {
        MapName::Outdoor(odm) => match dir_fn(odm) {
            Some(next) => Ok(next.to_string().trim_end_matches(".odm").to_string()),
            None => Err("No map in that direction.".to_string()),
        },
        MapName::Indoor(_) => Err("Directional navigation only works on outdoor maps.".to_string()),
    }
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
