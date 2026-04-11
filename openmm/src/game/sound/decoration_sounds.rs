//! Dawn and dusk ambient sounds for outdoor decorations.
//!
//! Decorations in `DecorationEntry` may have `sound_on_dawn` / `sound_on_dusk` flags
//! and a `sound_id`. When the in-game clock crosses the dawn threshold (~6am) or the
//! dusk threshold (~6pm), this system fires the sound for each matching decoration.

use bevy::prelude::*;

use crate::GameState;
use crate::game::coords::mm6_position_to_bevy;
use crate::game::hud_view::HudView;
use crate::game::world::{GameTime, is_outdoor};
use crate::states::loading::PreparedWorld;

use super::effects::PlaySoundEvent;

/// In-game time fraction for dawn (6am → tod=0.25) and dusk (6pm → tod=0.75).
const DAWN_TOD: f32 = 0.25;
const DUSK_TOD: f32 = 0.75;

/// Tracks whether it was daytime last frame to detect dawn/dusk transitions.
#[derive(Resource)]
struct DawnDuskState {
    was_day: bool,
}

pub struct DecorationSoundsPlugin;

impl Plugin for DecorationSoundsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(DawnDuskState { was_day: false }).add_systems(
            Update,
            dawn_dusk_sound_system
                .run_if(in_state(GameState::Game))
                .run_if(resource_equals(HudView::World))
                .run_if(is_outdoor),
        );
    }
}

fn dawn_dusk_sound_system(
    game_time: Res<GameTime>,
    mut state: ResMut<DawnDuskState>,
    prepared: Res<PreparedWorld>,
    mut sound_events: bevy::ecs::message::MessageWriter<PlaySoundEvent>,
) {
    let tod = game_time.time_of_day();
    let is_day = tod > DAWN_TOD && tod < DUSK_TOD;

    let was_day = state.was_day;
    state.was_day = is_day;

    if is_day == was_day {
        return; // No transition this frame
    }

    // Transitioning: night→day = dawn, day→night = dusk
    let is_dawn = is_day && !was_day;
    let is_dusk = !is_day && was_day;

    for dec in prepared.decorations.iter() {
        let should_play = (is_dawn && dec.sound_on_dawn) || (is_dusk && dec.sound_on_dusk);
        if should_play && dec.sound_id > 0 {
            let pos = mm6_position_to_bevy(dec.position[0], dec.position[1], dec.position[2]);
            sound_events.write(PlaySoundEvent {
                sound_id: dec.sound_id as u32,
                position: Vec3::from(pos),
            });
        }
    }
}
