use std::collections::VecDeque;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::{CursorOptions, PrimaryWindow};

use openmm_data::enums::{ActorAttributes, EvtVariable};
use openmm_data::evt::{EvtFile, EvtStep, GameEvent};

use crate::game::coords::{mm6_binary_angle_to_radians, mm6_position_to_bevy};

use crate::GameState;
use crate::assets::GameAssets;
use crate::game::party::Party;
use crate::game::sprites::material::SpriteMaterial;

/// Bundles save + state transition to stay within Bevy's 16-param system limit.
#[derive(SystemParam)]
struct TransitionParams<'w> {
    save_data: ResMut<'w, crate::save::GameSave>,
    game_state: ResMut<'w, NextState<GameState>>,
}
use super::events::MapEvents;
use super::state::GameVariables;
use crate::game::actors::Actor;
use crate::game::hud::{FooterText, HudView, OverlayImage};
use crate::game::interaction::DecorationInfo;
use crate::game::optional::OptionalWrite;
use crate::game::outdoor::ApplyTextureOutdoors;
use crate::game::sound::SoundManager;
use crate::game::sound::effects::PlayUiSoundEvent;
use crate::states::loading::LoadRequest;
use openmm_data::utils::MapName;

/// Bundles map entity queries to stay within Bevy's 16-param system limit.
/// Wraps the decoration sprite-swap query and actor visibility/flag query.
#[derive(SystemParam)]
struct MapEntityParams<'w, 's> {
    decorations: Query<
        'w,
        's,
        (
            &'static DecorationInfo,
            &'static mut MeshMaterial3d<SpriteMaterial>,
            &'static mut Mesh3d,
            &'static mut Transform,
        ),
        Without<crate::game::player::Player>,
    >,
    actors: Query<'w, 's, (&'static mut Actor, &'static mut Visibility)>,
    player: Query<'w, 's, &'static mut Transform, With<crate::game::player::Player>>,
    player_settings: Res<'w, crate::game::player::PlayerSettings>,
}

/// Bundles audio + mesh assets + game_time to stay within Bevy's 16-param limit.
#[derive(SystemParam)]
struct AudioParams<'w> {
    ui_sound: Option<bevy::ecs::message::MessageWriter<'w, PlayUiSoundEvent>>,
    texture_outdoors: bevy::ecs::message::MessageWriter<'w, ApplyTextureOutdoors>,
    sound_manager: Option<Res<'w, SoundManager>>,
    game_time: Option<Res<'w, super::time::GameTime>>,
    meshes: ResMut<'w, Assets<Mesh>>,
}

/// An event sequence — a list of steps from one event_id, executed as a script.
#[derive(Clone)]
struct EventSequence {
    event_id: Option<u16>,
    steps: Vec<EvtStep>,
}

/// Queue of event sequences waiting to be processed.
/// Each sequence is executed in full (with control flow) in one frame.
#[derive(Resource, Default)]
pub struct EventQueue {
    sequences: VecDeque<EventSequence>,
}

impl EventQueue {
    /// Enqueue all steps for a given event_id from the EvtFile as a single sequence.
    pub fn push_all(&mut self, event_id: u16, evt: &EvtFile) {
        if let Some(steps) = evt.events.get(&event_id)
            && !steps.is_empty()
        {
            self.sequences.push_back(EventSequence {
                event_id: Some(event_id),
                steps: steps.clone(),
            });
        }
    }

    /// Pop the next sequence from the front.
    fn pop(&mut self) -> Option<EventSequence> {
        self.sequences.pop_front()
    }

    /// Enqueue a single synthesized event (not from an EvtFile).
    pub fn push_single(&mut self, event: openmm_data::evt::GameEvent) {
        self.sequences.push_back(EventSequence {
            event_id: None,
            steps: vec![openmm_data::evt::EvtStep { step: 0, event }],
        });
    }

    /// Enqueue steps from index `start` onward (used to skip lifecycle marker steps).
    pub fn push_from(&mut self, event_id: u16, evt: &EvtFile, start: usize) {
        if let Some(steps) = evt.events.get(&event_id) {
            let tail: Vec<_> = steps[start.min(steps.len())..].to_vec();
            if !tail.is_empty() {
                self.sequences.push_back(EventSequence {
                    event_id: Some(event_id),
                    steps: tail,
                });
            }
        }
    }

    /// Clear all pending sequences.
    pub fn clear(&mut self) {
        self.sequences.clear();
    }
}

pub struct EventDispatchPlugin;

impl Plugin for EventDispatchPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EventQueue>()
            .add_systems(OnEnter(GameState::Game), dispatch_on_map_reload)
            .add_systems(Update, process_events.run_if(in_state(GameState::Game)));
    }
}

/// On every map entry, dispatch all events that contain an `OnMapReload` step.
/// Executes from the step immediately after `OnMapReload` — the marker can appear anywhere
/// in the script (not just as the first step), e.g. the apple tree event embeds its reload
/// handler mid-script after the interactive click path.
fn dispatch_on_map_reload(map_events: Option<Res<MapEvents>>, mut event_queue: ResMut<EventQueue>) {
    let Some(me) = map_events else { return };
    let Some(evt) = me.evt.as_ref() else { return };

    let mut ids: Vec<u16> = evt
        .events
        .iter()
        .filter(|(_, steps)| steps.iter().any(|s| matches!(s.event, GameEvent::OnMapReload)))
        .map(|(id, _)| *id)
        .collect();
    ids.sort();

    for id in ids {
        if let Some(steps) = evt.events.get(&id) {
            let Some(reload_idx) = steps.iter().position(|s| matches!(s.event, GameEvent::OnMapReload)) else {
                continue;
            };
            let remaining = steps.len() - reload_idx - 1;
            info!("OnMapReload: event {} ({} steps after marker)", id, remaining);
            // Skip the OnMapReload marker itself, run everything after it.
            event_queue.push_from(id, evt, reload_idx + 1);
        }
    }
}

