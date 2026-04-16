use bevy::prelude::*;

use crate::game::outdoor::OdmName;
use crate::save::GameSave;
use openmm_data::utils::MapName;

/// Centralized live runtime state for the game world.
/// The single source of truth for player state and current map.
/// Save/load copies between this and GameSave.
///
/// In-game time is owned by [`super::time::GameTime`], not here.
#[derive(Resource, Default)]
pub struct WorldState {
    pub player: PlayerRuntimeState,
    pub map: MapRuntimeState,
    pub debug: DebugRuntimeState,
    pub game_vars: GameVariables,
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
    /// Day counters 1-6 (EvtVariable 0xD8..0xDD), general-purpose timers.
    pub days_counters: [i32; 6],
    /// Whether the party is currently flying (EvtVariable 0xDE).
    pub flying: bool,
    /// Number of hired NPCs in party (EvtVariable 0xD6).
    pub npcs_in_party: i32,
    /// Total circus prize accumulated (EvtVariable 0xE0).
    pub total_circus_prize: i32,
    /// NPC topic overrides: npc_id → event_id (set by SetNPCTopic).
    pub npc_topics: std::collections::HashMap<i32, i32>,
    /// Party item counts: item_id → count. Backing store for CheckItemsCount / RemoveItems.
    pub items: std::collections::HashMap<i32, i32>,
    /// NPC greeting overrides: npc_id → greeting_id (set by SetNPCGreeting).
    pub npc_greetings: std::collections::HashMap<i32, i32>,
    /// NPC location overrides: npc_id → map_id (set by MoveNPC).
    pub npc_locations: std::collections::HashMap<i32, i32>,
    /// NPC group news overrides: npc_group → news_id (set by SetNPCGroupNews).
    pub npc_group_news: std::collections::HashMap<i32, i32>,
    /// Chest flag bitmasks: chest_id → flags (set by ToggleChestFlag).
    pub chest_flags: std::collections::HashMap<i32, i32>,
    /// Decoration indices that have been stopped (set by StopAnimation).
    pub stopped_decorations: std::collections::HashSet<i32>,
    /// Actor group overrides: actor_id → group_id (set by SetActorGroup / ChangeGroup).
    pub actor_groups: std::collections::HashMap<i32, i32>,
    /// Actor ally-group overrides: group_id → ally_group_id (set by ChangeGroupAlly).
    pub actor_ally_groups: std::collections::HashMap<i32, i32>,
    /// Decoration event overrides: billboard_index → new_event_id (set by ChangeEvent).
    pub event_overrides: std::collections::HashMap<usize, u16>,
    /// Actor flag overrides: ddm_id → bitflags (set by ToggleActorFlag). Mirrors ActorAttributes bits.
    pub actor_flags: std::collections::HashMap<i32, u32>,
    /// Kill counts by faction group: group_id → killed count (incremented when actor HP → 0).
    pub killed_groups: std::collections::HashMap<i32, u32>,
    /// Dead actor DDM IDs per map: map_name_string → set of ddm_id.
    /// Actors in this set are excluded from spawn on map (re)load.
    pub dead_actor_ids: std::collections::HashMap<String, std::collections::HashSet<i32>>,
}

pub struct PlayerRuntimeState {
    pub position: Vec3,
    pub yaw: f32,
    pub fly_mode: bool,
    pub is_running: bool,
}

impl Default for PlayerRuntimeState {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            yaw: 0.0,
            fly_mode: false,
            is_running: true,
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
            days_counters: [0; 6],
            flying: false,
            npcs_in_party: 0,
            total_circus_prize: 0,
            npc_topics: std::collections::HashMap::new(),
            items: std::collections::HashMap::new(),
            npc_greetings: std::collections::HashMap::new(),
            npc_locations: std::collections::HashMap::new(),
            npc_group_news: std::collections::HashMap::new(),
            chest_flags: std::collections::HashMap::new(),
            stopped_decorations: std::collections::HashSet::new(),
            actor_groups: std::collections::HashMap::new(),
            actor_ally_groups: std::collections::HashMap::new(),
            event_overrides: std::collections::HashMap::new(),
            actor_flags: std::collections::HashMap::new(),
            killed_groups: std::collections::HashMap::new(),
            dead_actor_ids: std::collections::HashMap::new(),
        }
    }
}

impl GameVariables {
    pub fn set_qbit(&mut self, bit: i32) {
        if self.quest_bits.insert(bit) {
            info!("[QBit {:4}] set", bit);
        }
    }

    pub fn clear_qbit(&mut self, bit: i32) {
        if self.quest_bits.remove(&bit) {
            info!("[QBit {:4}] cleared", bit);
        }
    }

    pub fn has_qbit(&self, bit: i32) -> bool {
        self.quest_bits.contains(&bit)
    }

    pub fn add_autonote(&mut self, note: i32) {
        if self.autonotes.insert(note) {
            info!("[Note {:4}] added", note);
        }
    }

    pub fn remove_autonote(&mut self, note: i32) {
        if self.autonotes.remove(&note) {
            info!("[Note {:4}] removed", note);
        }
    }

    pub fn has_autonote(&self, note: i32) -> bool {
        self.autonotes.contains(&note)
    }

    pub fn give_item(&mut self, item_id: i32, count: i32) {
        let entry = self.items.entry(item_id).or_insert(0);
        *entry += count;
        info!("[Item {:4}] count now {}", item_id, *entry);
    }

    pub fn remove_item(&mut self, item_id: i32, count: i32) {
        let entry = self.items.entry(item_id).or_insert(0);
        *entry = (*entry - count).max(0);
        info!("[Item {:4}] count now {}", item_id, *entry);
    }

    pub fn item_count(&self, item_id: i32) -> i32 {
        self.items.get(&item_id).copied().unwrap_or(0)
    }
}

impl WorldState {
    /// Copy live state into GameSave for persistence.
    pub fn write_to_save(&self, save: &mut GameSave) {
        save.player.position = [self.player.position.x, self.player.position.y, self.player.position.z];
        save.player.yaw = self.player.yaw;
        save.map.map_x = self.map.map_x;
        save.map.map_y = self.map.map_y;
        save.progress.quest_bits = self.game_vars.quest_bits.iter().copied().collect();
        save.progress.autonotes = self.game_vars.autonotes.iter().copied().collect();
        save.progress.gold = self.game_vars.gold;
        save.progress.food = self.game_vars.food;
    }

    /// Restore live state from GameSave after loading.
    pub fn read_from_save(&mut self, save: &GameSave) {
        let p = &save.player;
        self.player.position = Vec3::new(p.position[0], p.position[1], p.position[2]);
        self.player.yaw = p.yaw;
        self.map.map_x = save.map.map_x;
        self.map.map_y = save.map.map_y;
        self.game_vars.quest_bits = save.progress.quest_bits.iter().copied().collect();
        self.game_vars.autonotes = save.progress.autonotes.iter().copied().collect();
        self.game_vars.gold = save.progress.gold;
        self.game_vars.food = save.progress.food;
    }
}

pub struct WorldStatePlugin;

impl Plugin for WorldStatePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldState>();
    }
}
