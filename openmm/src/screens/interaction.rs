//! User interaction: hover, click, keyboard, pulse animation, and action processing.

use bevy::ecs::message::{MessageReader, MessageWriter};
use bevy::prelude::*;
use bevy::window::{CursorOptions, PrimaryWindow};

use super::runtime::{
    ClickFlash, ClickedAnimation, ClickedTexture, HoverAnimation, HoverOverlay, HoverTexture,
    RuntimeElement, ScreenActions, ScreenLayer, ScreenLayers, ScreenUiHovered,
};
use super::setup::{hide_screen, load_screen_replace_all, show_screen};
use super::ui_assets::UiAssets;
use crate::GameState;
use crate::assets::GameAssets;
use crate::game::optional::OptionalWrite;
use crate::system::config::GameConfig;

pub(super) fn screen_hover(
    mut query: Query<
        (
            &Interaction,
            Option<&Children>,
            Option<&mut ImageNode>,
            Option<&HoverTexture>,
            Option<&mut HoverAnimation>,
            Option<&super::runtime::FrameAnimation>,
        ),
        (Changed<Interaction>, With<RuntimeElement>),
    >,
    mut hover_overlay_query: Query<&mut Visibility, With<HoverOverlay>>,
    mut child_image_query: Query<
        (
            &mut ImageNode,
            Option<&HoverTexture>,
            Option<&mut HoverAnimation>,
            Option<&super::runtime::FrameAnimation>,
        ),
        Without<RuntimeElement>,
    >,
) {
    for (interaction, children, mut image_node, hover_tex, mut hover_anim, base_anim) in &mut query {
        let hovering = matches!(interaction, Interaction::Hovered | Interaction::Pressed);

        // Handle HoverOverlay (child entity toggle)
        if let Some(children) = children {
            for child in children.iter() {
                if let Ok(mut vis) = hover_overlay_query.get_mut(child) {
                    *vis = if hovering {
                        Visibility::Inherited
                    } else {
                        Visibility::Hidden
                    };
                }
            }
        }

        // Handle HoverTexture swap (main texture replace)
        if let (Some(node), Some(ht)) = (image_node.as_mut(), hover_tex) {
            if hovering {
                if hover_anim.is_none() {
                    node.image = ht.hover.clone();
                }
            } else if *interaction == Interaction::None {
                if let Some(def) = &ht.default {
                    node.image = def.clone();
                }
            }
        }

        // Handle HoverAnimation reset
        if let Some(anim) = hover_anim.as_mut() {
            if hovering {
                if let Some(node) = image_node.as_mut() {
                    node.image = anim.handles[anim.current_frame].clone();
                }
            } else if *interaction == Interaction::None {
                if let Some(node) = image_node.as_mut() {
                    if let Some(fa) = base_anim {
                        node.image = fa.handles[fa.current_frame].clone();
                    } else if let Some(def) = &anim.default {
                        node.image = def.clone();
                    }
                    anim.elapsed = 0.0;
                    anim.current_frame = 0;
                }
            }
        } else if !hovering && *interaction == Interaction::None {
            // No hover animation, but maybe a hover texture needs clearing back to base animation.
            if let (Some(node), Some(fa)) = (image_node.as_mut(), base_anim) {
                node.image = fa.handles[fa.current_frame].clone();
            }
        }

        if let Some(children) = children {
            // Check children for native_size/cropped texture swaps
            for child in children.iter() {
                if let Ok((mut node, ht, ha, ba)) = child_image_query.get_mut(child) {
                    if hovering {
                        if let Some(ha) = ha {
                            node.image = ha.handles[ha.current_frame].clone();
                        } else if let Some(ht) = ht {
                            node.image = ht.hover.clone();
                        }
                    } else if *interaction == Interaction::None {
                        if let Some(mut ha) = ha {
                            if let Some(fa) = ba {
                                node.image = fa.handles[fa.current_frame].clone();
                            } else if let Some(def) = &ha.default {
                                node.image = def.clone();
                            }
                            ha.elapsed = 0.0;
                            ha.current_frame = 0;
                        } else if let Some(ht) = ht {
                            if let Some(fa) = ba {
                                node.image = fa.handles[fa.current_frame].clone();
                            } else if let Some(def) = &ht.default {
                                node.image = def.clone();
                            }
                        } else if let Some(fa) = ba {
                            node.image = fa.handles[fa.current_frame].clone();
                        }
                    }
                }
            }
        }
    }
}


