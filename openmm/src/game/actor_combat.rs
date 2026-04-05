//! Monster melee attack system.
//! All attack logic (timing, range, animation, sound) lives here — not dispersed.

use bevy::{ecs::message::MessageWriter, prelude::*};

use crate::GameState;
use crate::game::entities::AnimationState;
use crate::game::entities::actor::Actor;
use crate::game::hud::HudView;
use crate::game::player::Player;
use crate::game::sound::effects::PlayOnceSoundEvent;

/// Per-attack animation duration in seconds (approx 5 frames at 0.15s).
const ATTACK_ANIM_SECS: f32 = 0.75;
/// Min/max seconds between attack attempts per actor (position-staggered).
const ATTACK_COOLDOWN_MIN: f32 = 2.0;
const ATTACK_COOLDOWN_MAX: f32 = 4.0;
/// Max attacks fired globally per budget window.
const MAX_ATTACKS_PER_WINDOW: u32 = 2;
const ATTACK_BUDGET_WINDOW: f32 = 2.0;
/// Seconds the dying animation plays before transitioning to Dead state.
const DYING_ANIM_SECS: f32 = 1.5;

/// Sent when the player clicks on a monster. Triggers death sequence.
#[derive(Message)]
pub struct KillActorEvent(pub Entity);

/// Marker added to a monster that is currently playing its dying animation.
#[derive(Component)]
pub struct DyingTimer(pub f32);

#[derive(Resource, Default)]
struct AttackBudget {
    count: u32,
    timer: f32,
}

pub struct ActorCombatPlugin;

impl Plugin for ActorCombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<KillActorEvent>()
            .init_resource::<AttackBudget>()
            .add_systems(
            Update,
            (
                monster_attack_system,
                monster_die_system,
                dying_to_dead_system,
            )
                .run_if(in_state(GameState::Game))
                .run_if(resource_equals(HudView::World)),
        );
    }
}

fn monster_attack_system(
    time: Res<Time>,
    mut budget: ResMut<AttackBudget>,
    mut actors: Query<(&Transform, &mut Actor, &mut AnimationState), Without<DyingTimer>>,
    player: Query<&Transform, With<Player>>,
    mut sounds: MessageWriter<PlayOnceSoundEvent>,
) {
    let Ok(player_tf) = player.single() else {
        return;
    };
    let player_pos = player_tf.translation;
    let dt = time.delta_secs();

    budget.timer -= dt;
    if budget.timer <= 0.0 {
        budget.count = 0;
        budget.timer = ATTACK_BUDGET_WINDOW;
    }

    for (transform, mut actor, mut anim_state) in actors.iter_mut() {
        if !actor.hostile || actor.attack_range <= 0.0 {
            continue;
        }

        // Tick attack animation; revert to Idle when done.
        if actor.attack_anim_remaining > 0.0 {
            actor.attack_anim_remaining -= dt;
            if actor.attack_anim_remaining <= 0.0 {
                *anim_state = AnimationState::Idle;
            }
        }

        // Tick cooldown between attacks.
        actor.attack_timer -= dt;
        if actor.attack_timer > 0.0 {
            continue;
        }

        // Re-arm with position-staggered interval.
        let stagger = (transform.translation.x * 0.017 + transform.translation.z * 0.011)
            .abs()
            .fract();
        actor.attack_timer = ATTACK_COOLDOWN_MIN + stagger * (ATTACK_COOLDOWN_MAX - ATTACK_COOLDOWN_MIN);

        if transform.translation.distance_squared(player_pos) > actor.attack_range * actor.attack_range {
            continue;
        }

        if budget.count >= MAX_ATTACKS_PER_WINDOW {
            continue;
        }

        // Trigger: animate + sound.
        budget.count += 1;
        *anim_state = AnimationState::Attacking;
        actor.attack_anim_remaining = ATTACK_ANIM_SECS;

        if actor.sound_ids[0] > 0 {
            sounds.write(PlayOnceSoundEvent {
                sound_id: actor.sound_ids[0] as u32,
                position: transform.translation,
            });
        }
    }
}

/// Handle click-to-kill: set dying animation, play die sound, add DyingTimer.
fn monster_die_system(
    mut kill_events: bevy::ecs::message::MessageReader<KillActorEvent>,
    mut actors: Query<(&Actor, &Transform, &mut AnimationState), Without<DyingTimer>>,
    mut commands: Commands,
    mut sounds: MessageWriter<PlayOnceSoundEvent>,
    mut world_state: Option<ResMut<crate::game::world_state::WorldState>>,
) {
    for KillActorEvent(entity) in kill_events.read() {
        let Ok((actor, transform, mut anim_state)) = actors.get_mut(*entity) else {
            continue;
        };
        info!("Actor '{}' (ddm_id={}) killed by player click", actor.name, actor.ddm_id);
        *anim_state = AnimationState::Dying;
        if actor.sound_ids[1] > 0 {
            sounds.write(PlayOnceSoundEvent {
                sound_id: actor.sound_ids[1] as u32,
                position: transform.translation,
            });
        }
        // Persist death: DDM-placed actors (ddm_id >= 0) are recorded so they don't
        // respawn when the map is reloaded. ODM spawn groups (ddm_id == -1) are skipped.
        if actor.ddm_id >= 0 {
            if let Some(ref mut ws) = world_state {
                let map_key = ws.map.name.to_string();
                ws.game_vars
                    .dead_actor_ids
                    .entry(map_key)
                    .or_default()
                    .insert(actor.ddm_id);
            }
        }
        commands.entity(*entity).insert(DyingTimer(DYING_ANIM_SECS));
    }
}

/// When DyingTimer expires, transition to Dead state and remove the timer.
/// The corpse stays in the world until looted.
fn dying_to_dead_system(
    time: Res<Time>,
    mut query: Query<(Entity, &mut DyingTimer, &mut AnimationState)>,
    mut commands: Commands,
) {
    for (entity, mut timer, mut anim_state) in query.iter_mut() {
        timer.0 -= time.delta_secs();
        if timer.0 <= 0.0 {
            *anim_state = AnimationState::Dead;
            commands.entity(entity).remove::<DyingTimer>();
        }
    }
}
