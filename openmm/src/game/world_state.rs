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
    pub game_vars: GameVariables,
    /// Time of day: 0.0 = midnight, 0.25 = sunrise, 0.5 = noon, 0.75 = sunset
    pub time_of_day: f32,
}

/// Game variables storage — quest flags, map locals, gold, food, etc.
pub struct GameVariables {
    /// Map-local variables (MapVar0..MapVar99), reset on map change.
    pub map_vars: [i32; 100],
    /// Global quest bits (set/cleared by event scripts).
    pub quest_bits: std::collections::HashSet<i32>,
    /// Party gold.
    pub gold: i32,
    /// Party food rations.
    pub food: i32,
    /// Party reputation (signed — negative is good, positive is bad in MM6).
    pub reputation: i32,
    /// Auto-notes (journal entries).
    pub autonotes: std::collections::HashSet<i32>,
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

impl Default for GameVariables {
    fn default() -> Self {
        Self {
            map_vars: [0; 100],
            quest_bits: std::collections::HashSet::new(),
            gold: 200, // Starting gold in MM6
            food: 7,
            reputation: 0,
            autonotes: std::collections::HashSet::new(),
        }
    }
}

impl Default for WorldState {
    fn default() -> Self {
        Self {
            player: PlayerRuntimeState::default(),
            map: MapRuntimeState::default(),
            debug: DebugRuntimeState::default(),
            game_vars: GameVariables::default(),
            time_of_day: 0.375, // 9am
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
