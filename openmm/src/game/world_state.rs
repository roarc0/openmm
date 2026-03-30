use bevy::prelude::*;

use crate::GameState;
use crate::game::map_name::MapName;
use crate::game::odm::OdmName;
use crate::game::player::Player;
use crate::save::GameSave;

/// Centralized live runtime state for the game world.
/// The single source of truth for player state and current map.
/// Save/load copies between this and GameSave.
#[derive(Resource)]
pub struct WorldState {
    pub player: PlayerRuntimeState,
    pub map: MapRuntimeState,
    pub debug: DebugRuntimeState,
}

pub struct PlayerRuntimeState {
    pub position: Vec3,
    pub yaw: f32,
    pub fly_mode: bool,
}

impl Default for PlayerRuntimeState {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            yaw: 0.0,
            fly_mode: false,
        }
    }
}

#[derive(Clone)]
pub struct MapRuntimeState {
    pub name: MapName,
    pub map_x: char,
    pub map_y: char,
}

pub struct DebugRuntimeState {
    pub show_play_area: bool,
    pub show_events: bool,
}

impl Default for DebugRuntimeState {
    fn default() -> Self {
        Self {
            show_play_area: true,
            show_events: true,
        }
    }
}

impl Default for MapRuntimeState {
    fn default() -> Self {
        Self {
            name: MapName::Outdoor(OdmName::default()),
            map_x: 'e',
            map_y: '3',
        }
    }
}

impl Default for WorldState {
    fn default() -> Self {
        Self {
            player: PlayerRuntimeState::default(),
            map: MapRuntimeState::default(),
            debug: DebugRuntimeState::default(),
        }
    }
}

impl WorldState {
    /// Copy live state into GameSave for persistence.
    pub fn write_to_save(&self, save: &mut GameSave) {
        save.player.position = [
            self.player.position.x,
            self.player.position.y,
            self.player.position.z,
        ];
        save.player.yaw = self.player.yaw;
        save.map.map_x = self.map.map_x;
        save.map.map_y = self.map.map_y;
    }

    /// Restore live state from GameSave after loading.
    pub fn read_from_save(&mut self, save: &GameSave) {
        let p = &save.player;
        self.player.position = Vec3::new(p.position[0], p.position[1], p.position[2]);
        self.player.yaw = p.yaw;
        self.map.map_x = save.map.map_x;
        self.map.map_y = save.map.map_y;
    }
}

pub struct WorldStatePlugin;

impl Plugin for WorldStatePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldState>()
            .add_systems(
                PostUpdate,
                sync_player_to_world_state.run_if(in_state(GameState::Game)),
            );
    }
}

/// Copy Player entity transform → WorldState every frame (PostUpdate).
fn sync_player_to_world_state(
    mut world_state: ResMut<WorldState>,
    player_query: Query<&Transform, With<Player>>,
) {
    if let Ok(transform) = player_query.single() {
        world_state.player.position = transform.translation;
        let (yaw, _, _) = transform.rotation.to_euler(EulerRot::YXZ);
        world_state.player.yaw = yaw;
    }
}
