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
//!     achieved after `resolve_movement`), first retry the offset that worked
//!     last frame (cached on the `Actor`) — hugging the same wall rarely needs
//!     a re-probe.
//!  3. If the cached offset also fails (or there is none), probe ±45 / ±90°
//!     and pick the heading with the best forward progress, then cache it.
//!
//! `resolve_movement` is the hot inner call; worst case 5 rays per blocked
//! actor per frame (was 9), steady state 1–2 rays while rounding an obstacle.
//! Works for both outdoor (no colliders → direct path) and indoor BSP geometry.

use bevy::{ecs::message::MessageWriter, prelude::*};

use crate::GameState;
use crate::game::actors::combat::{ActorDead, DyingTimer};
use crate::game::actors::physics::{is_passable, snap_actor_y};
use crate::game::actors::{Actor, MonsterAiType};
use crate::game::map::collision::{BuildingColliders, TerrainHeightMap, WaterMap};
use crate::game::map::indoor::DoorColliders;
use crate::game::optional::OptionalWrite;
use crate::game::player::Player;
use crate::game::sound::effects::PlayOnceSoundEvent;
use crate::game::sprites::{AnimationState, WorldEntity};
use crate::game::state::tick::{GameTickConfig, GameTickSet, position_phase, should_tick_actor};
use crate::system::config::GameConfig;
use openmm_data::ActorSoundSlot;

/// Heading offsets (degrees) probed when the direct path is blocked and the
/// cached detour (if any) also failed. Narrow angles first so a tight detour
/// wins ties over a big one.
const PROBE_OFFSETS_DEG: &[f32] = &[45.0, -45.0, 90.0, -90.0];

/// If the direct path delivers < this fraction of intended movement, try probes.
const BLOCK_THRESHOLD: f32 = 0.7;

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
            FixedUpdate,
            monster_ai_system
                .after(GameTickSet)
                .run_if(in_state(GameState::Game))
                .run_if(crate::game::ui::is_world_mode),
        );
    }
}

/// Try to move `from` toward `target_pos` by `speed` units, steering around walls.
///
/// Returns `(destination, facing_yaw)`. `cached_offset` is read and updated in
/// place so the same detour heading can be retried next frame without
/// re-probing the whole fan.
/// When `colliders` is `None` (outdoor, no buildings) the direct path is always used.
fn steer_toward(
    from: Vec3,
    target_pos: Vec3,
    speed: f32,
    collision_radius: f32,
    colliders: Option<&BuildingColliders>,
    door_colliders: Option<&DoorColliders>,
    cached_offset: &mut Option<f32>,
) -> (Vec3, f32) {
    let flat = Vec3::new(target_pos.x - from.x, 0.0, target_pos.z - from.z);
    if flat.length_squared() < 1.0 {
        *cached_offset = None;
        return (from, 0.0);
    }
    let base_yaw = flat.x.atan2(flat.z); // yaw pointing toward target

    let probe_dest = |yaw: f32| -> Vec3 {
        let dir = Vec3::new(yaw.sin(), 0.0, yaw.cos());
        let intended = from + dir * speed;
        let mut pos = intended;
        if let Some(c) = colliders {
            pos = c.resolve_movement(from, pos, collision_radius, ACTOR_EYE_HEIGHT);
        }
        if let Some(dc) = door_colliders {
            // Apply dynamic door wall collision
            pos = dc.resolve_movement(from, pos, collision_radius, ACTOR_EYE_HEIGHT);
            // Block movement into closed horizontal door panels (trapdoors, slabs)
            let feet_y = pos.y - ACTOR_EYE_HEIGHT;
            if dc.blocks_entry(pos.x, pos.z, feet_y, pos.y, collision_radius) {
                pos = from;
            }
        }
        pos
    };

    // 1) Direct heading first.
    let direct = probe_dest(base_yaw);
    let direct_progress = (direct - from).length();
    if direct_progress >= speed * BLOCK_THRESHOLD {
        *cached_offset = None;
        return (direct, base_yaw);
    }

    // 2) Blocked — try the cached detour offset before fanning out. Monsters
    // hugging the same wall converge on a stable offset, so this is usually a hit.
    if let Some(off) = *cached_offset {
        let yaw = base_yaw + off;
        let dest = probe_dest(yaw);
        let progress = (dest - from).length();
        if progress >= speed * BLOCK_THRESHOLD {
            return (dest, yaw);
        }
    }

    // 3) Still blocked — probe the fan. Cache whichever offset wins so next
    // frame's step 2 can short-circuit.
    let mut best_dest = direct;
    let mut best_progress = direct_progress;
    let mut best_yaw = base_yaw;
    let mut best_offset: Option<f32> = None;

    for &deg in PROBE_OFFSETS_DEG {
        let off = deg.to_radians();
        let yaw = base_yaw + off;
        let dest = probe_dest(yaw);
        let progress = (dest - from).length();
        if progress > best_progress {
            best_progress = progress;
            best_dest = dest;
            best_yaw = yaw;
            best_offset = Some(off);
        }
    }

    *cached_offset = best_offset;
    (best_dest, best_yaw)
}

