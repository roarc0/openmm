use std::collections::VecDeque;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::{CursorOptions, PrimaryWindow};

use lod::enums::EvtVariable;
use lod::evt::{EvtFile, EvtStep, GameEvent};
use lod::odm::mm6_to_bevy;

use crate::GameState;
use crate::assets::GameAssets;
use crate::game::party::Party;

/// Bundles save + state transition to stay within Bevy's 16-param system limit.
#[derive(SystemParam)]
struct TransitionParams<'w> {
    save_data: ResMut<'w, crate::save::GameSave>,
    game_state: ResMut<'w, NextState<GameState>>,
}
use crate::game::events::MapEvents;
use crate::game::hud::{FooterText, HudView, OverlayImage};
use crate::game::interaction::DecorationInfo;
use crate::game::map_name::MapName;
use crate::game::sound::SoundManager;
use crate::game::sound::effects::PlayUiSoundEvent;
use crate::game::world_state::GameVariables;
use crate::states::loading::LoadRequest;

/// Bundles audio + mesh assets + game_time to stay within Bevy's 16-param limit.
#[derive(SystemParam)]
struct AudioParams<'w> {
    ui_sound: bevy::ecs::message::MessageWriter<'w, PlayUiSoundEvent>,
    sound_manager: Option<Res<'w, SoundManager>>,
    game_time: Option<Res<'w, crate::game::game_time::GameTime>>,
    meshes: ResMut<'w, Assets<Mesh>>,
}

/// An event sequence — a list of steps from one event_id, executed as a script.
#[derive(Clone)]
struct EventSequence {
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
            self.sequences.push_back(EventSequence { steps: steps.clone() });
        }
    }

    /// Pop the next sequence from the front.
    fn pop(&mut self) -> Option<EventSequence> {
        self.sequences.pop_front()
    }

    /// Enqueue a single synthesized event (not from an EvtFile).
    pub fn push_single(&mut self, event: lod::evt::GameEvent) {
        self.sequences.push_back(EventSequence {
            steps: vec![lod::evt::EvtStep { step: 0, event }],
        });
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
            .add_systems(Update, process_events.run_if(in_state(GameState::Game)));
    }
}

// ── Variable read/write helpers ────────────────────────────────────────

