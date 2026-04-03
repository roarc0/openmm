use std::collections::VecDeque;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

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

/// Bundles audio writer + SoundManager to stay within Bevy's 16-param limit.
#[derive(SystemParam)]
struct AudioParams<'w> {
    ui_sound: bevy::ecs::message::MessageWriter<'w, PlayUiSoundEvent>,
    sound_manager: Option<Res<'w, SoundManager>>,
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

/// Map a building type string from 2devents.txt to its background image name.
fn building_background(building_type: &str) -> &'static str {
    let lower = building_type.to_lowercase();
    if lower.contains("weapon") {
        return "wepntabl";
    }
    if lower.contains("armor") {
        return "armory";
    }
    if lower.contains("magic") || lower.contains("guild") || lower.contains("alchemy") {
        return "magshelf";
    }
    if lower.contains("general") || lower.contains("store") {
        return "genshelf";
    }
    "evt02"
}

/// Load an icon from the LOD archive as a Bevy Image handle with nearest-neighbor sampling.
fn load_icon(name: &str, game_assets: &GameAssets, images: &mut Assets<Image>) -> Option<Handle<Image>> {
    let img = game_assets.game_lod().icon(name)?;
    let mut bevy_img = crate::assets::dynamic_to_bevy_image(img);
    bevy_img.sampler = bevy::image::ImageSampler::nearest();
    Some(images.add(bevy_img))
}

/// Resolve the background image for a building interaction.
fn resolve_building_image(
    house_id: u32,
    map_events: &MapEvents,
    game_assets: &GameAssets,
    images: &mut Assets<Image>,
) -> Option<Handle<Image>> {
    if let Some(houses) = map_events.houses.as_ref()
        && let Some(entry) = houses.houses.get(&house_id)
    {
        let pic_name = format!("evt{:02}", entry.picture_id);
        if let Some(handle) = load_icon(&pic_name, game_assets, images) {
            return Some(handle);
        }
        return load_icon(building_background(&entry.building_type), game_assets, images);
    }
    load_icon("evt02", game_assets, images)
}

/// Set cursor grab mode and visibility.
fn grab_cursor(cursor_query: &mut Query<&mut CursorOptions, With<PrimaryWindow>>, grab: bool) {
    if let Ok(mut cursor) = cursor_query.single_mut() {
        if grab {
            cursor.grab_mode = CursorGrabMode::Confined;
            cursor.visible = false;
        } else {
            cursor.grab_mode = CursorGrabMode::None;
            cursor.visible = true;
        }
    }
}

// ── Variable read/write helpers ────────────────────────────────────────

/// Read a game variable's current value.
fn get_variable(vars: &GameVariables, var: EvtVariable) -> i32 {
    if var.is_map_var() {
        return vars.map_vars[var.map_var_index().unwrap() as usize];
    }
    match var {
        EvtVariable::GOLD => vars.gold,
        EvtVariable::FOOD => vars.food,
        EvtVariable::REPUTATION_IS => vars.reputation,
        EvtVariable::QBITS => 0,          // compare uses contains check, handled separately
        EvtVariable::AUTONOTES_BITS => 0, // compare uses contains check
        _ => {
            debug!(
                "get_variable: unhandled variable {} (0x{:02x}), returning 0",
                var, var.0
            );
            0
        }
    }
}

/// Write a value to a game variable.
fn set_variable(vars: &mut GameVariables, var: EvtVariable, value: i32) {
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
        _ => {
            warn!(
                "  set_variable: unhandled variable {} (0x{:02x}) = {}",
                var, var.0, value
            );
        }
    }
}

/// Add to a game variable.
fn add_variable(vars: &mut GameVariables, var: EvtVariable, value: i32) {
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
        _ => {
            warn!(
                "  add_variable: unhandled variable {} (0x{:02x}) += {}",
                var, var.0, value
            );
        }
    }
}

/// Subtract from a game variable.
fn subtract_variable(vars: &mut GameVariables, var: EvtVariable, value: i32) {
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
        _ => {
            warn!(
                "  subtract_variable: unhandled variable {} (0x{:02x}) -= {}",
                var, var.0, value
            );
        }
    }
}