fn monster_ai_system(
    time: Res<Time>,
    tick_cfg: Res<GameTickConfig>,
    cfg: Res<GameConfig>,
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
    #[cfg(feature = "perf_log")] mut perf: ResMut<crate::screens::debug::perf_log::PerfCounters>,
) {
    #[cfg(feature = "perf_log")]
    let _start = crate::screens::debug::perf_log::perf_start();

    let Ok(player_tf) = player.single() else {
        return;
    };
    let player_pos = player_tf.translation;
    let dt = time.delta_secs();
    let c = colliders.as_deref();
    let dc = door_colliders.as_deref();
    let hm = terrain.as_deref();
    let wm = water_map.as_deref();

    let tick = tick_cfg.tick;
    let close_sq = tick_cfg.close_range_sq;
    let medium_sq = tick_cfg.medium_range_sq;
    let draw_dist_sq = cfg.draw_distance * cfg.draw_distance;

    #[cfg(feature = "perf_log")]
    let mut ai_count: u32 = 0;
    #[cfg(feature = "perf_log")]
    let mut steer_count: u32 = 0;

    for (mut transform, mut actor, mut anim_state, mut ai_mode) in query.iter_mut() {
        if actor.hp <= 0 {
            continue;
        }

        // Distance-tiered ticking: close actors every tick, medium every 2nd, far every 4th.
        let my_pos = transform.translation;
        let dist_sq = my_pos.distance_squared(player_pos);
        let phase = position_phase(actor.initial_position.x, actor.initial_position.z);
        if !should_tick_actor(tick, phase, dist_sq, close_sq, medium_sq, draw_dist_sq) {
            continue;
        }

        if actor.move_speed < 1.0 {
            continue;
        }

        // Let attack animation play out without interference.
        if matches!(*anim_state, AnimationState::Attacking) {
            continue;
        }

        #[cfg(feature = "perf_log")]
        {
            ai_count += 1;
        }

        // Fidget sound: play slot [Fidget] periodically while wandering.
        let fidget_sound = actor.sound_ids[ActorSoundSlot::Fidget as usize];
        if *ai_mode == MonsterAiMode::Wander && fidget_sound > 0 {
            actor.fidget_timer -= dt;
            if actor.fidget_timer <= 0.0 {
                let seed = (transform.translation.x * 0.031 + transform.translation.z * 0.017)
                    .abs()
                    .fract();
                actor.fidget_timer = 10.0 + seed * 15.0;
                sounds.try_write(PlayOnceSoundEvent {
                    sound_id: fidget_sound as u32,
                    position: transform.translation,
                });
            }
        }

        // ── Aggro state transitions ──────────────────────────────────────────
        if actor.hostile && actor.aggro_range > 0.0 {
            let effective_aggro = match actor.ai_type {
                MonsterAiType::Aggress => actor.aggro_range,
                MonsterAiType::Wimp => actor.aggro_range * 0.25,
                MonsterAiType::Suicidal => actor.aggro_range * 1.0,
                _ => actor.aggro_range,
            };
            let aggro_sq = effective_aggro * effective_aggro;
            let leash_sq = (effective_aggro * 2.0) * (effective_aggro * 2.0);

            match *ai_mode {
                MonsterAiMode::Wander if dist_sq < aggro_sq => {
                    *ai_mode = MonsterAiMode::Aggro;
                    *anim_state = AnimationState::Walking;
                    debug!("Monster '{}' aggros player", actor.name);
                    let alert_sound = actor.sound_ids[ActorSoundSlot::GotHit as usize];
                    if alert_sound > 0 {
                        sounds.try_write(PlayOnceSoundEvent {
                            sound_id: alert_sound as u32,
                            position: my_pos,
                        });
                    }
                }
                MonsterAiMode::Aggro if dist_sq > leash_sq => {
                    *ai_mode = MonsterAiMode::Wander;
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
                if *anim_state == AnimationState::Walking {
                    *anim_state = AnimationState::Idle;
                }
            } else {
                *anim_state = AnimationState::Walking;
                let speed = actor.move_speed * dt;

                let jitter_seed = actor.initial_position.x * 3.7 + actor.initial_position.z * 6.1;
                let jitter_phase = jitter_seed + time.elapsed_secs() * 0.3;
                let jitter_angle = jitter_phase.sin() * 0.4;
                let flat_to_player =
                    Vec3::new(player_pos.x - my_pos.x, 0.0, player_pos.z - my_pos.z).normalize_or_zero();
                let perp = Vec3::new(-flat_to_player.z, 0.0, flat_to_player.x);
                let jitter_offset = perp * (jitter_angle.sin() * actor.aggro_range * 0.15);
                let chase_target = player_pos + jitter_offset;

                let mut cache = actor.cached_steer_offset;
                #[cfg(feature = "perf_log")]
                {
                    steer_count += 1;
                }
                let (dest, facing) =
                    steer_toward(my_pos, chase_target, speed, actor.collision_radius, c, dc, &mut cache);
                actor.cached_steer_offset = cache;
                actor.facing_yaw = facing;
                let new_y = snap_actor_y(dest, actor.sprite_half_height, actor.can_fly, hm, c);
                if is_passable(&actor, transform.translation.y, dest, new_y, wm) {
                    transform.translation.x = dest.x;
                    transform.translation.z = dest.z;
                    transform.translation.y = new_y;
                }
            }
            continue;
        }

        // ── Wander: patrol near guard position ──────────────────────────────
        actor.wander_timer -= dt;

        if actor.wander_timer <= 0.0 {
            let pos_seed = actor.initial_position.x * 7.3 + actor.initial_position.z * 13.7;

            if *anim_state == AnimationState::Idle {
                let target_hash = actor.wander_target.x * 3.1 + actor.wander_target.z * 5.7;
                let seed = pos_seed * 1.618 + target_hash;
                let angle = seed.sin() * std::f32::consts::TAU;
                let dist = actor.tether_distance.max(300.0) * 0.4;
                actor.wander_target = actor.guarding_position + Vec3::new(angle.cos() * dist, 0.0, angle.sin() * dist);
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
                let speed = actor.move_speed.min(60.0) * dt;
                let mut cache = actor.cached_steer_offset;
                #[cfg(feature = "perf_log")]
                {
                    steer_count += 1;
                }
                let (dest, facing) = steer_toward(
                    my_pos,
                    actor.wander_target,
                    speed,
                    actor.collision_radius,
                    c,
                    dc,
                    &mut cache,
                );
                actor.cached_steer_offset = cache;
                actor.facing_yaw = facing;
                let new_y = snap_actor_y(dest, actor.sprite_half_height, actor.can_fly, hm, c);
                let wall_stuck = (dest - my_pos).length() < speed * 0.1;
                if is_passable(&actor, transform.translation.y, dest, new_y, wm) && !wall_stuck {
                    transform.translation.x = dest.x;
                    transform.translation.z = dest.z;
                    transform.translation.y = new_y;
                } else {
                    *anim_state = AnimationState::Idle;
                    actor.wander_timer = 0.0;
                }
            } else {
                *anim_state = AnimationState::Idle;
                actor.wander_timer = 2.0;
            }
        }
    }

    #[cfg(feature = "perf_log")]
    {
        perf.ai_iter += ai_count;
        perf.ai_steer_calls += steer_count;
        perf.time_ai_us += crate::screens::debug::perf_log::perf_elapsed_us(_start);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression for A2: with no colliders the direct heading always succeeds,
    /// so `steer_toward` must clear any stale cached detour offset instead of
    /// letting a stale sidestep angle persist and mislead a later blocked frame.
    #[test]
    fn steer_toward_clears_cache_on_direct_success() {
        let from = Vec3::new(0.0, 0.0, 0.0);
        let target = Vec3::new(100.0, 0.0, 0.0);
        let mut cache = Some(1.2); // stale offset from a previous frame
        let (dest, yaw) = steer_toward(from, target, 10.0, 20.0, None, None, &mut cache);
        assert!(cache.is_none(), "direct path should clear cached detour");
        // Direct heading toward +X is yaw = π/2 (atan2(1, 0)).
        assert!((yaw - std::f32::consts::FRAC_PI_2).abs() < 1e-4);
        assert!((dest - Vec3::new(10.0, 0.0, 0.0)).length() < 1e-3);
    }

    /// Zero-distance target should be a no-op and also clear the cache.
    #[test]
    fn steer_toward_zero_target_clears_cache() {
        let from = Vec3::new(5.0, 0.0, 5.0);
        let mut cache = Some(0.7);
        let (dest, _) = steer_toward(from, from, 10.0, 20.0, None, None, &mut cache);
        assert!(cache.is_none());
        assert_eq!(dest, from);
    }
}
