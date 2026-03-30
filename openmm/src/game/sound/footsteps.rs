use bevy::prelude::*;
use lod::odm::{ODM_SIZE, ODM_TILE_SCALE};

use crate::game::InGame;
use crate::game::player::Player;
use crate::states::loading::PreparedWorld;
use super::SoundManager;

/// Tileset enum values matching MM6/OpenEnroth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i16)]
pub enum Tileset {
    Grass = 1,
    Snow = 2,
    Desert = 3,
    Dirt = 4,
    Water = 5,
    Badlands = 6,
    Swamp = 7,
    Road = 8,
}

impl Tileset {
    pub fn from_tile_set(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Grass),
            2 => Some(Self::Snow),
            3 => Some(Self::Desert),
            4 => Some(Self::Dirt),
            5 => Some(Self::Water),
            6 => Some(Self::Badlands),
            7 => Some(Self::Swamp),
            8 => Some(Self::Road),
            _ => None,
        }
    }

    /// Sound ID for walking on this terrain (from OpenEnroth SoundEnums.h).
    pub fn walk_sound_id(self) -> u32 {
        match self {
            Self::Grass => 93,
            Self::Dirt => 92,
            Self::Desert => 91,
            Self::Snow => 97,
            Self::Water => 101,
            Self::Badlands => 88,
            Self::Swamp => 100,
            Self::Road => 96,
        }
    }
}

/// Look up the tileset at a Bevy world position using the ODM tile map.
pub fn tileset_at_position(
    tile_map: &[u8],
    tileset_lookup: &[i16],
    x: f32,
    z: f32,
) -> Option<Tileset> {
    // Bevy: X right, Z = -Y_mm6. Tile grid: col = x/512, row = y_mm6/512 = -z/512
    let col = (x / ODM_TILE_SCALE) as i32;
    let row = (-z / ODM_TILE_SCALE) as i32;

    if col < 0 || row < 0 || col >= ODM_SIZE as i32 || row >= ODM_SIZE as i32 {
        return None;
    }

    let tile_index = row as usize * ODM_SIZE + col as usize;
    let tile_id = *tile_map.get(tile_index)? as usize;
    let tile_set = *tileset_lookup.get(tile_id)?;
    Tileset::from_tile_set(tile_set)
}

/// Tracks the current footstep playback state.
#[derive(Default)]
struct FootstepState {
    last_position: Vec3,
    sound_entity: Option<Entity>,
    current_tileset: Option<Tileset>,
}

pub struct FootstepsPlugin;

impl Plugin for FootstepsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, footstep_system.run_if(resource_exists::<SoundManager>));
    }
}

fn footstep_system(
    mut commands: Commands,
    mut sound_manager: Option<ResMut<SoundManager>>,
    mut audio_sources: ResMut<Assets<AudioSource>>,
    prepared: Option<Res<PreparedWorld>>,
    player_query: Query<&Transform, With<Player>>,
    cfg: Res<crate::config::GameConfig>,
    mut state: Local<FootstepState>,
) {
    let Some(ref mut sound_manager) = sound_manager else { return };
    let Some(prepared) = prepared else { return };
    let Ok(player_tf) = player_query.single() else { return };

    let pos = player_tf.translation;

    // Initialize last_position on first frame
    if state.last_position == Vec3::ZERO && state.sound_entity.is_none() {
        state.last_position = pos;
        return;
    }

    // Check if player is moving (horizontal displacement)
    let delta = Vec3::new(pos.x - state.last_position.x, 0.0, pos.z - state.last_position.z);
    let moving = delta.length_squared() > 1.0;
    state.last_position = pos;

    if !moving || cfg.sfx_volume <= 0.0 {
        // Stop footstep sound
        if let Some(entity) = state.sound_entity.take() {
            commands.entity(entity).despawn();
            state.current_tileset = None;
        }
        return;
    }

    // Determine terrain tileset at player position
    let tileset = prepared.terrain_at(pos.x, pos.z).unwrap_or(Tileset::Grass);

    // If same tileset, keep current sound playing
    if state.current_tileset == Some(tileset) && state.sound_entity.is_some() {
        return;
    }

    // Stop old sound
    if let Some(entity) = state.sound_entity.take() {
        commands.entity(entity).despawn();
    }

    // Play new walking sound
    let sound_id = tileset.walk_sound_id();
    let Some(handle) = sound_manager.load_sound(sound_id, &mut audio_sources) else {
        return;
    };

    let entity = commands
        .spawn((
            AudioPlayer(handle),
            PlaybackSettings::LOOP
                .with_volume(bevy::audio::Volume::Linear(cfg.sfx_volume * 0.5)),
            InGame,
        ))
        .id();

    state.sound_entity = Some(entity);
    state.current_tileset = Some(tileset);
}