// ── Variable read/write helpers ────────────────────────────────────────

/// Read a game variable's current value.
fn get_variable(
    vars: &GameVariables,
    party: &Party,
    game_time: Option<&super::time::GameTime>,
    var: EvtVariable,
) -> i32 {
    if var.is_map_var() {
        return vars.map_vars[var.map_var_index().unwrap() as usize];
    }
    match var {
        EvtVariable::GOLD => vars.gold,
        EvtVariable::FOOD => vars.food,
        EvtVariable::REPUTATION_IS => vars.reputation,
        EvtVariable::QBITS => 0,          // compare uses contains check, handled separately
        EvtVariable::AUTONOTES_BITS => 0, // compare uses contains check
        EvtVariable::FLYING => vars.flying as i32,
        EvtVariable::NPCS => vars.npcs_in_party,
        EvtVariable::TOTAL_CIRCUS_PRIZE => vars.total_circus_prize,
        EvtVariable::SKILL_POINTS => party.get_member_var(party.active_target, var),
        EvtVariable::DAYS_COUNTER1 => vars.days_counters[0],
        EvtVariable::DAYS_COUNTER2 => vars.days_counters[1],
        EvtVariable::DAYS_COUNTER3 => vars.days_counters[2],
        EvtVariable::DAYS_COUNTER4 => vars.days_counters[3],
        EvtVariable::DAYS_COUNTER5 => vars.days_counters[4],
        EvtVariable::DAYS_COUNTER6 => vars.days_counters[5],
        EvtVariable::MONTH_IS => game_time.map(|t| t.calendar_date().1 as i32).unwrap_or(1),
        EvtVariable::HOUR_IS => game_time.map(|t| t.hour() as i32).unwrap_or(9),
        EvtVariable::DAY_OF_WEEK_IS => game_time.map(|t| t.day_of_week() as i32).unwrap_or(0),
        EvtVariable::DAY_OF_YEAR_IS => {
            game_time
                .map(|t| {
                    let (_, m, d) = t.calendar_date();
                    ((m - 1) * 28 + d) as i32 // MM6 uses 28-day months
                })
                .unwrap_or(1)
        }
        _ => {
            // Per-character variables (attrs, skills, conditions, etc.)
            let pv = party.get_member_var(party.active_target, var);
            if pv != 0 || is_character_var(var) {
                return pv;
            }
            debug!(
                "get_variable: unhandled variable {} (0x{:02x}), returning 0",
                var, var.0
            );
            0
        }
    }
}

/// Log steps skipped by a forward jump. `from_pc`..`to_pc` are step *indices* (not step numbers).
fn log_skipped(steps: &[EvtStep], from_pc: usize, to_pc: usize, reason: &str) {
    match to_pc.cmp(&from_pc) {
        std::cmp::Ordering::Equal => {}
        std::cmp::Ordering::Less => {
            debug!("  ↺ backward jump ({})", reason);
        }
        std::cmp::Ordering::Greater => {
            for s in &steps[from_pc..to_pc.min(steps.len())] {
                info!("  ↷ [step {}] skip({}): {}", s.step, reason, s.event);
            }
        }
    }
}

/// Log all remaining steps in the sequence as unreachable (sequence ended early).
fn log_tail_unreachable(steps: &[EvtStep], from_pc: usize) {
    for s in steps.get(from_pc..).unwrap_or(&[]) {
        info!("  ⊘ [step {}] unreachable: {}", s.step, s.event);
    }
}

/// Show the autonote text in the footer when a note is newly acquired.
fn show_autonote_text(id: i32, assets: &GameAssets, footer: &mut FooterText, time_secs: f64) {
    if let Some(note) = assets.autonotes().and_then(|t| t.get(id as u16))
        && !note.text.is_empty()
    {
        footer.set_status(&note.text, 4.0, time_secs);
    }
}

/// Returns true if this variable is per-character (not global).
fn is_character_var(var: EvtVariable) -> bool {
    matches!(var.0, 0x01..=0x68)
}

/// Write a value to a game variable.
fn set_variable(vars: &mut GameVariables, party: &mut Party, var: EvtVariable, value: i32) {
    if var.is_map_var() {
        let idx = var.map_var_index().unwrap() as usize;
        info!("  {} = {} (was {})", var, value, vars.map_vars[idx]);
        vars.map_vars[idx] = value;
        return;
    }
    match var {
        EvtVariable::GOLD => {
            info!("  Gold = {} (was {})", value, vars.gold);
            vars.gold = value;
        }
        EvtVariable::FOOD => {
            info!("  Food = {} (was {})", value, vars.food);
            vars.food = value;
        }
        EvtVariable::REPUTATION_IS => {
            info!("  Reputation = {} (was {})", value, vars.reputation);
            vars.reputation = value;
        }
        EvtVariable::QBITS => {
            if value != 0 {
                vars.set_qbit(value);
            }
        }
        EvtVariable::AUTONOTES_BITS => {
            if value != 0 {
                vars.add_autonote(value);
            }
        }
        EvtVariable::FLYING => vars.flying = value != 0,
        EvtVariable::NPCS => vars.npcs_in_party = value,
        EvtVariable::TOTAL_CIRCUS_PRIZE => vars.total_circus_prize = value,
        EvtVariable::DAYS_COUNTER1 => vars.days_counters[0] = value,
        EvtVariable::DAYS_COUNTER2 => vars.days_counters[1] = value,
        EvtVariable::DAYS_COUNTER3 => vars.days_counters[2] = value,
        EvtVariable::DAYS_COUNTER4 => vars.days_counters[3] = value,
        EvtVariable::DAYS_COUNTER5 => vars.days_counters[4] = value,
        EvtVariable::DAYS_COUNTER6 => vars.days_counters[5] = value,
        _ => {
            if is_character_var(var) {
                let target = party.active_target;
                party.set_member_var(target, var, value);
            } else {
                warn!(
                    "  set_variable: unhandled variable {} (0x{:02x}) = {}",
                    var, var.0, value
                );
            }
        }
    }
}

