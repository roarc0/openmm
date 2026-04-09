//! Monster AI state machine: wander and aggro + obstacle-steering.
//!
//! Monsters start in `Wander` mode, patrolling near their guard position.
//! When the player enters `actor.aggro_range`, they switch to `Aggro` and
//! chase the player using probe-based steering to navigate around walls.
//!
//! ## Steering algorithm
//! Each frame in aggro mode:
//!  1. Try the direct heading toward the player.
//!  2. If the direct path is heavily blocked (< 70 % of intended movement
//!     achieved after `resolve_movement`), probe ±20 / ±40 / ±70 / ±110°.
//!  3. Pick the heading with the best forward progress.
//!
//! This is O(probes × walls) per actor per frame — fast enough for any
//! realistic group size. Works for both outdoor (no colliders → direct path)
//! and indoor BSP geometry.

use bevy::{ecs::message::MessageWriter, prelude::*};

use crate::GameState;
use crate::game::actors::Actor;
use crate::game::actors::combat::{ActorDead, DyingTimer};
use crate::game::actors::physics::{is_passable, snap_actor_y};
use crate::game::collision::{BuildingColliders, TerrainHeightMap, WaterMap};
use crate::game::hud::HudView;
use crate::game::indoor::DoorColliders;
use crate::game::optional::OptionalWrite;
use crate::game::player::Player;
use crate::game::sound::effects::PlayOnceSoundEvent;
use crate::game::sprites::{AnimationState, WorldEntity};
use openmm_data::ActorSoundSlot;

/// Heading offsets (degrees) probed when the direct path is blocked.
/// Symmetric left/right pairs; wider angles last so narrower detours win ties.
const PROBE_OFFSETS_DEG: &[f32] = &[20.0, -20.0, 40.0, -40.0, 70.0, -70.0, 110.0, -110.0];

/// If the direct path delivers < this fraction of intended movement, try probes.
const BLOCK_THRESHOLD: f32 = 0.7;

/// Actor radius used in `resolve_movement` — matches player collision (actors are smaller).
const ACTOR_RADIUS: f32 = 20.0;
/// Eye-height passed to `resolve_movement` — half a typical monster height.
const ACTOR_EYE_HEIGHT: f32 = 140.0;

/// Current AI mode for a monster entity.
#[derive(Component, Default, PartialEq, Clone, Copy, Debug)]
pub enum MonsterAiMode {
    /// Patrolling near guard position.
    #[default]
    Wander,
    /// Chasing the player.
    Aggro,
}

pub struct MonsterAiPlugin;

impl Plugin for MonsterAiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            monster_ai_system
                .run_if(in_state(GameState::Game))
                .run_if(resource_equals(HudView::World)),
        );
    }
}

/// Try to move `from` toward `target_pos` by `speed` units, steering around walls.
///
/// Returns `(destination, facing_yaw)`.
/// When `colliders` is `None` (outdoor, no buildings) the direct path is always used.
fn steer_toward(
    from: Vec3,
    target_pos: Vec3,
    speed: f32,
    colliders: Option<&BuildingColliders>,
    door_colliders: Option<&DoorColliders>,
) -> (Vec3, f32) {
    let flat = Vec3::new(target_pos.x - from.x, 0.0, target_pos.z - from.z);
    if flat.length_squared() < 1.0 {
        return (from, 0.0);
    }
    let base_yaw = flat.x.atan2(flat.z); // yaw pointing toward target

    let probe_dest = |yaw: f32| -> Vec3 {
        let dir = Vec3::new(yaw.sin(), 0.0, yaw.cos());
        let intended = from + dir * speed;
        let mut pos = intended;
        if let Some(c) = colliders {
            pos = c.resolve_movement(from, pos, ACTOR_RADIUS, ACTOR_EYE_HEIGHT);
        }
        if let Some(dc) = door_colliders {
            // Apply dynamic door wall collision
            pos = dc.resolve_movement(from, pos, ACTOR_RADIUS, ACTOR_EYE_HEIGHT);
            // Block movement into closed horizontal door panels (trapdoors, slabs)
            let feet_y = pos.y - ACTOR_EYE_HEIGHT;
            if dc.blocks_entry(pos.x, pos.z, feet_y, pos.y, ACTOR_RADIUS) {
                pos = from;
            }
        }
        pos
    };

    // Direct heading first.
    let direct = probe_dest(base_yaw);
    let direct_progress = (direct - from).length();

    // If mostly unblocked, go direct.
    if direct_progress >= speed * BLOCK_THRESHOLD {
        return (direct, base_yaw);
    }

    // Blocked — probe side angles, pick best forward progress.
    let mut best_dest = direct;
    let mut best_progress = direct_progress;
    let mut best_yaw = base_yaw;

    for &deg in PROBE_OFFSETS_DEG {
        let yaw = base_yaw + deg.to_radians();
        let dest = probe_dest(yaw);
        let progress = (dest - from).length();
        if progress > best_progress {
            best_progress = progress;
            best_dest = dest;
            best_yaw = yaw;
        }
    }

    (best_dest, best_yaw)
}