pub(super) fn text_hover(
    mut query: Query<(&Interaction, &mut super::runtime::RuntimeText), Changed<Interaction>>,
) {
    for (interaction, mut rt) in &mut query {
        let hovering = matches!(interaction, Interaction::Hovered | Interaction::Pressed);
        if hovering {
            if let Some(hover_color) = rt.hover_color {
                rt.color = hover_color;
            }
        } else {
            rt.color = rt.base_color;
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

/// Animate hover animations for elements currently being hovered.
pub(super) fn hover_animate_tick(
    time: Res<Time>,
    // Animation is triggered by Interaction on elements with RuntimeElement
    mut query: Query<
        (Entity, &Interaction, Option<&Children>),
        (With<RuntimeElement>, Or<(With<HoverAnimation>, With<Children>)>),
    >,
    mut anim_query: Query<(&mut HoverAnimation, &mut ImageNode)>,
) {
    let delta = time.delta().as_secs_f32();
    for (entity, interaction, children) in &mut query {
        if !matches!(interaction, Interaction::Hovered | Interaction::Pressed) {
            continue;
        }

        // 1. Check main element
        if let Ok((mut anim, mut node)) = anim_query.get_mut(entity) {
            tick_anim(&mut anim, &mut node, delta);
        }

        // 2. Check children
        if let Some(children) = children {
            for child in children.iter() {
                if let Ok((mut anim, mut node)) = anim_query.get_mut(child) {
                    tick_anim(&mut anim, &mut node, delta);
                }
            }
        }
    }
}

fn tick_anim(anim: &mut HoverAnimation, node: &mut ImageNode, delta: f32) {
    anim.elapsed += delta;
    let n = anim.handles.len();
    if n == 0 {
        return;
    }

    let frame_idx_total = (anim.elapsed * anim.fps) as usize;

    let frame = if anim.ping_pong && n > 1 {
        let cycle_len = 2 * (n - 1);
        let idx_in_cycle = frame_idx_total % cycle_len;
        if idx_in_cycle < n {
            idx_in_cycle
        } else {
            cycle_len - idx_in_cycle
        }
    } else {
        frame_idx_total % n
    };

    if frame != anim.current_frame {
        anim.current_frame = frame;
        node.image = anim.handles[frame].clone();
    }
}

/// On click: swap to "clicked" texture (or hide if none), then fire actions after 200ms.
pub(super) fn screen_click(
    mut commands: Commands,
    mut query: Query<
        (
            Entity,
            &Interaction,
            &RuntimeElement,
            Option<&mut ImageNode>,
            Option<&ClickedTexture>,
            Option<&mut ClickedAnimation>,
        ),
        (Changed<Interaction>, With<Button>),
    >,
    layers: Res<ScreenLayers>,
    flash_query: Query<&ClickFlash>,
    mut ui_sound: Option<bevy::ecs::message::MessageWriter<crate::game::sound::effects::PlayUiSoundEvent>>,
) {
    for (entity, interaction, rt_elem, mut image_node, clicked_tex, clicked_anim) in &mut query {
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
        if elem.on_click().is_empty() && clicked_tex.is_none() {
            continue;
        }

        info!("screen click [{}/{}]", rt_elem.screen_id, elem.id());

        if let Some(img) = elem.as_image()
            && img.click_sound_id > 0
            && let Some(ref mut sound) = ui_sound
        {
            sound.write(crate::game::sound::effects::PlayUiSoundEvent {
                sound_id: img.click_sound_id,
            });
        }

        // Swap to "clicked" texture if available, otherwise hide briefly.
        if let Some(ref mut node) = image_node {
            if let Some(mut ca) = clicked_anim {
                ca.elapsed = 0.0;
                ca.current_frame = 0;
                node.image = ca.handles[0].clone();
            } else if let Some(ct) = clicked_tex {
                node.image = ct.clicked.clone();
            } else {
                commands.entity(entity).insert(Visibility::Hidden);
            }
        } else {
            commands.entity(entity).insert(Visibility::Hidden);
        }

        commands.entity(entity).insert(ClickFlash {
            timer: Timer::from_seconds(0.2, TimerMode::Once),
            pending_actions: elem.on_click().to_vec(),
        });
    }
}

/// Check keyboard shortcuts defined in all active screens.
pub(super) fn screen_keys(
    _commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    layers: Res<ScreenLayers>,
    mut actions: Option<MessageWriter<ScreenActions>>,
) {
    // Check keyboard shortcuts — Modal screens block lower-priority screens.
    use crate::screens::ScreenKind;
    let has_modal = layers.screens.values().any(|s| s.kind == ScreenKind::Modal);

    for screen in layers.screens.values() {
        // When a Modal screen is active, only Modal screens handle keys.
        if has_modal && screen.kind != ScreenKind::Modal {
            continue;
        }
        for (key_name, action_strings) in &screen.keys {
            if let Some(code) = crate::game::controls::parse_key_code(key_name)
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

/// Tick flash timers — restore default texture/visibility and fire pending actions.
pub(super) fn click_flash_tick(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(
        Entity,
        &mut ClickFlash,
        &mut Visibility,
        &Interaction,
        Option<&mut ImageNode>,
        Option<&ClickedTexture>,
        Option<&mut ClickedAnimation>,
        Option<&HoverTexture>,
        Option<&mut HoverAnimation>,
        Option<&super::runtime::FrameAnimation>,
        Option<&Children>,
    ), With<RuntimeElement>>,
    mut child_image_query: Query<
        (
            &mut ImageNode,
            Option<&ClickedTexture>,
            Option<&mut ClickedAnimation>,
            Option<&HoverTexture>,
            Option<&mut HoverAnimation>,
            Option<&super::runtime::FrameAnimation>,
        ),
        Without<RuntimeElement>,
    >,
    mut actions: Option<MessageWriter<ScreenActions>>,
) {
    for (entity, mut flash, mut vis, interaction, mut image_node, clicked_tex, mut clicked_anim, hover_tex, hover_anim, base_anim, children) in
        &mut query
    {
        if let Some(ref mut ca) = clicked_anim {
            tick_clicked_anim(ca, image_node.as_deref_mut(), time.delta_secs());
        }

        flash.timer.tick(time.delta());
        if !flash.timer.just_finished() {
            continue;
        }

        // Restore default texture or visibility.
        if let Some(node) = image_node.as_mut() {
            let default_tex = clicked_tex.and_then(|ct| ct.default.clone())
                .or_else(|| clicked_anim.as_ref().and_then(|ca| ca.default.clone()))
                .or_else(|| base_anim.as_ref().and_then(|ba| Some(ba.handles[ba.current_frame].clone())));

            if let Some(default) = default_tex {
                let hovering = matches!(interaction, Interaction::Hovered | Interaction::Pressed);
                if hovering {
                    if let Some(ht) = hover_tex {
                        node.image = ht.hover.clone();
                    } else if let Some(ha) = &hover_anim {
                        node.image = ha.handles[ha.current_frame].clone();
                    } else {
                        node.image = default;
                    }
                } else {
                    node.image = default;
                }
            } else if clicked_tex.is_some() || clicked_anim.is_some() {
                **node = ImageNode::default();
            }
        }
        if let Some(children) = children {
            let mut restored = false;
            for child in children.iter() {
                if let Ok((mut node, ct, mut ca, ht, ha, ba)) = child_image_query.get_mut(child) {
                    if let Some(ref mut ca) = ca {
                        tick_clicked_anim(ca, Some(&mut node), time.delta_secs());
                    }
                    if flash.timer.just_finished() {
                        // Restore default
                        let default_tex = ct.and_then(|ct| ct.default.clone())
                            .or_else(|| ca.as_ref().and_then(|ca| ca.default.clone()))
                            .or_else(|| ba.as_ref().and_then(|ba| Some(ba.handles[ba.current_frame].clone())));

                        if let Some(default) = default_tex {
                            let hovering = matches!(interaction, Interaction::Hovered | Interaction::Pressed);
                            if hovering {
                                if let Some(ht) = ht {
                                    node.image = ht.hover.clone();
                                } else if let Some(ha) = ha {
                                    node.image = ha.handles[ha.current_frame].clone();
                                } else {
                                    node.image = default;
                                }
                            } else {
                                node.image = default;
                            }
                            restored = true;
                        } else if ct.is_some() || ca.is_some() {
                            *node = ImageNode::default();
                            restored = true;
                        }
                    }
                }
            }
            if !restored && flash.timer.just_finished() {
                *vis = Visibility::Inherited;
            }
        } else if flash.timer.just_finished() {
            *vis = Visibility::Inherited;
        }

        if flash.timer.just_finished() {
            let p_actions: Vec<String> = flash.pending_actions.drain(..).collect();
            commands.entity(entity).remove::<ClickFlash>();

            if !p_actions.is_empty() {
                actions.try_write(ScreenActions { actions: p_actions });
            }
        }
    }
}

fn tick_clicked_anim(anim: &mut ClickedAnimation, node: Option<&mut ImageNode>, delta: f32) {
    anim.elapsed += delta;
    let frame = (anim.elapsed * anim.fps) as usize % anim.handles.len();
    if frame != anim.current_frame {
        anim.current_frame = frame;
        if let Some(node) = node {
            node.image = anim.handles[frame].clone();
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
    world_state: Option<Res<crate::game::state::WorldState>>,
    mut event_queue: Option<ResMut<crate::game::events::scripting::EventQueue>>,
    mut ui_state: Option<ResMut<crate::game::ui::UiState>>,
    mut cursor_query: Query<&mut CursorOptions, With<PrimaryWindow>>,
    sound_manager: Option<Res<crate::game::sound::SoundManager>>,
) {
    use super::scripting::Action;

    let actions_to_process: Vec<ScreenActions> = actions_set.p0().read().cloned().collect();
    let mut actions = actions_set.p1();

    for event in actions_to_process {
        let action_strings = event.actions.clone();

        // Build script context from available resources.
        let default_vars = crate::game::state::state::GameVariables::default();
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
                    hide_screen(&id, &mut commands, &mut layers, &layer_entities, &mut actions);
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
                Action::CloseWindow => {
                    info!("action: CloseWindow");
                    if let Some(ref mut ui) = ui_state {
                        crate::game::ui::set_ui_mode(ui, &mut cursor_query, crate::game::ui::UiMode::World);
                    }
                }
                Action::PlaySoundNamed(ref name) => {
                    push_sound_by_name(name, &sound_manager, &mut event_queue);
                }
                Action::EnterTurnBattle => {
                    info!("action: EnterTurnBattle");
                    if let Some(ref mut ui) = ui_state {
                        crate::game::ui::set_ui_mode(ui, &mut cursor_query, crate::game::ui::UiMode::TurnBattle);
                    }
                }
                Action::GreetingSound => {
                    const GREETING_SOUNDS: &[&str] = &["MaleA31a", "MaleA31b"];
                    let nanos = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .subsec_nanos() as usize;
                    push_sound_by_name(
                        GREETING_SOUNDS[nanos % GREETING_SOUNDS.len()],
                        &sound_manager,
                        &mut event_queue,
                    );
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
/// Resolve a dsounds name to a sound ID and push it to the event queue.
fn push_sound_by_name(
    name: &str,
    sound_manager: &Option<Res<crate::game::sound::SoundManager>>,
    event_queue: &mut Option<ResMut<crate::game::events::scripting::EventQueue>>,
) {
    if let Some(sm) = sound_manager
        && let Some(info) = sm.dsounds.get_by_name(name)
        && let Some(eq) = event_queue
    {
        eq.push_single(openmm_data::evt::GameEvent::PlaySound {
            sound_id: info.sound_id,
        });
    } else if sound_manager.is_some() {
        warn!("PlaySoundNamed: '{}' not found in dsounds", name);
    }
}

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
fn proxy_evt_action(evt_str: &str, event_queue: &mut crate::game::events::scripting::EventQueue) {
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