/// Queue a non-positional UI sound by its `dsounds.bin` name (e.g. `"Quest"`,
/// `"EventSFX01"`). Silent no-op if the sound manager isn't available
/// (headless build) or the name isn't in the table.
///
/// Kept generic so any event handler that needs a named jingle — pickups,
/// quest completions, UI feedback — can reach for one call instead of
/// reimplementing the `dsounds → sound_id → PlayUiSoundEvent` chain.
fn play_ui_sound_named(
    name: &str,
    sound_manager: Option<&SoundManager>,
    ui_sound: &mut Option<bevy::ecs::message::MessageWriter<PlayUiSoundEvent>>,
) {
    let Some(sm) = sound_manager else {
        return;
    };
    let Some(sound_id) = sm.dsounds.get_by_name(name).map(|s| s.sound_id) else {
        warn!("ui sound '{}' not found in dsounds", name);
        return;
    };
    ui_sound.try_write(PlayUiSoundEvent { sound_id });
}

/// Add to a game variable.
fn add_variable(vars: &mut GameVariables, party: &mut Party, var: EvtVariable, value: i32) {
    if var.is_map_var() {
        let idx = var.map_var_index().unwrap() as usize;
        let old = vars.map_vars[idx];
        vars.map_vars[idx] = old.wrapping_add(value);
        info!("  {} += {} ({} -> {})", var, value, old, vars.map_vars[idx]);
        return;
    }
    match var {
        EvtVariable::GOLD => {
            let old = vars.gold;
            vars.gold += value;
            info!("  Gold += {} ({} -> {})", value, old, vars.gold);
        }
        EvtVariable::FOOD => {
            let old = vars.food;
            vars.food += value;
            info!("  Food += {} ({} -> {})", value, old, vars.food);
        }
        EvtVariable::QBITS => {
            vars.set_qbit(value);
        }
        EvtVariable::AUTONOTES_BITS => {
            vars.add_autonote(value);
        }
        EvtVariable::DAYS_COUNTER1 => vars.days_counters[0] += value,
        EvtVariable::DAYS_COUNTER2 => vars.days_counters[1] += value,
        EvtVariable::DAYS_COUNTER3 => vars.days_counters[2] += value,
        EvtVariable::DAYS_COUNTER4 => vars.days_counters[3] += value,
        EvtVariable::DAYS_COUNTER5 => vars.days_counters[4] += value,
        EvtVariable::DAYS_COUNTER6 => vars.days_counters[5] += value,
        EvtVariable::TOTAL_CIRCUS_PRIZE => vars.total_circus_prize += value,
        _ => {
            if is_character_var(var) {
                let target = party.active_target;
                party.add_member_var(target, var, value);
            } else {
                warn!(
                    "  add_variable: unhandled variable {} (0x{:02x}) += {}",
                    var, var.0, value
                );
            }
        }
    }
}

/// Subtract from a game variable.
fn subtract_variable(vars: &mut GameVariables, party: &mut Party, var: EvtVariable, value: i32) {
    if var.is_map_var() {
        let idx = var.map_var_index().unwrap() as usize;
        let old = vars.map_vars[idx];
        vars.map_vars[idx] = old.wrapping_sub(value);
        info!("  {} -= {} ({} -> {})", var, value, old, vars.map_vars[idx]);
        return;
    }
    match var {
        EvtVariable::GOLD => {
            let old = vars.gold;
            vars.gold -= value;
            info!("  Gold -= {} ({} -> {})", value, old, vars.gold);
        }
        EvtVariable::FOOD => {
            let old = vars.food;
            vars.food -= value;
            info!("  Food -= {} ({} -> {})", value, old, vars.food);
        }
        EvtVariable::QBITS => {
            vars.clear_qbit(value);
        }
        EvtVariable::AUTONOTES_BITS => {
            vars.remove_autonote(value);
        }
        EvtVariable::TOTAL_CIRCUS_PRIZE => vars.total_circus_prize -= value,
        _ => {
            if is_character_var(var) {
                let target = party.active_target;
                party.add_member_var(target, var, -value);
            } else {
                warn!(
                    "  subtract_variable: unhandled variable {} (0x{:02x}) -= {}",
                    var, var.0, value
                );
            }
        }
    }
}

/// Apply a single ActorAttributes flag change to a live actor entity.
/// Handles VISIBLE → Bevy Visibility and HOSTILE → actor.hostile. Other bits
/// in the MM6 `ActorAttributes` bitflag are not yet modelled in the runtime.
fn apply_actor_flags(actor: &mut Actor, vis: &mut Visibility, flag: u32, on: bool) {
    let flags = ActorAttributes::from_bits_truncate(flag);
    if flags.contains(ActorAttributes::VISIBLE) {
        *vis = if on { Visibility::Visible } else { Visibility::Hidden };
    }
    if flags.contains(ActorAttributes::HOSTILE) {
        actor.hostile = on;
    }
}

