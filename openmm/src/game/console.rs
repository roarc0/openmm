//! Drop-down developer console (Quake-style, toggled with Tab).
//!
//! Renders a semi-transparent overlay at the top of the viewport with a text input line.
//! Supports commands like `load oute3` to fire game events.

use bevy::input::ButtonState;
use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowMode};

use lod::evt::GameEvent;

use crate::GameState;
use crate::fonts::GameFonts;
use crate::game::InGame;
use crate::game::event_dispatch::EventQueue;
use crate::game::hud::viewport_inner_rect;
use crate::config::GameConfig;
use crate::ui_assets::UiAssets;

/// Whether the console is open.
#[derive(Resource, Default)]
pub struct ConsoleState {
    pub open: bool,
    pub input: String,
    /// History of executed commands (most recent last).
    history: Vec<String>,
    /// Lines of output (command results, errors).
    output: Vec<String>,
    /// Generation counter for re-rendering.
    generation: u64,
}

impl ConsoleState {
    fn push_output(&mut self, line: String) {
        self.output.push(line);
        // Keep last 20 lines
        if self.output.len() > 20 {
            self.output.remove(0);
        }
        self.generation += 1;
    }
}

/// Marker for the console UI root node.
#[derive(Component)]
struct ConsoleUI;

/// Marker for the console text image.
#[derive(Component)]
struct ConsoleText;

pub struct ConsolePlugin;

impl Plugin for ConsolePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ConsoleState>()
            .add_systems(
                Update,
                (toggle_console, console_input, update_console_ui)
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
        // Spawn console UI
        let Ok(window) = windows.single() else { return };
        let (left, top, vp_w, vp_h) = viewport_inner_rect(&window, &cfg, &ui_assets);
        let console_h = vp_h * 0.4; // 40% of viewport height

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
                    padding: UiRect::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
                GlobalZIndex(100),
                ConsoleUI,
                InGame,
            ))
            .with_children(|parent| {
                // Text display area
                parent.spawn((
                    Name::new("console_text"),
                    ImageNode::new(Handle::default()),
                    Node {
                        width: Val::Auto,
                        height: Val::Auto,
                        ..default()
                    },
                    Visibility::Hidden,
                    ConsoleText,
                ));
            });

        state.generation += 1; // Force re-render
    } else {
        // Despawn console UI
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
                    execute_command(&cmd, &mut state, &mut event_queue, &mut camera_msaa, &mut windows, &mut exit);
                    state.input.clear();
                }
            }
            KeyCode::Backspace => {
                state.input.pop();
                state.generation += 1;
            }
            KeyCode::Tab => {
                // Consumed by toggle, ignore here
            }
            _ => {
                // Append typed text
                if let Some(ref text) = event.text {
                    let s = text.as_str();
                    // Filter out control characters
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
) {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    let Some(&command) = parts.first() else { return };
    let arg = parts.get(1).copied().unwrap_or("");

    state.push_output(format!("> {}", cmd));

    match command {
        "load" | "map" => {
            if arg.is_empty() {
                state.push_output("Usage: load <map> (e.g. load oute3)".to_string());
            } else {
                state.push_output(format!("Loading map: {}", arg));
                event_queue.push(GameEvent::MoveToMap {
                    x: 0,
                    y: 0,
                    z: 0,
                    direction: 0,
                    map_name: arg.to_string(),
                });
            }
        }

        "msaa" => {
            if let Ok(mut msaa) = camera_msaa.single_mut() {
                match arg {
                    "off" | "0" => { *msaa = Msaa::Off; state.push_output("MSAA: off".to_string()); }
                    "2" => { *msaa = Msaa::Sample2; state.push_output("MSAA: 2x".to_string()); }
                    "4" => { *msaa = Msaa::Sample4; state.push_output("MSAA: 4x".to_string()); }
                    "8" => { *msaa = Msaa::Sample8; state.push_output("MSAA: 8x".to_string()); }
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
            state.push_output("  load <map>     - Load map (e.g. load oute3)".to_string());
            state.push_output("  msaa <off|2|4|8> - Set anti-aliasing".to_string());
            state.push_output("  fullscreen     - Fullscreen mode".to_string());
            state.push_output("  borderless     - Borderless fullscreen".to_string());
            state.push_output("  windowed       - Windowed mode".to_string());
            state.push_output("  exit           - Quit the game".to_string());
        }

        _ => {
            state.push_output(format!("Unknown command: '{}'. Type 'help'.", command));
        }
    }
}

/// Re-render console text when state changes.
fn update_console_ui(
    state: Res<ConsoleState>,
    mut last_gen: Local<u64>,
    game_fonts: Res<GameFonts>,
    mut images: ResMut<Assets<Image>>,
    mut query: Query<(&mut ImageNode, &mut Visibility, &mut Node), With<ConsoleText>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cfg: Res<GameConfig>,
    ui_assets: Res<UiAssets>,
    // Update console position on resize
    mut console_q: Query<
        &mut Node,
        (With<ConsoleUI>, Without<ConsoleText>),
    >,
) {
    if !state.open {
        return;
    }

    // Update console position on resize
    if let Ok(window) = windows.single() {
        let (left, top, vp_w, vp_h) = viewport_inner_rect(&window, &cfg, &ui_assets);
        let console_h = vp_h * 0.4;
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

    // Render prompt line: "> input_"
    let prompt = format!("> {}_", state.input);
    let font = "smallnum";
    let color = crate::fonts::WHITE;

    for (mut img_node, mut vis, _node) in query.iter_mut() {
        if let Some(handle) = game_fonts.render(&prompt, font, color, &mut images) {
            img_node.image = handle;
            *vis = Visibility::Inherited;
        } else {
            *vis = Visibility::Hidden;
        }
    }
}