/// Evaluate a Compare condition. Returns true if condition is met (don't jump).
fn evaluate_compare(vars: &GameVariables, var: EvtVariable, value: i32) -> bool {
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
    let current = get_variable(vars, var);
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
    mut decoration_query: Query<(&DecorationInfo, &mut MeshMaterial3d<StandardMaterial>)>,
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

        info!("  [step {}] {}", step, event);

        match event {
            // ── Already implemented (side-effects) ───────────────────
            GameEvent::Hint { text, .. } => {
                footer.set(text);
            }
            GameEvent::SpeakInHouse { house_id } => {
                let image = map_events
                    .as_ref()
                    .and_then(|me| resolve_building_image(*house_id, me, &game_assets, &mut images))
                    .or_else(|| load_icon("evt02", &game_assets, &mut images));
                if let Some(image) = image {
                    commands.insert_resource(OverlayImage { image });
                    *hud_view = HudView::Building;
                    grab_cursor(&mut cursor_query, false);
                }
            }
            GameEvent::OpenChest { .. } => {
                if let Some(image) = load_icon("chest01", &game_assets, &mut images) {
                    // Play chest-open sound if available
                    if let Some(ref sm) = audio.sound_manager
                        && let Some(id) = sm.chest_open_sound_id
                    {
                        audio.ui_sound.write(PlayUiSoundEvent { sound_id: id });
                    }
                    commands.insert_resource(OverlayImage { image });
                    *hud_view = HudView::Chest;
                    grab_cursor(&mut cursor_query, false);
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
                // Stop processing this sequence and clear the queue
                event_queue.clear();
                return;
            }

            // ── Control flow (NOW WORKING) ───────────────────────────
            GameEvent::Compare { var, value, jump_step } => {
                if !evaluate_compare(&world_state.game_vars, *var, *value) {
                    // Condition failed — jump to target step
                    if let Some(target_idx) = steps.iter().position(|s| s.step >= *jump_step) {
                        debug!("  Compare failed -> jumping to step {}", jump_step);
                        pc = target_idx;
                    } else {
                        debug!("  Compare failed -> jump target step {} not found, ending", jump_step);
                        return;
                    }
                }
            }
            GameEvent::Jmp { target_step } => {
                if let Some(target_idx) = steps.iter().position(|s| s.step >= *target_step) {
                    debug!("  Jmp -> step {}", target_step);
                    pc = target_idx;
                } else {
                    debug!("  Jmp -> step {} not found, ending", target_step);
                    return;
                }
            }
            GameEvent::RandomGoTo { steps: goto_steps } => {
                if !goto_steps.is_empty() {
                    // Simple deterministic pick (first option) — proper RNG can be added later
                    let idx = (step as usize) % goto_steps.len();
                    let target_step = goto_steps[idx];
                    debug!("  RandomGoTo -> picked step {} from {:?}", target_step, goto_steps);
                    if let Some(target_idx) = sequence.steps.iter().position(|s| s.step >= target_step) {
                        pc = target_idx;
                    } else {
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
                add_variable(&mut world_state.game_vars, *var, *value);
            }
            GameEvent::Subtract { var, value } => {
                subtract_variable(&mut world_state.game_vars, *var, *value);
            }
            GameEvent::Set { var, value } => {
                set_variable(&mut world_state.game_vars, *var, *value);
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
                // Find the decoration entity by billboard_index and replace its material
                let target_idx = *decoration_id as usize;
                let mut found = false;
                for (deco_info, mut mat_handle) in decoration_query.iter_mut() {
                    if deco_info.billboard_index == target_idx {
                        // Load the new sprite and create a material for it
                        let sprite_lower = sprite_name.to_lowercase();
                        if let Some(img) = game_assets.game_lod().sprite(&sprite_lower) {
                            let bevy_img = crate::assets::dynamic_to_bevy_image(img);
                            let tex = images.add(bevy_img);
                            let new_mat = materials.add(StandardMaterial {
                                unlit: true,
                                base_color_texture: Some(tex),
                                alpha_mode: AlphaMode::Mask(0.5),
                                cull_mode: None,
                                double_sided: true,
                                perceptual_roughness: 1.0,
                                reflectance: 0.0,
                                ..default()
                            });
                            mat_handle.0 = new_mat;
                            found = true;
                        } else {
                            warn!("SetSprite: sprite '{}' not found in LOD", sprite_name);
                        }
                        break;
                    }
                }
                if !found {
                    debug!(
                        "SetSprite: decoration {} not found (may not have DecorationInfo)",
                        target_idx
                    );
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
                warn!(
                    "STUB CheckItemsCount: item={} count={} -> failing, jumping to step {}",
                    item_id, count, jump_step
                );
                if let Some(target_idx) = steps.iter().position(|s| s.step >= *jump_step) {
                    pc = target_idx;
                } else {
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
                warn!(
                    "STUB SetNPCTopic: npc={} topic={} event={}",
                    npc_id, topic_index, event_id
                );
            }
            GameEvent::MoveNPC { npc_id, map_id } => {
                warn!("STUB MoveNPC: npc={} map={}", npc_id, map_id);
            }
            GameEvent::SpeakNPC { npc_id } => {
                // For generated street NPCs (npc_id >= GENERATED_NPC_ID_BASE), look up generated_npcs.
                // For quest NPCs (npc_id < GENERATED_NPC_ID_BASE), look up npcdata.txt.
                let (portrait_name, npc_display_name) = if *npc_id >= crate::game::events::GENERATED_NPC_ID_BASE {
                    let entry = map_events.as_ref().and_then(|me| me.generated_npcs.get(npc_id));
                    let portrait = entry
                        .map(|g| format!("NPC{:03}", g.portrait))
                        .unwrap_or_else(|| format!("NPC{:03}", npc_id));
                    let name = entry.map(|g| g.name.clone());
                    (portrait, name)
                } else {
                    let portrait = map_events
                        .as_ref()
                        .and_then(|me| me.npc_table.as_ref())
                        .and_then(|t| t.portrait_name(*npc_id))
                        .unwrap_or_else(|| format!("NPC{:03}", npc_id));
                    let name = map_events
                        .as_ref()
                        .and_then(|me| me.npc_table.as_ref())
                        .and_then(|t| t.npc_name(*npc_id).map(str::to_string));
                    (portrait, name)
                };

                info!(
                    "SpeakNPC: npc_id={} portrait='{}' name={:?}",
                    npc_id, portrait_name, npc_display_name
                );

                // Resolve profession name from npcprof.txt for both generated and quest NPCs.
                let profession = if *npc_id >= crate::game::events::GENERATED_NPC_ID_BASE {
                    map_events
                        .as_ref()
                        .and_then(|me| me.generated_npcs.get(npc_id))
                        .filter(|g| g.profession_id > 0)
                        .and_then(|g| {
                            game_assets
                                .game_data()
                                .prof_table
                                .as_ref()
                                .and_then(|pt| pt.get(g.profession_id as u16))
                                .map(|p| p.name.clone())
                        })
                } else {
                    map_events
                        .as_ref()
                        .and_then(|me| me.npc_table.as_ref())
                        .and_then(|t| t.get(*npc_id))
                        .filter(|entry| entry.profession_id > 0)
                        .and_then(|entry| {
                            game_assets
                                .game_data()
                                .prof_table
                                .as_ref()
                                .and_then(|pt| pt.get(entry.profession_id as u16))
                                .map(|p| p.name.clone())
                        })
                };

                let portrait_img = game_assets
                    .game_lod()
                    .icon(&portrait_name)
                    .or_else(|| game_assets.game_lod().icon("npc001"));
                if let Some(portrait_img) = portrait_img {
                    let size = Vec2::new(portrait_img.width() as f32, portrait_img.height() as f32);
                    let mut bevy_img = crate::assets::dynamic_to_bevy_image(portrait_img);
                    bevy_img.sampler = bevy::image::ImageSampler::nearest();
                    let handle = images.add(bevy_img);
                    commands.insert_resource(crate::game::hud::NpcPortrait { image: handle, size });
                    // Original MM6 shows only the first name under the portrait.
                    let first_name = npc_display_name
                        .as_deref()
                        .and_then(|n| n.split_whitespace().next())
                        .unwrap_or_default()
                        .to_string();
                    commands.insert_resource(crate::game::hud::NpcProfile {
                        name: first_name,
                        profession,
                    });
                    *hud_view = HudView::NpcDialogue;
                    grab_cursor(&mut cursor_query, false);
                } else {
                    warn!(
                        "SpeakNPC: no portrait found for npc_id={} portrait='{}'",
                        npc_id, portrait_name
                    );
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
                // TODO: check actual kill count; for now, always fail (jump)
                warn!(
                    "STUB IsActorKilled: group={} count={} -> failing, jumping to step {}",
                    actor_group, count, jump_step
                );
                if let Some(target_idx) = steps.iter().position(|s| s.step >= *jump_step) {
                    pc = target_idx;
                } else {
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
                    var,
                    skill_level,
                    best,
                    party.active_target,
                    if pass { "pass" } else { "fail -> jump" }
                );
                if !pass {
                    if let Some(target_idx) = steps.iter().position(|s| s.step >= *jump_step) {
                        pc = target_idx;
                    } else {
                        return;
                    }
                }
            }
            GameEvent::CheckSeason { season, jump_step } => {
                let name = match season {
                    0 => "Winter",
                    1 => "Spring",
                    2 => "Summer",
                    3 => "Autumn",
                    _ => "Unknown",
                };
                warn!(
                    "STUB CheckSeason: {}({}) -> failing, jumping to step {}",
                    name, season, jump_step
                );
                if let Some(target_idx) = steps.iter().position(|s| s.step >= *jump_step) {
                    pc = target_idx;
                } else {
                    return;
                }
            }
            GameEvent::IsNPCInParty { npc_id, jump_step } => {
                // Always fail for now (NPC not in party)
                warn!(
                    "STUB IsNPCInParty: npc={} -> failing, jumping to step {}",
                    npc_id, jump_step
                );
                if let Some(target_idx) = steps.iter().position(|s| s.step >= *jump_step) {
                    pc = target_idx;
                } else {
                    return;
                }
            }
            GameEvent::IsTotalBountyHuntingAwardInRange { min, max, jump_step } => {
                warn!(
                    "STUB IsTotalBountyHuntingAwardInRange: min={} max={} -> failing, jumping to step {}",
                    min, max, jump_step
                );
                if let Some(target_idx) = steps.iter().position(|s| s.step >= *jump_step) {
                    pc = target_idx;
                } else {
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
                warn!("STUB SpecialJump: value={}", jump_value);
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
