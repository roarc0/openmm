use bevy::prelude::*;

use crate::GameState;
use crate::game::hud_view::HudView;
use crate::game::world::is_indoor;

pub(crate) mod indoor;

pub use indoor::*;

pub struct BlvPlugin;

impl Plugin for BlvPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Game), indoor::spawn_indoor_world.run_if(is_indoor))
            .add_systems(
                Update,
                (
                    indoor::indoor_interact_system,
                    indoor::indoor_touch_trigger_system,
                    indoor::door_animation_system,
                )
                    .run_if(in_state(GameState::Game))
                    .run_if(resource_equals(HudView::World)),
            );
    }
}