/// Evaluate a Compare condition. Returns true if condition is met (JUMP to jump_step).
/// MM6 Compare semantics: jump when condition is TRUE (e.g. "already done" → skip).
fn evaluate_compare(
    vars: &GameVariables,
    party: &Party,
    game_time: Option<&super::time::GameTime>,
    var: EvtVariable,
    value: i32,
) -> bool {
    // Special cases: QBits and Autonotes check set membership
    if var == EvtVariable::QBITS {
        let result = vars.has_qbit(value);
        debug!("  Compare: QBit {} present? -> {}", value, result);
        return result;
    }
    if var == EvtVariable::AUTONOTES_BITS {
        let result = vars.has_autonote(value);
        debug!("  Compare: Autonote {} present? -> {}", value, result);
        return result;
    }
    if var == EvtVariable::INVENTORY {
        let result = vars.item_count(value) >= 1;
        debug!("  Compare: HasItem({}) -> {}", value, result);
        return result;
    }

    // MM6 Compare semantics: numeric variables use >= (not ==)
    let current = get_variable(vars, party, game_time, var);
    let result = current >= value;
    debug!("  Compare: {} = {} >= {}? -> {}", var, current, value, result);
    result
}

/// Process one event sequence per frame from the EventQueue.
/// Each sequence is executed as a script with control flow (Compare/Jmp/RandomGoTo).
fn process_events(
    mut event_queue: ResMut<EventQueue>,
    map_events: Option<Res<MapEvents>>,
    game_assets: Res<GameAssets>,
    mut images: ResMut<Assets<Image>>,
    mut sprite_materials: Option<ResMut<Assets<SpriteMaterial>>>,
    mut commands: Commands,
    mut hud_view: ResMut<HudView>,
    mut footer: ResMut<FooterText>,
    mut cursor_query: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mut transition: TransitionParams,
    mut blv_doors: Option<ResMut<crate::game::indoor::BlvDoors>>,
    mut audio: AudioParams,
    mut world_state: ResMut<super::state::WorldState>,
    mut party: ResMut<Party>,
    time: Res<Time>,
    mut entities: MapEntityParams,
) {
    // Tick footer timer every frame
    footer.tick(time.elapsed_secs_f64());
    // Don't process events while a UI overlay is blocking
    if !matches!(*hud_view, HudView::World) {
        return;
    }

    let Some(sequence) = event_queue.pop() else {
        return;
    };

    let steps = &sequence.steps;
    let id_str = sequence
        .event_id
        .map(|id| format!(" event_id={}", id))
        .unwrap_or_default();
    info!("── Event{} ({} steps) ──", id_str, steps.len());
    let qb = game_assets.quests();
    let mut pc = 0usize; // program counter (index into steps vec)
    let mut iterations = 0u32;
    const MAX_ITERATIONS: u32 = 500;

    while pc < steps.len() {
        iterations += 1;
        if iterations > MAX_ITERATIONS {
            warn!(
                "Event script exceeded {} iterations, aborting (infinite loop?)",
                MAX_ITERATIONS
            );
            break;
        }

        let EvtStep { step, ref event } = steps[pc];
        pc += 1; // advance past current instruction

        info!("  ▶ [step {}] {}", step, qb.annotate(&event.to_string()));

        match event {
            // ── Already implemented (side-effects) ───────────────────
            GameEvent::Hint { str_id, text } => {
                debug!("Hint(id={}): {}", str_id, text);
                footer.set(text);
            }
            GameEvent::SpeakInHouse { house_id } => {
                // Show transition/location description if one exists for this house_id.
                if let Some(desc) = game_assets
                    .trans()
                    .and_then(|t| t.get(*house_id as u16))
                    .map(|e| e.description.clone())
                    .filter(|s| !s.is_empty())
                {
                    footer.set_status(&desc, 4.0, time.elapsed_secs_f64());
                }
                let image = map_events
                    .as_ref()
                    .and_then(|me| super::events::resolve_building_image(*house_id, me, &game_assets, &mut images))
                    .or_else(|| game_assets.load_icon("evt02", &mut images));
                if let Some(image) = image {
                    commands.insert_resource(OverlayImage { image });
                    *hud_view = HudView::Building;
                    crate::game::hud::grab_cursor(&mut cursor_query, false);
                }
            }
            GameEvent::OpenChest { id } => {
                debug!("OpenChest(id={})", id);
                let icon_name = format!("chest{:02}", id);
                if let Some(image) = game_assets.load_icon(&icon_name, &mut images) {
                    // Play chest-open sound if available
                    if let Some(ref sm) = audio.sound_manager
                        && let Some(s) = sm.dsounds.get_by_name("openchest0101")
                    {
                        audio.ui_sound.try_write(PlayUiSoundEvent { sound_id: s.sound_id });
                    }
                    commands.insert_resource(OverlayImage { image });
                    *hud_view = HudView::Chest;
                    crate::game::hud::grab_cursor(&mut cursor_query, false);
                }
            }
            GameEvent::MoveToMap {
                x,
                y,
                z,
                direction,
                map_name,
            } => {
                // A name with no letters (e.g. "0") means same-map teleport — just
                // reposition the player without reloading the map.
                // The original MM6 engine hardcodes playing the teleport sound here
                // (there is no PlaySound step in the EVT data for MoveToMap events).
                if !map_name.chars().any(|c| c.is_ascii_alphabetic()) {
                    if let Some(ref sm) = audio.sound_manager
                        && let Some(s) = sm.dsounds.get_by_name("teleport")
                    {
                        audio.ui_sound.try_write(PlayUiSoundEvent { sound_id: s.sound_id });
                    }
                    let base = Vec3::from(mm6_position_to_bevy(*x, *y, *z));
                    // Player Transform.y is at eye level (feet + eye_height), same as spawn.
                    let pos = Vec3::new(base.x, base.y + entities.player_settings.eye_height, base.z);
                    let yaw = mm6_binary_angle_to_radians(*direction);
                    if let Ok(mut tf) = entities.player.single_mut() {
                        tf.translation = pos;
                        tf.rotation = Quat::from_rotation_y(yaw);
                        info!(
                            "MoveToMap same-map teleport: pos={:?} yaw={:.1}deg",
                            pos,
                            yaw.to_degrees()
                        );
                    }
                    event_queue.clear();
                    return;
                }
                let Ok(target) = MapName::try_from(map_name.as_str()) else {
                    warn!("MoveToMap: invalid map name '{}'", map_name);
                    return;
                };

                let pos = mm6_position_to_bevy(*x, *y, *z);
                let yaw = mm6_binary_angle_to_radians(*direction);

                debug!(
                    "MoveToMap: '{}' mm6=({},{},{}) dir={} -> bevy={:?} yaw={:.1}deg",
                    map_name,
                    x,
                    y,
                    z,
                    direction,
                    pos,
                    yaw.to_degrees()
                );

                if let MapName::Outdoor(ref odm) = target {
                    transition.save_data.map.map_x = odm.x;
                    transition.save_data.map.map_y = odm.y;
                    world_state.map.map_x = odm.x;
                    world_state.map.map_y = odm.y;
                }
                world_state.map.name = target.clone();

                transition.save_data.player.position = pos;
                transition.save_data.player.yaw = yaw;

                commands.insert_resource(LoadRequest {
                    map_name: target,
                    spawn_position: Some(pos),
                    spawn_yaw: Some(yaw),
                });
                transition.game_state.set(GameState::Loading);

                // Reset map vars on map transition
                world_state.game_vars.map_vars = [0; 100];
                event_queue.clear();
                return; // Stop executing this sequence
            }
            GameEvent::ChangeDoorState { door_id, action } => {
                debug!("ChangeDoorState door_id={} action={}", door_id, action);
                if let Some(ref mut doors) = blv_doors {
                    crate::game::indoor::trigger_door(doors, *door_id as u32, action.as_u8());
                }
            }
            GameEvent::PlaySound { sound_id } => {
                audio.ui_sound.try_write(PlayUiSoundEvent { sound_id: *sound_id });
            }
            GameEvent::StatusText { str_id, text } => {
                debug!("StatusText(id={}): {}", str_id, text);
                footer.set_status(text, 2.0, time.elapsed_secs_f64());
            }
            GameEvent::LocationName { str_id, text } => {
                debug!("LocationName(id={}): {}", str_id, text);
                footer.set_status(text, 2.0, time.elapsed_secs_f64());
            }
            GameEvent::ShowMessage { str_id, text } => {
                debug!("ShowMessage(id={}): {}", str_id, text);
                footer.set_status(text, 4.0, time.elapsed_secs_f64());
            }
            GameEvent::PlayVideo { name, skippable } => {
                commands.insert_resource(crate::states::video::VideoRequest {
                    name: name.clone(),
                    skippable: *skippable,
                    next: GameState::Game,
                });
                transition.game_state.set(GameState::Video);
                event_queue.clear();
                return;
            }
            GameEvent::Exit => {
                log_tail_unreachable(steps, pc);
                event_queue.clear();
                return;
            }

            // ── Control flow (NOW WORKING) ───────────────────────────
            GameEvent::Compare { var, value, jump_step } => {
                if evaluate_compare(&world_state.game_vars, &party, audio.game_time.as_deref(), *var, *value) {
                    if let Some(target_idx) = steps.iter().position(|s| s.step >= *jump_step) {
                        log_skipped(steps, pc, target_idx, "Compare true");
                        pc = target_idx;
                    } else {
                        log_tail_unreachable(steps, pc);
                        return;
                    }
                }
            }
            GameEvent::Jmp { target_step } => {
                if let Some(target_idx) = steps.iter().position(|s| s.step >= *target_step) {
                    log_skipped(steps, pc, target_idx, "Jmp");
                    pc = target_idx;
                } else {
                    log_tail_unreachable(steps, pc);
                    return;
                }
            }
            GameEvent::RandomGoTo { steps: goto_steps } => {
                if !goto_steps.is_empty() {
                    let idx = (step as usize) % goto_steps.len();
                    let target_step = goto_steps[idx];
                    debug!("  RandomGoTo -> picked step {} from {:?}", target_step, goto_steps);
                    if let Some(target_idx) = sequence.steps.iter().position(|s| s.step >= target_step) {
                        log_skipped(steps, pc, target_idx, "RandomGoTo");
                        pc = target_idx;
                    } else {
                        log_tail_unreachable(steps, pc);
                        return;
                    }
                }
            }
            GameEvent::ForPartyMember { player } => {
                if let Some(target) = openmm_data::enums::EvtTargetCharacter::from_u8(*player) {
                    info!("  ForPartyMember: target = {:?}", target);
                    party.active_target = target;
                } else {
                    warn!("  ForPartyMember: unknown player byte {}", player);
                }
            }

            // ── Variable operations (NOW WORKING) ────────────────────
            GameEvent::Add { var, value } => {
                let show_note =
                    *var == EvtVariable::AUTONOTES_BITS && *value != 0 && !world_state.game_vars.has_autonote(*value);
                // Positive gold/food deltas are pickups (apple tree, gold
                // pile, etc.) — play the stock MM6 quest/pickup jingle so
                // acquiring objects feels the same as it did in the original.
                let is_pickup = *value > 0 && matches!(*var, EvtVariable::GOLD | EvtVariable::FOOD);
                add_variable(&mut world_state.game_vars, &mut party, *var, *value);
                if is_pickup {
                    play_ui_sound_named("Quest", audio.sound_manager.as_deref(), &mut audio.ui_sound);
                }
                if show_note {
                    show_autonote_text(*value, &game_assets, &mut footer, time.elapsed_secs_f64());
                }
            }
            GameEvent::Subtract { var, value } => {
                subtract_variable(&mut world_state.game_vars, &mut party, *var, *value);
            }
            GameEvent::Set { var, value } => {
                let show_note =
                    *var == EvtVariable::AUTONOTES_BITS && *value != 0 && !world_state.game_vars.has_autonote(*value);
                set_variable(&mut world_state.game_vars, &mut party, *var, *value);
                if show_note {
                    show_autonote_text(*value, &game_assets, &mut footer, time.elapsed_secs_f64());
                }
            }

            // ── World operations (stubs with warns) ──────────────────
            GameEvent::SetSnow { on } => {
                info!("SetSnow: on={} (no weather system)", on);
            }
            GameEvent::SetFacesBit { face_id, bit, on } => {
                warn!("STUB SetFacesBit: face={} bit=0x{:x} on={}", face_id, bit, on);
            }
            GameEvent::ToggleActorFlag { actor_id, flag, on } => {
                let flag = *flag as u32;
                let on = *on != 0;
                // Store in persistent flags map.
                let entry = world_state.game_vars.actor_flags.entry(*actor_id).or_insert(0);
                if on {
                    *entry |= flag;
                } else {
                    *entry &= !flag;
                }
                // Apply VISIBLE (0x8) and HOSTILE (0x01000000) immediately to live entities.
                for (mut actor, mut vis) in entities.actors.iter_mut() {
                    if actor.ddm_id != *actor_id {
                        continue;
                    }
                    apply_actor_flags(&mut actor, &mut vis, flag, on);
                    break;
                }
                info!("ToggleActorFlag: actor={} flag=0x{:x} on={}", actor_id, flag, on);
            }
            GameEvent::SetTexture { face_id, texture_name } => {
                warn!("STUB SetTexture: face={} tex='{}'", face_id, texture_name);
            }
            GameEvent::SetSprite {
                decoration_id,
                sprite_name,
            } => {
                info!("SetSprite: deco={} sprite='{}'", decoration_id, sprite_name);
                let target_idx = *decoration_id as usize;
                // Find the target entity first to get declist_id and ground_y.
                let target = entities
                    .decorations
                    .iter()
                    .find(|(d, ..)| d.billboard_index == target_idx)
                    .map(|(d, ..)| (d.declist_id, d.ground_y));
                let Some((declist_id, ground_y)) = target else {
                    debug!("SetSprite: decoration {} not found", target_idx);
                    continue;
                };
                let _ = declist_id; // stored for future use (e.g. directional swap)
                let Some(sprite_materials) = sprite_materials.as_deref_mut() else {
                    continue;
                };
                // New materials reference the shared tint storage buffer, so
                // they pick up the current day/night tint automatically without
                // any per-material write here. Default to regular; selflit sprite
                // swaps aren't handled by the current SetSprite opcode.
                let Some((new_mat, new_mesh, _new_w, new_h)) =
                    crate::game::sprites::loading::load_static_decoration_sprite(
                        sprite_name,
                        game_assets.assets(),
                        &mut images,
                        sprite_materials,
                        &mut audio.meshes,
                        false,
                    )
                else {
                    warn!("SetSprite: sprite '{}' not found in LOD", sprite_name);
                    continue;
                };
                for (deco_info, mut mat_handle, mut mesh_handle, mut transform) in entities.decorations.iter_mut() {
                    if deco_info.billboard_index == target_idx {
                        transform.translation.y = ground_y + new_h / 2.0;
                        mesh_handle.0 = new_mesh.clone();
                        mat_handle.0 = new_mat.clone();
                        break;
                    }
                }
            }
            GameEvent::ToggleIndoorLight { light_id, on } => {
                warn!("STUB ToggleIndoorLight: light={} on={}", light_id, on);
            }

            // ── Combat / items ───────────────────────────────────────
            GameEvent::SummonMonsters {
                monster_id,
                count,
                x,
                y,
                z,
            } => {
                warn!(
                    "STUB SummonMonsters: id={} count={} at ({},{},{})",
                    monster_id, count, x, y, z
                );
            }
            GameEvent::CastSpell {
                spell_id,
                skill_level,
                skill_mastery,
                from_x,
                from_y,
                from_z,
                to_x,
                to_y,
                to_z,
            } => {
                warn!(
                    "STUB CastSpell: spell={} level={} mastery={} from=({},{},{}) to=({},{},{})",
                    spell_id, skill_level, skill_mastery, from_x, from_y, from_z, to_x, to_y, to_z
                );
            }
            GameEvent::ReceiveDamage { damage_type, amount } => {
                warn!("STUB ReceiveDamage: type={} amount={}", damage_type, amount);
            }
            GameEvent::ShowFace { player, expression } => {
                debug!("ShowFace: player={} expr={} (no portrait UI)", player, expression);
            }
            GameEvent::GiveItem {
                strength,
                item_type,
                item_id,
            } => {
                // strength and item_type control enchantment/quality — not tracked yet.
                world_state.game_vars.give_item(*item_id as i32, 1);
                play_ui_sound_named("Quest", audio.sound_manager.as_deref(), &mut audio.ui_sound);
                info!("GiveItem: id={} str={} type={}", item_id, strength, item_type);
            }
            GameEvent::SetNPCTopic {
                npc_id,
                topic_index,
                event_id,
            } => {
                warn!(
                    "STUB SetNPCTopic: npc={} topic={} event={}",
                    npc_id, topic_index, event_id
                );
            }
            GameEvent::MoveNPC { npc_id, map_id } => {
                warn!("STUB MoveNPC: npc={} map_id={}", npc_id, map_id);
            }
            GameEvent::SpeakNPC { npc_id } => {
                // day_of_week: GameTime uses 0=Monday epoch; proftext uses 0=Sunday.
                // Shift by 6 to convert: Monday(0)→1, …, Sunday(6)→0.
                let dow = audio
                    .game_time
                    .as_ref()
                    .map(|gt| (gt.day_of_week() + 6) % 7)
                    .unwrap_or(0);
                if let Some((portrait, profile)) = crate::game::hud::overlay::prepare_npc_dialogue(
                    *npc_id,
                    &map_events,
                    &game_assets,
                    &mut images,
                    dow,
                    &world_state.game_vars.npc_greetings,
                ) {
                    commands.insert_resource(portrait);
                    commands.insert_resource(profile);
                    *hud_view = HudView::NpcDialogue;
                    crate::game::hud::grab_cursor(&mut cursor_query, false);
                }
            }
            GameEvent::ChangeEvent { target, new_event_id } => {
                warn!("STUB ChangeEvent: target={} event={}", target, new_event_id);
            }
            GameEvent::SetNPCGreeting { npc_id, greeting_id } => {
                info!("SetNPCGreeting: npc={} greeting={}", npc_id, greeting_id);
                world_state.game_vars.npc_greetings.insert(*npc_id, *greeting_id);
            }
            GameEvent::OnMapReload => {
                debug!("Marker: OnMapReload");
                // Marker step — handled by dispatch_on_map_reload. no-op here.
            }
            GameEvent::OnMapLeave => {
                debug!("Marker: OnMapLeave");
                // Marker step.
            }
            GameEvent::OnTimer {
                year,
                month,
                week,
                day,
                hour,
                minute,
            } => {
                warn!(
                    "STUB OnTimer trigger: {:04}-{:02}-{:02} (week {}) {:02}:{:02}",
                    year, month, day, week, hour, minute
                );
            }
            GameEvent::OnLongTimer { timer_data } => {
                warn!("STUB OnLongTimer trigger: data={:02x?}", timer_data);
            }
            GameEvent::OnCanShowDialogItemCmp { var, value } => {
                debug!("Marker: OnCanShowDialogItemCmp({:?} == {})", var, value);
                // Logic marker for dialogue system.
            }
            GameEvent::EndCanShowDialogItem => {
                debug!("Marker: EndCanShowDialogItem");
                // Logic marker.
            }
            GameEvent::SetCanShowDialogItem { on } => {
                debug!("Marker: SetCanShowDialogItem({})", on);
                // State marker.
            }
            GameEvent::IsActorKilled {
                actor_group,
                count,
                jump_step,
            } => {
                // Check if >= count actors in group are dead (killed_actors map).
                let killed = world_state
                    .game_vars
                    .killed_groups
                    .get(actor_group)
                    .copied()
                    .unwrap_or(0);
                if killed >= *count as u32 {
                    if let Some(target_idx) = steps.iter().position(|s| s.step >= *jump_step) {
                        log_skipped(steps, pc, target_idx, "IsActorKilled jump");
                        pc = target_idx;
                    } else {
                        log_tail_unreachable(steps, pc);
                        return;
                    }
                }
            }
            GameEvent::CheckSkill {
                skill_id,
                skill_level,
                jump_step,
            } => {
                // Check skill level of current active character.
                let var = openmm_data::enums::EvtVariable(0x38 + *skill_id);
                let current = party.get_member_var(party.active_target, var);
                if current >= *skill_level as i32 {
                    if let Some(target_idx) = steps.iter().position(|s| s.step >= *jump_step) {
                        log_skipped(steps, pc, target_idx, "CheckSkill jump");
                        pc = target_idx;
                    } else {
                        log_tail_unreachable(steps, pc);
                        return;
                    }
                }
            }
            GameEvent::SummonItem { item_id, x, y, z } => {
                warn!("STUB SummonItem: id={} at ({},{},{})", item_id, x, y, z);
            }
            GameEvent::CharacterAnimation { player, anim_id } => {
                debug!(
                    "CharacterAnimation: player={} anim={} (no portrait UI)",
                    player, anim_id
                );
            }
            GameEvent::PressAnyKey => {
                debug!("Marker: PressAnyKey");
                // Logic block marker.
            }
            GameEvent::SetTextureOutdoors {
                model,
                facet,
                texture_name,
            } => {
                info!(
                    "SetTextureOutdoors: model={} facet={} tex='{}'",
                    model, facet, texture_name
                );
                audio.texture_outdoors.write(ApplyTextureOutdoors {
                    model: *model,
                    facet: *facet,
                    texture_name: texture_name.clone(),
                });
            }
            GameEvent::CheckItemsCount {
                item_id,
                count,
                jump_step,
            } => {
                let current = world_state.game_vars.item_count(*item_id);
                if current >= *count {
                    if let Some(target_idx) = steps.iter().position(|s| s.step >= *jump_step) {
                        log_skipped(steps, pc, target_idx, "CheckItemsCount jump");
                        pc = target_idx;
                    } else {
                        log_tail_unreachable(steps, pc);
                        return;
                    }
                }
            }
            GameEvent::RemoveItems { item_id, count } => {
                info!("RemoveItems: id={} cnt={}", item_id, count);
                world_state.game_vars.remove_item(*item_id, *count);
            }
            GameEvent::InputString { params } => {
                warn!("STUB InputString: params={:02x?}", params);
            }
            GameEvent::SetNPCGroupNews { npc_group, news_id } => {
                warn!("STUB SetNPCGroupNews: group={} news={}", npc_group, news_id);
            }
            GameEvent::SetActorGroup { actor_id, group_id } => {
                info!("SetActorGroup: actor={} group={}", actor_id, group_id);
                world_state.game_vars.actor_groups.insert(*actor_id, *group_id);
            }
            GameEvent::NPCSetItem { npc_id, item_id, on } => {
                warn!("STUB NPCSetItem: npc={} item={} on={}", npc_id, item_id, on);
            }
            GameEvent::CanShowTopicIsActorKilled { actor_group, count } => {
                debug!(
                    "Marker: CanShowTopicIsActorKilled(group={} count={})",
                    actor_group, count
                );
                // Marker.
            }
            GameEvent::ChangeGroup { old_group, new_group } => {
                info!("ChangeGroup: {} -> {}", old_group, new_group);
                // Remap all actors currently in old_group to new_group.
                for g in world_state.game_vars.actor_groups.values_mut() {
                    if *g == *old_group {
                        *g = *new_group;
                    }
                }
            }
            GameEvent::ChangeGroupAlly { group_id, ally_group } => {
                info!("ChangeGroupAlly: group={} ally={}", group_id, ally_group);
                world_state.game_vars.actor_ally_groups.insert(*group_id, *ally_group);
            }
            GameEvent::CheckSeason { season, jump_step } => {
                // MM6 simplified season (0=winter?)
                let current = (audio.game_time.as_ref().map(|gt| gt.calendar_date().1).unwrap_or(1) - 1) / 3;
                if current as i32 == *season {
                    if let Some(target_idx) = steps.iter().position(|s| s.step >= *jump_step) {
                        log_skipped(steps, pc, target_idx, "CheckSeason jump");
                        pc = target_idx;
                    } else {
                        log_tail_unreachable(steps, pc);
                        return;
                    }
                }
            }
            GameEvent::ToggleActorGroupFlag { group_id, flag, on } => {
                let flag = *flag as u32;
                let on = *on != 0;
                info!("ToggleActorGroupFlag: group={} flag=0x{:x} on={}", group_id, flag, on);
                for (mut actor, mut vis) in entities.actors.iter_mut() {
                    if world_state.game_vars.actor_groups.get(&actor.ddm_id) == Some(group_id) {
                        apply_actor_flags(&mut actor, &mut vis, flag, on);
                    }
                }
            }
            GameEvent::ToggleChestFlag { chest_id, flag, on } => {
                warn!("STUB ToggleChestFlag: chest={} flag=0x{:x} on={}", chest_id, flag, on);
            }
            GameEvent::SetActorItem { actor_id, item_id, on } => {
                warn!("STUB SetActorItem: actor={} item={} on={}", actor_id, item_id, on);
            }
            GameEvent::OnDateTimer { timer_data } => {
                warn!("STUB OnDateTimer: data={:02x?}", timer_data);
            }
            GameEvent::EnableDateTimer { timer_id, on } => {
                warn!("STUB EnableDateTimer: id={} on={}", timer_id, on);
            }
            GameEvent::StopAnimation { decoration_id } => {
                warn!("STUB StopAnimation: deco={}", decoration_id);
            }
            GameEvent::SpecialJump { jump_value } => {
                if let Some(target_idx) = steps.iter().position(|s| s.step >= *jump_value as u8) {
                    log_skipped(steps, pc, target_idx, "SpecialJump");
                    pc = target_idx;
                } else {
                    log_tail_unreachable(steps, pc);
                    return;
                }
            }
            GameEvent::IsNPCInParty { npc_id, jump_step } => {
                warn!("STUB IsNPCInParty: npc={} (assuming fail)", npc_id);
                if let Some(target_idx) = steps.iter().position(|s| s.step >= *jump_step) {
                    log_skipped(steps, pc, target_idx, "IsNPCInParty fail");
                    pc = target_idx;
                } else {
                    log_tail_unreachable(steps, pc);
                    return;
                }
            }
            GameEvent::IsTotalBountyHuntingAwardInRange { min, max, jump_step } => {
                warn!(
                    "STUB IsTotalBountyHuntingAwardInRange: min={} max={} (assuming fail)",
                    min, max
                );
                if let Some(target_idx) = steps.iter().position(|s| s.step >= *jump_step) {
                    log_skipped(steps, pc, target_idx, "BountyHuntingRange fail");
                    pc = target_idx;
                } else {
                    log_tail_unreachable(steps, pc);
                    return;
                }
            }
            GameEvent::Unhandled {
                opcode,
                opcode_name,
                params,
            } => {
                warn!(
                    "Unhandled opcode: 0x{:02x} ({}) params={:02x?}",
                    opcode, opcode_name, params
                );
            }
        }
    }
}