/// Read a game variable's current value.
fn get_variable(
    vars: &GameVariables,
    party: &Party,
    game_time: Option<&crate::game::game_time::GameTime>,
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
        EvtVariable::MONTH_IS => {
            game_time.map(|t| t.calendar_date().1 as i32).unwrap_or(1)
        }
        EvtVariable::HOUR_IS => {
            game_time.map(|t| t.hour() as i32).unwrap_or(9)
        }
        EvtVariable::DAY_OF_WEEK_IS => {
            game_time.map(|t| t.day_of_week() as i32).unwrap_or(0)
        }
        EvtVariable::DAY_OF_YEAR_IS => {
            game_time.map(|t| {
                let (_, m, d) = t.calendar_date();
                ((m - 1) * 28 + d) as i32 // MM6 uses 28-day months
            }).unwrap_or(1)
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

/// Evaluate a Compare condition. Returns true if condition is met (JUMP to jump_step).
/// MM6 Compare semantics: jump when condition is TRUE (e.g. "already done" → skip).
fn evaluate_compare(
    vars: &GameVariables,
    party: &Party,
    game_time: Option<&crate::game::game_time::GameTime>,
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
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
    mut hud_view: ResMut<HudView>,
    mut footer: ResMut<FooterText>,
    mut cursor_query: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mut transition: TransitionParams,
    mut blv_doors: Option<ResMut<crate::game::blv::BlvDoors>>,
    mut audio: AudioParams,
    mut world_state: ResMut<crate::game::world_state::WorldState>,
    mut party: ResMut<Party>,
    time: Res<Time>,
    mut decoration_query: Query<(&DecorationInfo, &mut MeshMaterial3d<StandardMaterial>, &mut Mesh3d, &mut Transform)>,
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
    let qb = game_assets.quest_bits();
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
            GameEvent::Hint { text, .. } => {
                footer.set(text);
            }
            GameEvent::SpeakInHouse { house_id } => {
                let image = map_events
                    .as_ref()
                    .and_then(|me| crate::game::events::resolve_building_image(*house_id, me, &game_assets, &mut images))
                    .or_else(|| game_assets.load_icon("evt02", &mut images));
                if let Some(image) = image {
                    commands.insert_resource(OverlayImage { image });
                    *hud_view = HudView::Building;
                    crate::game::hud::grab_cursor(&mut cursor_query, false);
                }
            }
            GameEvent::OpenChest { .. } => {
                if let Some(image) = game_assets.load_icon("chest01", &mut images) {
                    // Play chest-open sound if available
                    if let Some(ref sm) = audio.sound_manager
                        && let Some(id) = sm.chest_open_sound_id
                    {
                        audio.ui_sound.write(PlayUiSoundEvent { sound_id: id });
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
                let Ok(target) = MapName::try_from(map_name.as_str()) else {
                    warn!("MoveToMap: invalid map name '{}'", map_name);
                    return;
                };

                let pos = mm6_to_bevy(*x, *y, *z);
                let yaw = (*direction as f32) * std::f32::consts::TAU / 65536.0;

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
                    crate::game::blv::trigger_door(doors, *door_id as u32, action.as_u8());
                }
            }
            GameEvent::PlaySound { sound_id } => {
                audio.ui_sound.write(PlayUiSoundEvent { sound_id: *sound_id });
            }
            GameEvent::StatusText { text, .. } => {
                footer.set_status(text, 2.0, time.elapsed_secs_f64());
            }
            GameEvent::LocationName { text, .. } => {
                footer.set_status(text, 2.0, time.elapsed_secs_f64());
            }
            GameEvent::ShowMessage { text, .. } => {
                footer.set_status(text, 4.0, time.elapsed_secs_f64());
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
                if let Some(target) = lod::enums::EvtTargetCharacter::from_u8(*player) {
                    info!("  ForPartyMember: target = {:?}", target);
                    party.active_target = target;
                } else {
                    warn!("  ForPartyMember: unknown player byte {}", player);
                }
            }

            // ── Variable operations (NOW WORKING) ────────────────────
            GameEvent::Add { var, value } => {
                add_variable(&mut world_state.game_vars, &mut party, *var, *value);
            }
            GameEvent::Subtract { var, value } => {
                subtract_variable(&mut world_state.game_vars, &mut party, *var, *value);
            }
            GameEvent::Set { var, value } => {
                set_variable(&mut world_state.game_vars, &mut party, *var, *value);
            }

            // ── World operations (stubs with warns) ──────────────────
            GameEvent::SetSnow { on } => {
                warn!("STUB SetSnow: on={} (no weather system yet)", on);
            }
            GameEvent::SetFacesBit { face_id, bit, on } => {
                warn!("STUB SetFacesBit: face={} bit=0x{:x} on={}", face_id, bit, on);
            }
            GameEvent::ToggleActorFlag { actor_id, flag, on } => {
                warn!("STUB ToggleActorFlag: actor={} flag=0x{:x} on={}", actor_id, flag, on);
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
                let target = decoration_query
                    .iter()
                    .find(|(d, ..)| d.billboard_index == target_idx)
                    .map(|(d, ..)| (d.declist_id, d.ground_y));
                let Some((declist_id, ground_y)) = target else {
                    debug!("SetSprite: decoration {} not found", target_idx);
                    continue;
                };
                let _ = declist_id; // stored for future use (e.g. directional swap)
                let Some((new_mat, new_mesh, _new_w, new_h)) =
                    crate::game::entities::sprites::load_static_decoration_sprite(
                        sprite_name,
                        game_assets.lod_manager(),
                        game_assets.billboard_manager(),
                        &mut images,
                        &mut materials,
                        &mut audio.meshes,
                    )
                else {
                    warn!("SetSprite: sprite '{}' not found in LOD", sprite_name);
                    continue;
                };
                for (deco_info, mut mat_handle, mut mesh_handle, mut transform) in decoration_query.iter_mut() {
                    if deco_info.billboard_index == target_idx {
                        transform.translation.y = ground_y + new_h / 2.0;
                        mesh_handle.0 = new_mesh;
                        mat_handle.0 = new_mat;
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
                    "STUB SummonMonsters: id={} count={} pos=({},{},{})",
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
            GameEvent::GiveItem {
                strength,
                item_type,
                item_id,
            } => {
                warn!("STUB GiveItem: str={} type={} id={}", strength, item_type, item_id);
            }
            GameEvent::SummonItem { item_id, x, y, z } => {
                warn!("STUB SummonItem: id={} pos=({},{},{})", item_id, x, y, z);
            }
            GameEvent::CheckItemsCount {
                item_id,
                count,
                jump_step,
            } => {
                // TODO: check actual inventory count; for now, always fail (jump)
                warn!("STUB CheckItemsCount: item={} count={} (assuming fail)", item_id, count);
                if let Some(target_idx) = steps.iter().position(|s| s.step >= *jump_step) {
                    log_skipped(steps, pc, target_idx, "CheckItemsCount fail");
                    pc = target_idx;
                } else {
                    log_tail_unreachable(steps, pc);
                    return;
                }
            }
            GameEvent::RemoveItems { item_id, count } => {
                warn!("STUB RemoveItems: item={} count={}", item_id, count);
            }

            // ── NPC operations ───────────────────────────────────────
            GameEvent::SetNPCTopic {
                npc_id,
                topic_index,
                event_id,
            } => {
                info!("SetNPCTopic: npc={} topic={} event={}", npc_id, topic_index, event_id);
                // Store event override keyed by npc_id; topic_index is ignored (MM6 has one active topic).
                world_state.game_vars.npc_topics.insert(*npc_id as i32, *event_id as i32);
            }
            GameEvent::MoveNPC { npc_id, map_id } => {
                warn!("STUB MoveNPC: npc={} map={}", npc_id, map_id);
            }
            GameEvent::SpeakNPC { npc_id } => {
                if let Some((portrait, profile)) =
                    crate::game::hud::overlay::prepare_npc_dialogue(*npc_id, &map_events, &game_assets, &mut images)
                {
                    commands.insert_resource(portrait);
                    commands.insert_resource(profile);
                    *hud_view = HudView::NpcDialogue;
                    crate::game::hud::grab_cursor(&mut cursor_query, false);
                } else {
                    warn!("SpeakNPC: no portrait found for npc_id={}", npc_id);
                }
            }
            GameEvent::ChangeEvent { target, new_event_id } => {
                warn!("STUB ChangeEvent: target={} event={}", target, new_event_id);
            }
            GameEvent::SetNPCGreeting { npc_id, greeting_id } => {
                warn!("STUB SetNPCGreeting: npc={} greeting={}", npc_id, greeting_id);
            }
            GameEvent::SetNPCGroupNews { npc_group, news_id } => {
                warn!("STUB SetNPCGroupNews: group={} news={}", npc_group, news_id);
            }
            GameEvent::NPCSetItem { npc_id, item_id, on } => {
                warn!("STUB NPCSetItem: npc={} item={} on={}", npc_id, item_id, on);
            }

            // ── Character / UI ───────────────────────────────────────
            GameEvent::ShowFace { player, expression } => {
                warn!("STUB ShowFace: player={} expr={}", player, expression);
            }
            GameEvent::CharacterAnimation { player, anim_id } => {
                warn!("STUB CharacterAnimation: player={} anim={}", player, anim_id);
            }
            GameEvent::ShowMovie { movie_name } => {
                warn!("STUB ShowMovie: '{}'", movie_name);
            }
            GameEvent::PressAnyKey => {
                warn!("STUB PressAnyKey");
            }
            GameEvent::InputString { params } => {
                warn!("STUB InputString: params={:02x?}", params);
            }

            // ── Timer / conditional hooks ────────────────────────────
            GameEvent::OnTimer { .. }
            | GameEvent::OnLongTimer { .. }
            | GameEvent::OnDateTimer { .. }
            | GameEvent::OnMapReload
            | GameEvent::OnMapLeave => {
                // These are lifecycle hooks, not dispatched via the queue
            }
            GameEvent::EnableDateTimer { timer_id, on } => {
                warn!("STUB EnableDateTimer: timer={} on={}", timer_id, on);
            }

            // ── Dialogue conditions ──────────────────────────────────
            GameEvent::OnCanShowDialogItemCmp { var, value } => {
                warn!("STUB OnCanShowDialogItemCmp: {} == {}?", var, value);
            }
            GameEvent::EndCanShowDialogItem => {
                warn!("STUB EndCanShowDialogItem");
            }
            GameEvent::SetCanShowDialogItem { on } => {
                warn!("STUB SetCanShowDialogItem: on={}", on);
            }
            GameEvent::CanShowTopicIsActorKilled { actor_group, count } => {
                warn!("STUB CanShowTopicIsActorKilled: group={} count={}", actor_group, count);
            }

            // ── Skills / kill / condition checks ─────────────────────
            GameEvent::IsActorKilled {
                actor_group,
                count,
                jump_step,
            } => {
                warn!("STUB IsActorKilled: group={} count={} (assuming fail)", actor_group, count);
                if let Some(target_idx) = steps.iter().position(|s| s.step >= *jump_step) {
                    log_skipped(steps, pc, target_idx, "IsActorKilled fail");
                    pc = target_idx;
                } else {
                    log_tail_unreachable(steps, pc);
                    return;
                }
            }
            GameEvent::CheckSkill {
                skill_id,
                skill_level,
                jump_step,
            } => {
                let var = EvtVariable(*skill_id);
                let best = party.max_skill(party.active_target, var);
                let pass = best >= *skill_level;
                info!(
                    "  CheckSkill: {} level {} required, best={} target={:?} -> {}",
                    var, skill_level, best, party.active_target,
                    if pass { "pass" } else { "fail" }
                );
                if !pass {
                    if let Some(target_idx) = steps.iter().position(|s| s.step >= *jump_step) {
                        log_skipped(steps, pc, target_idx, "CheckSkill fail");
                        pc = target_idx;
                    } else {
                        log_tail_unreachable(steps, pc);
                        return;
                    }
                }
            }
            GameEvent::CheckSeason { season, jump_step } => {
                let current_season = audio.game_time.as_deref().map(|gt| {
                    let (_, month, _) = gt.calendar_date();
                    match month {
                        3..=5 => 1u8,
                        6..=8 => 2,
                        9..=11 => 3,
                        _ => 0,
                    }
                });
                let season_name = match season {
                    0 => "Winter", 1 => "Spring", 2 => "Summer", 3 => "Autumn", _ => "?",
                };
                let matches = current_season == Some(*season as u8);
                info!(
                    "  CheckSeason: want {} current={:?} -> {}",
                    season_name, current_season, if matches { "pass" } else { "fail" }
                );
                if !matches {
                    if let Some(target_idx) = steps.iter().position(|s| s.step >= *jump_step) {
                        log_skipped(steps, pc, target_idx, "CheckSeason fail");
                        pc = target_idx;
                    } else {
                        log_tail_unreachable(steps, pc);
                        return;
                    }
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
                warn!("STUB IsTotalBountyHuntingAwardInRange: min={} max={} (assuming fail)", min, max);
                if let Some(target_idx) = steps.iter().position(|s| s.step >= *jump_step) {
                    log_skipped(steps, pc, target_idx, "BountyHuntingRange fail");
                    pc = target_idx;
                } else {
                    log_tail_unreachable(steps, pc);
                    return;
                }
            }

            // ── Actor / group operations ─────────────────────────────
            GameEvent::SetActorGroup { actor_id, group_id } => {
                warn!("STUB SetActorGroup: actor={} group={}", actor_id, group_id);
            }
            GameEvent::ChangeGroup { old_group, new_group } => {
                warn!("STUB ChangeGroup: old={} new={}", old_group, new_group);
            }
            GameEvent::ChangeGroupAlly { group_id, ally_group } => {
                warn!("STUB ChangeGroupAlly: group={} ally={}", group_id, ally_group);
            }
            GameEvent::ToggleActorGroupFlag { group_id, flag, on } => {
                warn!(
                    "STUB ToggleActorGroupFlag: group={} flag=0x{:x} on={}",
                    group_id, flag, on
                );
            }
            GameEvent::SetActorItem { actor_id, item_id, on } => {
                warn!("STUB SetActorItem: actor={} item={} on={}", actor_id, item_id, on);
            }
            GameEvent::StopAnimation { decoration_id } => {
                warn!("STUB StopAnimation: deco={}", decoration_id);
            }
            GameEvent::ToggleChestFlag { chest_id, flag, on } => {
                warn!("STUB ToggleChestFlag: chest={} flag=0x{:x} on={}", chest_id, flag, on);
            }
            GameEvent::SpecialJump { jump_value } => {
                let target_step = *jump_value as u8;
                if let Some(target_idx) = steps.iter().position(|s| s.step >= target_step) {
                    log_skipped(steps, pc, target_idx, "SpecialJump");
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

#[cfg(test)]
mod tests {
    use super::*;
    use lod::enums::EvtVariable;

    fn make_vars() -> GameVariables {
        GameVariables::default()
    }

    fn make_party() -> Party {
        Party::default()
    }

    fn cmp(vars: &GameVariables, party: &Party, var: EvtVariable, value: i32) -> bool {
        evaluate_compare(vars, party, None, var, value)
    }

    /// MM6 Compare: condition MET (true) → jump (skip). NOT met → fall through.
    /// Regression for apple tree events in oute3: Compare(MapVar9 >= 1)
    ///   - first click (MapVar9=0): 0 >= 1 = FALSE → don't jump → pick apple
    ///   - second click (MapVar9=1): 1 >= 1 = TRUE → jump → already picked, skip
    #[test]
    fn compare_jumps_when_condition_met_not_when_unmet() {
        // MapVar9 = EvtVariable(0x69 + 9) = EvtVariable(0x72)
        let map_var9 = EvtVariable(0x69 + 9);
        let party = make_party();

        let mut vars = make_vars();
        // First click: not yet picked (MapVar9 = 0)
        vars.map_vars[9] = 0;
        assert!(
            !cmp(&vars, &party, map_var9, 1),
            "MapVar9=0 >= 1 should be FALSE (don't jump, fall through to pick apple)"
        );

        // Second click: already picked (MapVar9 = 1)
        vars.map_vars[9] = 1;
        assert!(
            cmp(&vars, &party, map_var9, 1),
            "MapVar9=1 >= 1 should be TRUE (jump to skip — tree already picked)"
        );
    }

    #[test]
    fn compare_qbit_jumps_when_set() {
        let mut vars = make_vars();
        let party = make_party();
        // QBit not set → FALSE (don't jump, can do quest)
        assert!(!cmp(&vars, &party, EvtVariable::QBITS, 5));
        // QBit set → TRUE (jump, quest already done)
        vars.set_qbit(5);
        assert!(cmp(&vars, &party, EvtVariable::QBITS, 5));
    }
}
