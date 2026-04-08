use bevy::{ecs::message::MessageWriter, prelude::*};
use openmm_data::ActorSoundSlot;

use super::effects::PlayOnceSoundEvent;
use crate::GameState;
use crate::game::actors::Actor;
use crate::game::hud::HudView;
use crate::game::player::Player;

/// Hear actor fidget sounds within this radius (Bevy units).
const ACTOR_SOUND_RANGE: f32 = 1500.0;
/// Max fidget sounds fired per budget window.
const MAX_FIDGETS_PER_WINDOW: u32 = 3;
/// Budget resets every N seconds.
const BUDGET_WINDOW_SECS: f32 = 2.0;
/// Min/max seconds between fidget attempts for a single actor.
const FIDGET_INTERVAL_MIN: f32 = 7.0;
const FIDGET_INTERVAL_MAX: f32 = 18.0;

#[derive(Resource, Default)]
pub struct ActorSoundBudget {
    fidgets_this_window: u32,
    window_timer: f32,
}

pub struct ActorSoundsPlugin;

impl Plugin for ActorSoundsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActorSoundBudget>().add_systems(
            Update,
            actor_fidget_sounds
                .run_if(in_state(GameState::Game))
                .run_if(resource_equals(HudView::World)),
        );
    }
}

fn actor_fidget_sounds(
    time: Res<Time>,
    mut budget: ResMut<ActorSoundBudget>,
    mut actor_query: Query<(&Transform, &mut Actor)>,
    player_query: Query<&Transform, With<Player>>,
    mut sound_events: MessageWriter<PlayOnceSoundEvent>,
) {
    let Ok(player_tf) = player_query.single() else {
        return;
    };
    let player_pos = player_tf.translation;
    let dt = time.delta_secs();

    budget.window_timer -= dt;
    if budget.window_timer <= 0.0 {
        budget.fidgets_this_window = 0;
        budget.window_timer = BUDGET_WINDOW_SECS;
    }

    let range_sq = ACTOR_SOUND_RANGE * ACTOR_SOUND_RANGE;

    for (transform, mut actor) in actor_query.iter_mut() {
        if actor.hp <= 0 {
            continue;
        }
        // Use bypass_change_detection for the timer tick so decrementing the timer each frame
        // does not mark the Actor component as Changed (which would wake up change-detection
        // watchers for every actor every frame). Only the re-arm write below needs detection.
        actor.bypass_change_detection().fidget_timer -= dt;
        if actor.fidget_timer > 0.0 {
            continue;
        }

        let fidget_id = actor.sound_ids[ActorSoundSlot::Fidget as usize] as u32;

        // Stagger re-arm using position so actors don't synchronize over time.
        let stagger = (transform.translation.x * 0.013 + transform.translation.z * 0.019)
            .abs()
            .fract();
        // Write through Mut (intentional change) so fidget_timer is visible to other systems.
        actor.fidget_timer = FIDGET_INTERVAL_MIN + stagger * (FIDGET_INTERVAL_MAX - FIDGET_INTERVAL_MIN);

        if fidget_id == 0 {
            continue;
        }
        if transform.translation.distance_squared(player_pos) > range_sq {
            continue;
        }
        if budget.fidgets_this_window >= MAX_FIDGETS_PER_WINDOW {
            continue;
        }

        budget.fidgets_this_window += 1;
        sound_events.write(PlayOnceSoundEvent {
            sound_id: fidget_id,
            position: transform.translation,
        });
    }
}
