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
use crate::game::outdoor::OdmName;
use crate::game::ui_assets::UiAssets;
use crate::game::viewport::viewport_inner_rect;
use crate::game::world::WorldState;
use crate::save::GameSave;
use openmm_data::utils::MapName;

mod commands;

const FONT_SIZE: f32 = 16.0;
const MAX_OUTPUT_LINES: usize = 50;
pub(super) const NEEDS_RELOAD: &str = "(run 'reload' to apply)";
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
    mut game_time: ResMut<crate::game::world::GameTime>,
    mut speed_mul: ResMut<crate::game::player::SpeedMultiplier>,
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
                        &mut speed_mul,
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

/// Thin dispatcher: parses the command name and calls the matching handler in
/// `commands`. Each handler takes only the state it actually needs — add a new
/// command by writing a `cmd_foo` function in `commands.rs` and adding an arm
/// here.
#[allow(clippy::too_many_arguments)]
fn execute_command(
    cmd: &str,
    state: &mut ConsoleState,
    exit: &mut MessageWriter<AppExit>,
    world: &mut WorldState,
    save_data: &mut GameSave,
    cmds: &mut Commands,
    game_state: &mut NextState<GameState>,
    cfg: &mut GameConfig,
    wireframe_config: &mut WireframeConfig,
    game_assets: &crate::GameAssets,
    game_time: &mut crate::game::world::GameTime,
    speed_mul: &mut crate::game::player::SpeedMultiplier,
) {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    let Some(&command) = parts.first() else { return };
    let arg = parts.get(1).copied().unwrap_or("");

    state.push_output(format!("> {}", cmd));

    match command {
        // Map loading
        "reload" => commands::cmd_reload(state, world, save_data, cmds, game_state),
        "load" | "map" => commands::cmd_load(state, &parts, world, save_data, cmds, game_state, game_assets),

        // Graphics
        "msaa" | "aa" => commands::cmd_msaa(state, cfg, arg),
        "tonemapping" | "tonemap" => commands::cmd_tonemap(state, cfg, arg),
        "wireframe" | "wf" => commands::cmd_wireframe(state, wireframe_config),
        "shadows" => commands::cmd_shadows(state, cfg, arg),
        "speed" => commands::cmd_speed(state, speed_mul, arg),
        "bloom" => commands::cmd_bloom(state, cfg, arg),
        "ssao" => commands::cmd_ssao(state, cfg, arg),
        "fog" => commands::cmd_fog(state, cfg, &parts),
        "draw_distance" | "dd" => commands::cmd_draw_distance(state, cfg, arg),
        "exposure" => commands::cmd_exposure(state, cfg, arg),
        "dof" | "depth_of_field" => commands::cmd_dof(state, cfg, arg),

        // Inventory / quest bits
        "item" => commands::cmd_item(state, world, &parts),
        "qbit" => commands::cmd_qbit(state, world, &parts),

        // Gameplay
        "fly" => commands::cmd_fly(state, world, arg),
        "turn_speed" => commands::cmd_turn_speed(state, cfg, arg),
        "sensitivity" | "sens" => commands::cmd_sensitivity(state, cfg, arg),
        "pos" => commands::cmd_pos(state, world),

        // Window
        "fullscreen" | "fs" => commands::cmd_fullscreen(state, cfg),
        "borderless" => commands::cmd_borderless(state, cfg),
        "windowed" | "window" => commands::cmd_windowed(state, cfg),
        "aspect" | "aspect_ratio" | "ar" => commands::cmd_aspect(state, cfg, arg),
        "vsync" => commands::cmd_vsync(state, cfg, arg),
        "fps_cap" => commands::cmd_fps_cap(state, cfg, arg),

        // Audio
        "mute" => commands::cmd_mute(state, cfg),
        "unmute" => commands::cmd_unmute(state, cfg),
        "music" => commands::cmd_music(state, cfg, arg),
        "sfx" => commands::cmd_sfx(state, cfg, arg),
        "volume" | "vol" => commands::cmd_volume(state, cfg, arg),

        // System
        "debug" => commands::cmd_debug(state, world, cfg, arg),
        "lighting" => commands::cmd_lighting(state, cfg, arg),
        "filtering" => commands::cmd_filtering(state, cfg, arg),
        "terrain_filtering" | "tf" => commands::cmd_terrain_filtering(state, cfg, arg),
        "models_filtering" | "mf" => commands::cmd_models_filtering(state, cfg, arg),
        "hud_filtering" | "hf" => commands::cmd_hud_filtering(state, cfg, arg),
        "exit" | "quit" | "q" => commands::cmd_exit(exit),
        "clear" | "cls" => commands::cmd_clear(state),
        "save_cfg" => commands::cmd_save_cfg(state, cfg),
        "time" => commands::cmd_time(state, game_time, &parts),
        "help" | "?" => commands::cmd_help(state),

        _ => commands::cmd_unknown(state, command),
    }
}

// --- Shared helpers (used by command handlers) ---

pub(super) fn set_filtering(state: &mut ConsoleState, field: &mut String, label: &str, arg: &str) {
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

pub(super) fn parse_toggle(arg: &str, current: bool) -> bool {
    match arg {
        "on" | "1" => true,
        "off" | "0" => false,
        _ => !current,
    }
}

pub(super) const HELP_TEXT: &[&str] = &[
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

pub(super) fn resolve_direction(
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

pub(super) fn parse_coords(s: &str) -> Option<[f32; 3]> {
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
