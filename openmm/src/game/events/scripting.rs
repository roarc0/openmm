use std::collections::VecDeque;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::{CursorOptions, PrimaryWindow};

use openmm_data::enums::{ActorAttributes, EvtVariable};
use openmm_data::evt::{EvtFile, EvtStep, GameEvent};

use crate::GameState;
use crate::assets::GameAssets;
use crate::game::player::party::Party;
use crate::game::sprites::material::SpriteMaterial;

/// Bundles save + state transition to stay within Bevy's 16-param system limit.
#[derive(SystemParam)]
pub(super) struct TransitionParams<'w> {
    pub save_data: ResMut<'w, crate::system::save::GameSave>,
    pub game_state: ResMut<'w, NextState<GameState>>,
}
use super::event_handlers;
use super::events::MapEvents;
use crate::game::actors::Actor;
use crate::game::interaction::DecorationInfo;
use crate::game::map::outdoor::ApplyTextureOutdoors;
use crate::game::optional::OptionalWrite;
use crate::game::sound::SoundManager;
use crate::game::sound::effects::PlayUiSoundEvent;
use crate::game::state::variables;
use crate::game::ui::UiState;

/// Bundles map entity queries to stay within Bevy's 16-param system limit.
/// Wraps the decoration sprite-swap query and actor visibility/flag query.
#[derive(SystemParam)]
pub(super) struct MapEntityParams<'w, 's> {
    pub decorations: Query<
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
    pub actors: Query<'w, 's, (&'static mut Actor, &'static mut Visibility)>,
    pub player: Query<'w, 's, &'static mut Transform, With<crate::game::player::Player>>,
    pub player_settings: Res<'w, crate::game::player::PlayerSettings>,
}

/// Bundles audio + mesh assets + game_time to stay within Bevy's 16-param limit.
#[derive(SystemParam)]
pub(super) struct AudioParams<'w> {
    pub ui_sound: Option<bevy::ecs::message::MessageWriter<'w, PlayUiSoundEvent>>,
    pub texture_outdoors: bevy::ecs::message::MessageWriter<'w, ApplyTextureOutdoors>,
    pub sound_manager: Option<Res<'w, SoundManager>>,
    pub game_time: Option<Res<'w, crate::game::state::GameTime>>,
    pub meshes: ResMut<'w, Assets<Mesh>>,
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

/// Helper to handle conditional jumps in event scripts.
/// Finds the target step index, logs skipped steps, and updates the program counter.
/// Returns true if execution should continue, false if the sequence was terminated.
fn execute_conditional_jump(steps: &[EvtStep], pc: &mut usize, jump_step: u8, reason: &str) -> bool {
    if let Some(target_idx) = steps.iter().position(|s| s.step >= jump_step) {
        log_skipped(steps, *pc, target_idx, reason);
        *pc = target_idx;
        true
    } else {
        log_tail_unreachable(steps, *pc);
        false
    }
}

/// Macro for concise stub logging in event arms.
macro_rules! stub_event {
    ($name:literal, $fmt:literal) => {
        warn!(concat!("STUB ", $name, ": ", $fmt))
    };
    ($name:literal, $fmt:literal, $($arg:tt)*) => {
        warn!(concat!("STUB ", $name, ": ", $fmt), $($arg)*)
    };
}

/// Apply a single ActorAttributes flag change to a live actor entity.
/// Handles VISIBLE -> Bevy Visibility and HOSTILE -> actor.hostile. Other bits
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