fn monster_ai_system(
    time: Res<Time>,
    colliders: Option<Res<BuildingColliders>>,
    door_colliders: Option<Res<DoorColliders>>,
    terrain: Option<Res<TerrainHeightMap>>,
    water_map: Option<Res<WaterMap>>,
    player: Query<&Transform, With<Player>>,
    mut query: Query<
        (&mut Transform, &mut Actor, &mut AnimationState, &mut MonsterAiMode),
        (
            With<WorldEntity>,
            Without<DyingTimer>,
            Without<ActorDead>,
            Without<Player>,
        ),
    >,
    mut sounds: Option<MessageWriter<PlayOnceSoundEvent>>,
) {
    let Ok(player_tf) = player.single() else {
        return;
    };
    let player_pos = player_tf.translation;
    let dt = time.delta_secs();
    let c = colliders.as_deref();
    let dc = door_colliders.as_deref();
    let hm = terrain.as_deref();
    let wm = water_map.as_deref();

    // Sound events must be collected to avoid capturing the non-clonable MessageWriter in par_iter
    let pending_sounds = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));

    query
        .par_iter_mut()
        .for_each(|(mut transform, mut actor, mut anim_state, mut ai_mode)| {
            let sounds = &pending_sounds;
            if actor.hp <= 0 {
                return;
            }

            if actor.move_speed < 1.0 {
                return;
            }

            // Let attack animation play out without interference.
            if matches!(*anim_state, AnimationState::Attacking) {
                return;
            }

            // Fidget sound: play slot [Fidget] periodically while wandering.
            let fidget_sound = actor.sound_ids[ActorSoundSlot::Fidget as usize];
            if *ai_mode == MonsterAiMode::Wander && fidget_sound > 0 {
                actor.fidget_timer -= dt;
                if actor.fidget_timer <= 0.0 {
                    // Randomise next fidget interval: 10–25s, position-seeded for variance.
                    let seed = (transform.translation.x * 0.031 + transform.translation.z * 0.017)
                        .abs()
                        .fract();
                    actor.fidget_timer = 10.0 + seed * 15.0;
                    sounds.lock().unwrap().push(PlayOnceSoundEvent {
                        sound_id: fidget_sound as u32,
                        position: transform.translation,
                    });
                }
            }

            let my_pos = transform.translation;
            let dist_sq = my_pos.distance_squared(player_pos);

            // ── Aggro state transitions ──────────────────────────────────────────
            if actor.hostile && actor.aggro_range > 0.0 {
                // ai_type modifies effective aggro radius:
                //   Aggress  → aggros from 50% further (sees you coming)
                //   Wimp     → aggros only at 60% of normal range (cautious)
                //   Suicidal → always aggros if in LOS (treat as very large range)
                //   Normal   → base range
                let effective_aggro = match actor.ai_type.as_str() {
                    "Aggress" => actor.aggro_range * 1.5,
                    "Wimp" => actor.aggro_range * 0.6,
                    "Suicidal" => actor.aggro_range * 3.0,
                    _ => actor.aggro_range,
                };
                let aggro_sq = effective_aggro * effective_aggro;
                // Leash at 2× effective aggro radius so the monster doesn't instantly de-aggro.
                let leash_sq = (effective_aggro * 2.0) * (effective_aggro * 2.0);

                match *ai_mode {
                    MonsterAiMode::Wander if dist_sq < aggro_sq => {
                        *ai_mode = MonsterAiMode::Aggro;
                        *anim_state = AnimationState::Walking;
                        debug!("Monster '{}' aggros player", actor.name);
                        // Battle-growl on aggro (got_hit slot reused as alert sound).
                        let alert_sound = actor.sound_ids[ActorSoundSlot::GotHit as usize];
                        if alert_sound > 0 {
                            sounds.lock().unwrap().push(PlayOnceSoundEvent {
                                sound_id: alert_sound as u32,
                                position: my_pos,
                            });
                        }
                    }
                    MonsterAiMode::Aggro if dist_sq > leash_sq => {
                        *ai_mode = MonsterAiMode::Wander;
                        // Walk straight back to guard position. Large timer so the
                        // wander tick doesn't interrupt the return walk mid-trip.
                        actor.wander_target = actor.guarding_position;
                        actor.wander_timer = 60.0;
                        *anim_state = AnimationState::Walking;
                        debug!("Monster '{}' leashes back to guard position", actor.name);
                    }
                    _ => {}
                }
            }

            // ── Aggro: chase the player with obstacle steering ───────────────────
            if *ai_mode == MonsterAiMode::Aggro {
                let stop_sq = actor.attack_range * actor.attack_range;

                if dist_sq <= stop_sq {
                    // In attack range — stand still, let monster_attack_system fire.
                    if *anim_state == AnimationState::Walking {
                        *anim_state = AnimationState::Idle;
                    }
                } else {
                    *anim_state = AnimationState::Walking;
                    let speed = actor.move_speed * dt;

                    // Per-monster lateral jitter so a group fans out instead of
                    // stacking on the same spot. Seed from initial position so each
                    // monster gets a stable, unique offset that wobbles gently.
                    let jitter_seed = actor.initial_position.x * 3.7 + actor.initial_position.z * 6.1;
                    let jitter_phase = jitter_seed + time.elapsed_secs() * 0.3;
                    let jitter_angle = jitter_phase.sin() * 0.4; // ±0.4 rad ≈ ±23°
                    let flat_to_player =
                        Vec3::new(player_pos.x - my_pos.x, 0.0, player_pos.z - my_pos.z).normalize_or_zero();
                    let perp = Vec3::new(-flat_to_player.z, 0.0, flat_to_player.x);
                    let jitter_offset = perp * (jitter_angle.sin() * actor.aggro_range * 0.15);
                    let chase_target = player_pos + jitter_offset;

                    let (dest, facing) = steer_toward(my_pos, chase_target, speed, c, dc);
                    actor.facing_yaw = facing;
                    let new_y = snap_actor_y(dest, actor.sprite_half_height, actor.can_fly, hm, c);
                    if is_passable(&actor, transform.translation.y, dest, new_y, wm) {
                        transform.translation.x = dest.x;
                        transform.translation.z = dest.z;
                        transform.translation.y = new_y;
                    }
                }
                return;
            }

            // ── Wander: patrol near guard position ──────────────────────────────
            actor.wander_timer -= dt;

            if actor.wander_timer <= 0.0 {
                let pos_seed = actor.initial_position.x * 7.3 + actor.initial_position.z * 13.7;

                if *anim_state == AnimationState::Idle {
                    // Purely position-based seed incremented by wander_target so each wander
                    // cycle picks a different angle — no time.elapsed_secs() to avoid sync.
                    let target_hash = actor.wander_target.x * 3.1 + actor.wander_target.z * 5.7;
                    let seed = pos_seed * 1.618 + target_hash;
                    let angle = seed.sin() * std::f32::consts::TAU;
                    let dist = actor.tether_distance.max(300.0) * 0.4;
                    actor.wander_target =
                        actor.guarding_position + Vec3::new(angle.cos() * dist, 0.0, angle.sin() * dist);
                    actor.wander_timer = 3.0 + (seed.cos().abs()) * 3.0;
                    *anim_state = AnimationState::Walking;
                } else {
                    actor.wander_timer = 2.0 + (pos_seed * 3.7).sin().abs() * 3.0;
                    *anim_state = AnimationState::Idle;
                }
            }

            if *anim_state == AnimationState::Walking {
                let dir = actor.wander_target - my_pos;
                let flat_dir = Vec3::new(dir.x, 0.0, dir.z);
                if flat_dir.length() > 20.0 {
                    // Wander uses capped speed; steering handles indoor walls.
                    let speed = actor.move_speed.min(60.0) * dt;
                    let (dest, facing) = steer_toward(my_pos, actor.wander_target, speed, c, dc);
                    actor.facing_yaw = facing;
                    let new_y = snap_actor_y(dest, actor.sprite_half_height, actor.can_fly, hm, c);
                    // If wall-stuck (moved <10% of intended), or terrain blocked, abandon target.
                    let wall_stuck = (dest - my_pos).length() < speed * 0.1;
                    if is_passable(&actor, transform.translation.y, dest, new_y, wm) && !wall_stuck {
                        transform.translation.x = dest.x;
                        transform.translation.z = dest.z;
                        transform.translation.y = new_y;
                    } else {
                        // Can't reach wander target — go idle and pick a new one next tick.
                        *anim_state = AnimationState::Idle;
                        actor.wander_timer = 0.0;
                    }
                } else {
                    *anim_state = AnimationState::Idle;
                    actor.wander_timer = 2.0;
                }
            }
        });

    // Write all queued sound events serially
    for event in pending_sounds.lock().unwrap().drain(..) {
        sounds.try_write(event);
    }
}
