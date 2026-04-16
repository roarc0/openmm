//! User interaction: hover, click, keyboard, pulse animation, and action processing.

use bevy::ecs::message::{MessageReader, MessageWriter};
use bevy::prelude::*;

use super::runtime::{
    ClickFlash, HiddenByDefault, HoverOverlay, Pulsable, Pulsing, RuntimeElement, ScreenActions, ScreenLayer,
    ScreenLayers, ScreenUiHovered,
};
use super::setup::{hide_screen, load_screen_replace_all, show_screen};
use crate::GameState;
use crate::assets::GameAssets;
use crate::config::GameConfig;
use crate::game::optional::OptionalWrite;
use crate::game::ui_assets::UiAssets;

// ── Interaction systems ─────────────────────────────────────────────────────

pub(super) fn screen_hover(
    query: Query<(&Interaction, &Children), (Changed<Interaction>, With<RuntimeElement>)>,
    mut hover_query: Query<&mut Visibility, With<HoverOverlay>>,
) {
    for (interaction, children) in &query {
        let show = matches!(interaction, Interaction::Hovered | Interaction::Pressed);
        for child in children.iter() {
            if let Ok(mut vis) = hover_query.get_mut(child) {
                *vis = if show {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
        }
    }
}

/// Dispatch on_hover actions via PendingActions on hover start.
/// Only active when the cursor is free (not grabbed by gameplay crosshair).
/// Maintains ScreenUiHovered flag every frame (not just on change)
/// so the world interaction system doesn't clear the footer while hovering.
pub(super) fn hover_actions(
    changed_query: Query<(&Interaction, &RuntimeElement), Changed<Interaction>>,
    all_query: Query<(&Interaction, &RuntimeElement)>,
    layers: Res<ScreenLayers>,
    mut ui_hovered: ResMut<ScreenUiHovered>,
    mut actions: Option<MessageWriter<ScreenActions>>,
    cursor_query: Query<&bevy::window::CursorOptions, With<bevy::window::PrimaryWindow>>,
) {
    // Skip screen hover when cursor is grabbed (crosshair mode).
    let cursor_free = cursor_query
        .single()
        .is_ok_and(|c| matches!(c.grab_mode, bevy::window::CursorGrabMode::None));
    if !cursor_free {
        ui_hovered.0 = false;
        return;
    }

    // Check ALL hovered elements (not just changed) to keep the flag stable.
    let any_hovered = all_query.iter().any(|(interaction, rt_elem)| {
        matches!(interaction, Interaction::Hovered | Interaction::Pressed)
            && layers.screens.get(&rt_elem.screen_id).is_some_and(|screen| {
                screen.elements[rt_elem.index]
                    .on_hover()
                    .iter()
                    .any(|a| a.trim() != "PulseSprite()")
            })
    });
    ui_hovered.0 = any_hovered;

    // Dispatch actions only on hover start (Changed<Interaction>).
    for (interaction, rt_elem) in &changed_query {
        if *interaction != Interaction::Hovered {
            continue;
        }
        let Some(screen) = layers.screens.get(&rt_elem.screen_id) else {
            continue;
        };
        let hover_actions: Vec<String> = screen.elements[rt_elem.index]
            .on_hover()
            .iter()
            .filter(|a| a.trim() != "PulseSprite()")
            .cloned()
            .collect();
        if !hover_actions.is_empty() {
            actions.try_write(ScreenActions { actions: hover_actions });
        }
    }
}

/// Start/stop pulsing on hover for Pulsable elements.
/// Checks that the element's screen layer is still active to avoid
/// inserting on entities queued for despawn by a screen transition.
pub(super) fn pulse_hover(
    mut commands: Commands,
    query: Query<(Entity, &Interaction, Has<HiddenByDefault>, &ScreenLayer), (Changed<Interaction>, With<Pulsable>)>,
    pulsing_query: Query<&Pulsing>,
    mut image_query: Query<&mut ImageNode>,
    layers: Res<ScreenLayers>,
) {
    for (entity, interaction, hidden_default, layer) in &query {
        // Skip entities whose screen was just replaced (despawn is deferred).
        if !layers.screens.contains_key(&layer.0) {
            continue;
        }
        let hovering = matches!(interaction, Interaction::Hovered | Interaction::Pressed);
        if hovering && !pulsing_query.contains(entity) {
            commands
                .entity(entity)
                .try_insert((Pulsing { elapsed: 0.0 }, Visibility::Inherited));
        } else if !hovering && pulsing_query.contains(entity) {
            commands.entity(entity).remove::<Pulsing>();
            if let Ok(mut img) = image_query.get_mut(entity) {
                img.color = img.color.with_alpha(1.0);
            }
            if hidden_default {
                commands.entity(entity).try_insert(Visibility::Hidden);
            }
        }
    }
}

/// Animate alpha on pulsing elements: smooth 0->1->0 each second via sine wave.
pub(super) fn pulse_animate(time: Res<Time>, mut query: Query<(&mut Pulsing, &mut ImageNode)>) {
    for (mut pulse, mut img) in &mut query {
        pulse.elapsed += time.delta_secs();
        // sin gives -1..1, remap to 0..1. Full cycle = 1 second (2pi per second).
        let alpha = (pulse.elapsed * std::f32::consts::TAU).sin() * 0.5 + 0.5;
        img.color = img.color.with_alpha(alpha);
    }
}

/// On click: hide element briefly, then fire actions after the flash.
pub(super) fn screen_click(
    mut commands: Commands,
    query: Query<(Entity, &Interaction, &RuntimeElement), (Changed<Interaction>, With<Button>)>,
    layers: Res<ScreenLayers>,
    flash_query: Query<&ClickFlash>,
) {
    for (entity, interaction, rt_elem) in &query {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if flash_query.contains(entity) {
            continue;
        }
        let Some(screen) = layers.screens.get(&rt_elem.screen_id) else {
            continue;
        };
        let elem = &screen.elements[rt_elem.index];
        if elem.on_click().is_empty() {
            continue;
        }

        info!("screen click [{}/{}]", rt_elem.screen_id, elem.id());
        commands.entity(entity).insert((
            Visibility::Hidden,
            ClickFlash {
                timer: Timer::from_seconds(0.15, TimerMode::Once),
                pending_actions: elem.on_click().to_vec(),
            },
        ));
    }
}

/// Check keyboard shortcuts defined in all active screens.
pub(super) fn screen_keys(
    _commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    layers: Res<ScreenLayers>,
    mut actions: Option<MessageWriter<ScreenActions>>,
) {
    // Check keyboard shortcuts defined in all active screens.
    for screen in layers.screens.values() {
        for (key_name, action_strings) in &screen.keys {
            if let Some(code) = crate::input::parse_key_code(key_name)
                && keys.just_pressed(code)
            {
                info!("screen key [{}]: {}", screen.id, key_name);
                actions.try_write(ScreenActions {
                    actions: action_strings.clone(),
                });
                return; // one key per frame
            }
        }
    }
}

/// Tick flash timers -- collect actions into PendingActions for deferred processing.
pub(super) fn click_flash_tick(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut ClickFlash, &mut Visibility)>,
    mut actions: Option<MessageWriter<ScreenActions>>,
) {
    for (entity, mut flash, mut vis) in &mut query {
        flash.timer.tick(time.delta());
        if !flash.timer.just_finished() {
            continue;
        }

        *vis = Visibility::Inherited;
        let p_actions: Vec<String> = flash.pending_actions.drain(..).collect();
        commands.entity(entity).remove::<ClickFlash>();

        if !p_actions.is_empty() {
            actions.try_write(ScreenActions { actions: p_actions });
        }
    }
}

/// Process queued actions with full system access (commands, layers, entities, exit).
/// Uses the scripting executor for Compare/Else/End control flow.
pub(super) fn process_pending_actions(
    mut commands: Commands,
    mut actions_set: ParamSet<(MessageReader<ScreenActions>, Option<MessageWriter<ScreenActions>>)>,
    mut layers: ResMut<ScreenLayers>,
    layer_entities: Query<(Entity, &ScreenLayer)>,
    mut sprite_query: Query<(&RuntimeElement, &mut Visibility)>,
    mut cfg: ResMut<GameConfig>,
    game_assets: Res<GameAssets>,
    mut ui_assets: ResMut<UiAssets>,
    mut images: ResMut<Assets<Image>>,
    mut audio_sources: ResMut<Assets<AudioSource>>,
    mut exit_writer: bevy::ecs::message::MessageWriter<bevy::app::AppExit>,
    world_state: Option<Res<crate::game::world::WorldState>>,
    mut event_queue: Option<ResMut<crate::game::world::scripting::EventQueue>>,
    _time: Res<Time>,
) {
    use super::scripting::Action;

    let actions_to_process: Vec<ScreenActions> = actions_set.p0().read().cloned().collect();
    let mut actions = actions_set.p1();

    for event in actions_to_process {
        let action_strings = event.actions.clone();

        // Build script context from available resources.
        let default_vars = crate::game::world::state::GameVariables::default();
        let vars = world_state.as_ref().map(|ws| &ws.game_vars).unwrap_or(&default_vars);
        let config_flags = build_config_flags(&cfg);
        let ctx = super::scripting::ScriptContext {
            vars,
            config_flags: &config_flags,
        };

        let actions_list = super::scripting::execute_actions(&action_strings, &ctx);

        for action in actions_list {
            match action {
                Action::Quit => {
                    info!("action: Quit");
                    exit_writer.write(bevy::app::AppExit::Success);
                }
                Action::NewGame => {
                    info!("action: NewGame");
                    commands.set_state(GameState::Loading);
                }
                Action::LoadScreen(id) => {
                    info!("action: LoadScreen(\"{}\")", id);
                    load_screen_replace_all(
                        &id,
                        &mut commands,
                        &mut layers,
                        &layer_entities,
                        &mut ui_assets,
                        &game_assets,
                        &mut images,
                        &mut audio_sources,
                        &cfg,
                        &mut actions,
                    );
                }
                Action::ShowScreen(id) => {
                    info!("action: ShowScreen(\"{}\")", id);
                    show_screen(
                        &id,
                        &mut commands,
                        &mut layers,
                        &mut ui_assets,
                        &game_assets,
                        &mut images,
                        &mut audio_sources,
                        &cfg,
                        &mut actions,
                    );
                }
                Action::HideScreen(id) => {
                    info!("action: HideScreen(\"{}\")", id);
                    hide_screen(&id, &mut commands, &mut layers, &layer_entities);
                }
                Action::ShowSprite(ref id) => {
                    for (elem, mut vis) in &mut sprite_query {
                        if elem.element_id == *id {
                            *vis = Visibility::Inherited;
                        }
                    }
                }
                Action::HideSprite(ref id) => {
                    for (elem, mut vis) in &mut sprite_query {
                        if elem.element_id == *id {
                            *vis = Visibility::Hidden;
                        }
                    }
                }
                Action::PulseSprite => {} // handled at spawn time
                Action::EvtProxy(evt_str) => {
                    if let Some(ref mut eq) = event_queue {
                        proxy_evt_action(&evt_str, eq);
                    }
                }
                Action::SaveConfig(key, value) => {
                    info!("action: SaveConfig(\"{}\", \"{}\")", key, value);
                    match key.as_str() {
                        "skipIntro" | "skip_intro" => {
                            cfg.skip_intro = value == "true";
                        }
                        "skipLogo" | "skip_logo" => {
                            cfg.skip_logo = value == "true";
                        }
                        "debug" => {
                            cfg.debug = value == "true";
                        }
                        _ => {
                            warn!("SaveConfig: unknown key '{}'", key);
                        }
                    }
                    if let Err(e) = cfg.save() {
                        error!("SaveConfig: failed to save: {}", e);
                    }
                }
                Action::Unknown(s) => {
                    warn!("unknown screen action: '{}'", s);
                }
                Action::Compare(_) | Action::Else | Action::End => {} // consumed by execute_actions
            }
        }
    }
}

/// Build config flags set from GameConfig for condition evaluation.
fn build_config_flags(cfg: &GameConfig) -> std::collections::HashSet<String> {
    let mut flags = std::collections::HashSet::new();
    if cfg.skip_intro {
        flags.insert("skip_intro".into());
    }
    if cfg.skip_logo {
        flags.insert("skip_logo".into());
    }
    if cfg.debug {
        flags.insert("debug".into());
    }
    if cfg.console {
        flags.insert("console".into());
    }
    flags
}

/// Proxy an `evt:` action string to the EVT EventQueue.
fn proxy_evt_action(evt_str: &str, event_queue: &mut crate::game::world::scripting::EventQueue) {
    use openmm_data::evt::GameEvent;

    let s = evt_str.trim();

    // PlaySound(id)
    if let Some(rest) = s.strip_prefix("PlaySound(").and_then(|r| r.strip_suffix(')'))
        && let Ok(id) = rest.trim().parse::<u32>()
    {
        event_queue.push_single(GameEvent::PlaySound { sound_id: id });
        return;
    }

    // Hint("text")
    if let Some(text) = super::scripting::parse_string_arg(s, "Hint") {
        event_queue.push_single(GameEvent::Hint {
            str_id: 0,
            text: text.to_string(),
        });
        return;
    }

    // StatusText("text")
    if let Some(text) = super::scripting::parse_string_arg(s, "StatusText") {
        event_queue.push_single(GameEvent::StatusText {
            str_id: 0,
            text: text.to_string(),
        });
        return;
    }

    warn!("evt: unknown proxy action: '{}'", s);
}