/// Process one event sequence per frame from the EventQueue.
/// Each sequence is executed as a script with control flow (Compare/Jmp/RandomGoTo).
fn process_events(
    mut event_queue: ResMut<EventQueue>,
    map_events: Option<Res<MapEvents>>,
    game_assets: Res<GameAssets>,
    mut images: ResMut<Assets<Image>>,
    mut sprite_materials: Option<ResMut<Assets<SpriteMaterial>>>,
    mut commands: Commands,
    mut ui: ResMut<UiState>,
    mut cursor_query: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mut transition: TransitionParams,
    mut blv_doors: Option<ResMut<crate::game::map::indoor::BlvDoors>>,
    mut audio: AudioParams,
    mut world_state: ResMut<crate::game::state::WorldState>,
    mut party: ResMut<Party>,
    time: Res<Time>,
    mut entities: MapEntityParams,
) {
    // Don't process events while a UI overlay is blocking
    if !matches!(ui.mode, crate::game::ui::UiMode::World) {
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
                ui.footer.set(text);
            }
            GameEvent::SpeakInHouse { house_id } => {
                event_handlers::handle_speak_in_house(
                    *house_id,
                    &game_assets,
                    &map_events,
                    &mut images,
                    &mut commands,
                    &mut ui,
                    &mut cursor_query,
                    &time,
                );
            }
            GameEvent::OpenChest { id } => {
                event_handlers::handle_open_chest(
                    *id,
                    &game_assets,
                    &mut images,
                    &mut commands,
                    &mut ui,
                    &mut cursor_query,
                    &mut audio,
                );
            }
            GameEvent::MoveToMap {
                x,
                y,
                z,
                direction,
                map_name,
            } => {
                event_handlers::handle_move_to_map(
                    map_name,
                    *x,
                    *y,
                    *z,
                    *direction,
                    &mut event_queue,
                    &mut audio,
                    &mut entities,
                    &mut transition,
                    &mut world_state,
                    &mut commands,
                );
                return; // MoveToMap always terminates the sequence
            }
            GameEvent::ChangeDoorState { door_id, action } => {
                debug!("ChangeDoorState door_id={} action={}", door_id, action);
                if let Some(ref mut doors) = blv_doors {
                    crate::game::map::indoor::trigger_door(doors, *door_id as u32, action.as_u8());
                }
            }
            GameEvent::PlaySound { sound_id } => {
                audio.ui_sound.try_write(PlayUiSoundEvent { sound_id: *sound_id });
            }
            GameEvent::StatusText { str_id, text } => {
                debug!("StatusText(id={}): {}", str_id, text);
                ui.footer.set_status(text, 2.0, time.elapsed_secs_f64());
            }
            GameEvent::LocationName { str_id, text } => {
                debug!("LocationName(id={}): {}", str_id, text);
                ui.footer.set_status(text, 2.0, time.elapsed_secs_f64());
            }
            GameEvent::ShowMessage { str_id, text } => {
                debug!("ShowMessage(id={}): {}", str_id, text);
                ui.footer.set_status(text, 4.0, time.elapsed_secs_f64());
            }
            GameEvent::PlayVideo { name, skippable: _ } => {
                // TODO: wire PlayVideo through the screen system (InlineVideo).
                warn!("PlayVideo('{}') — not yet wired to screen runtime", name);
            }
            GameEvent::Exit => {
                log_tail_unreachable(steps, pc);
                event_queue.clear();
                return;
            }

            // ── Control flow (NOW WORKING) ───────────────────────────
            GameEvent::Compare { var, value, jump_step } => {
                if variables::evaluate_compare(&world_state.game_vars, &party, audio.game_time.as_deref(), *var, *value)
                    && !execute_conditional_jump(steps, &mut pc, *jump_step, "Compare true")
                {
                    return;
                }
            }
            GameEvent::Jmp { target_step } => {
                if !execute_conditional_jump(steps, &mut pc, *target_step, "Jmp") {
                    return;
                }
            }
            GameEvent::RandomGoTo { steps: goto_steps } => {
                if !goto_steps.is_empty() {
                    let idx = (step as usize) % goto_steps.len();
                    let target_step = goto_steps[idx];
                    debug!("  RandomGoTo -> picked step {} from {:?}", target_step, goto_steps);
                    if !execute_conditional_jump(steps, &mut pc, target_step, "RandomGoTo") {
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
                variables::add_variable(&mut world_state.game_vars, &mut party, *var, *value);
                if is_pickup {
                    event_handlers::play_ui_sound_named("Quest", audio.sound_manager.as_deref(), &mut audio.ui_sound);
                }
                if show_note {
                    event_handlers::show_autonote_text(*value, &game_assets, &mut ui, time.elapsed_secs_f64());
                }
            }
            GameEvent::Subtract { var, value } => {
                variables::subtract_variable(&mut world_state.game_vars, &mut party, *var, *value);
            }
            GameEvent::Set { var, value } => {
                let show_note =
                    *var == EvtVariable::AUTONOTES_BITS && *value != 0 && !world_state.game_vars.has_autonote(*value);
                variables::set_variable(&mut world_state.game_vars, &mut party, *var, *value);
                if show_note {
                    event_handlers::show_autonote_text(*value, &game_assets, &mut ui, time.elapsed_secs_f64());
                }
            }

            // ── World operations (stubs with warns) ──────────────────
            GameEvent::SetSnow { on } => {
                info!("SetSnow: on={} (no weather system)", on);
            }
            GameEvent::SetFacesBit { face_id, bit, on } => {
                stub_event!("SetFacesBit", "face={} bit=0x{:x} on={}", face_id, bit, on);
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
                stub_event!("SetTexture", "face={} tex='{}'", face_id, texture_name);
            }
            GameEvent::SetSprite {
                decoration_id,
                sprite_name,
            } => {
                event_handlers::handle_set_sprite(
                    *decoration_id,
                    sprite_name,
                    &game_assets,
                    &mut images,
                    sprite_materials.as_deref_mut(),
                    &mut audio.meshes,
                    &mut entities,
                );
            }
            GameEvent::ToggleIndoorLight { light_id, on } => {
                stub_event!("ToggleIndoorLight", "light={} on={}", light_id, on);
            }

            // ── Combat / items ───────────────────────────────────────
            GameEvent::SummonMonsters {
                monster_id,
                count,
                x,
                y,
                z,
            } => {
                stub_event!(
                    "SummonMonsters",
                    "id={} count={} at ({},{},{})",
                    monster_id,
                    count,
                    x,
                    y,
                    z
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
                stub_event!(
                    "CastSpell",
                    "spell={} level={} mastery={} from=({},{},{}) to=({},{},{})",
                    spell_id,
                    skill_level,
                    skill_mastery,
                    from_x,
                    from_y,
                    from_z,
                    to_x,
                    to_y,
                    to_z
                );
            }
            GameEvent::ReceiveDamage { damage_type, amount } => {
                stub_event!("ReceiveDamage", "type={} amount={}", damage_type, amount);
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
                event_handlers::play_ui_sound_named("Quest", audio.sound_manager.as_deref(), &mut audio.ui_sound);
                info!("GiveItem: id={} str={} type={}", item_id, strength, item_type);
            }
            GameEvent::SetNPCTopic {
                npc_id,
                topic_index,
                event_id,
            } => {
                stub_event!("SetNPCTopic", "npc={} topic={} event={}", npc_id, topic_index, event_id);
            }
            GameEvent::MoveNPC { npc_id, map_id } => {
                stub_event!("MoveNPC", "npc={} map_id={}", npc_id, map_id);
            }
            GameEvent::SpeakNPC { npc_id } => {
                event_handlers::handle_speak_npc(
                    *npc_id,
                    &game_assets,
                    &map_events,
                    &mut images,
                    &mut commands,
                    &mut ui,
                    &mut cursor_query,
                    &audio,
                    &world_state,
                );
            }
            GameEvent::ChangeEvent { target, new_event_id } => {
                stub_event!("ChangeEvent", "target={} event={}", target, new_event_id);
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
                stub_event!(
                    "OnTimer trigger",
                    "{:04}-{:02}-{:02} (week {}) {:02}:{:02}",
                    year,
                    month,
                    day,
                    week,
                    hour,
                    minute
                );
            }
            GameEvent::OnLongTimer { timer_data } => {
                stub_event!("OnLongTimer trigger", "data={:02x?}", timer_data);
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
                if killed >= *count as u32
                    && !execute_conditional_jump(steps, &mut pc, *jump_step, "IsActorKilled jump")
                {
                    return;
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
                if current >= *skill_level as i32
                    && !execute_conditional_jump(steps, &mut pc, *jump_step, "CheckSkill jump")
                {
                    return;
                }
            }
            GameEvent::SummonItem { item_id, x, y, z } => {
                stub_event!("SummonItem", "id={} at ({},{},{})", item_id, x, y, z);
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
                if current >= *count && !execute_conditional_jump(steps, &mut pc, *jump_step, "CheckItemsCount jump") {
                    return;
                }
            }
            GameEvent::RemoveItems { item_id, count } => {
                info!("RemoveItems: id={} cnt={}", item_id, count);
                world_state.game_vars.remove_item(*item_id, *count);
            }
            GameEvent::InputString { params } => {
                stub_event!("InputString", "params={:02x?}", params);
            }
            GameEvent::SetNPCGroupNews { npc_group, news_id } => {
                stub_event!("SetNPCGroupNews", "group={} news={}", npc_group, news_id);
            }
            GameEvent::SetActorGroup { actor_id, group_id } => {
                info!("SetActorGroup: actor={} group={}", actor_id, group_id);
                world_state.game_vars.actor_groups.insert(*actor_id, *group_id);
            }
            GameEvent::NPCSetItem { npc_id, item_id, on } => {
                stub_event!("NPCSetItem", "npc={} item={} on={}", npc_id, item_id, on);
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
                if current as i32 == *season
                    && !execute_conditional_jump(steps, &mut pc, *jump_step, "CheckSeason jump")
                {
                    return;
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
                stub_event!("ToggleChestFlag", "chest={} flag=0x{:x} on={}", chest_id, flag, on);
            }
            GameEvent::SetActorItem { actor_id, item_id, on } => {
                stub_event!("SetActorItem", "actor={} item={} on={}", actor_id, item_id, on);
            }
            GameEvent::OnDateTimer { timer_data } => {
                stub_event!("OnDateTimer", "data={:02x?}", timer_data);
            }
            GameEvent::EnableDateTimer { timer_id, on } => {
                stub_event!("EnableDateTimer", "id={} on={}", timer_id, on);
            }
            GameEvent::StopAnimation { decoration_id } => {
                stub_event!("StopAnimation", "deco={}", decoration_id);
            }
            GameEvent::SpecialJump { jump_value } => {
                if !execute_conditional_jump(steps, &mut pc, *jump_value as u8, "SpecialJump") {
                    return;
                }
            }
            GameEvent::IsNPCInParty { npc_id, jump_step } => {
                stub_event!("IsNPCInParty", "npc={} (assuming fail)", npc_id);
                if !execute_conditional_jump(steps, &mut pc, *jump_step, "IsNPCInParty fail") {
                    return;
                }
            }
            GameEvent::IsTotalBountyHuntingAwardInRange { min, max, jump_step } => {
                stub_event!(
                    "IsTotalBountyHuntingAwardInRange",
                    "min={} max={} (assuming fail)",
                    min,
                    max
                );
                if !execute_conditional_jump(steps, &mut pc, *jump_step, "BountyHuntingRange fail") {
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
