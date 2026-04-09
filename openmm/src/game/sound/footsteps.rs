use bevy::prelude::*;
use openmm_data::dtile::Tileset;

use super::SoundManager;
use crate::game::InGame;
use crate::game::player::Player;
use crate::states::loading::PreparedWorld;

/// Base terrain suffix used to build footstep sound names ("Walk<Suffix>" / "Run<Suffix>").
fn terrain_suffix(tileset: Tileset) -> &'static str {
    match tileset {
        Tileset::Grass => "Grass",
        Tileset::Snow => "Snow",
        Tileset::Desert => "Desert",
        Tileset::Volcanic => "Badlands",
        Tileset::Dirt => "Dirt",
        Tileset::Water => "Water",
        Tileset::CrackedSwamp | Tileset::Swamp => "Swamp",
        Tileset::Road => "Road",
    }
}

#[derive(Default)]
struct FootstepState {
    last_position: Vec3,
    sound_entity: Option<Entity>,
    current_tileset: Option<Tileset>,
    current_running: bool,
}

pub struct FootstepsPlugin;

impl Plugin for FootstepsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            footstep_system
                // Read the player transform *after* movement/look have written
                // it — otherwise footsteps lag one frame behind input.
                .after(crate::game::player::PlayerInputSet)
                .run_if(resource_exists::<SoundManager>)
                .run_if(resource_exists::<PreparedWorld>),
        );
    }
}

fn footstep_system(
    mut commands: Commands,
    mut sound_manager: ResMut<SoundManager>,
    mut audio_sources: ResMut<Assets<AudioSource>>,
    prepared: Res<PreparedWorld>,
    player_query: Query<&Transform, With<Player>>,
    cfg: Res<crate::config::GameConfig>,
    world_state: Res<crate::game::world::WorldState>,
    mut state: Local<FootstepState>,
) {
    let Ok(player_tf) = player_query.single() else { return };

    let pos = player_tf.translation;

    if state.last_position == Vec3::ZERO && state.sound_entity.is_none() {
        state.last_position = pos;
        return;
    }

    let delta = Vec3::new(pos.x - state.last_position.x, 0.0, pos.z - state.last_position.z);
    let moving = delta.length_squared() > 1.0;
    state.last_position = pos;

    if !moving || cfg.sfx_volume <= 0.0 || world_state.player.fly_mode {
        if let Some(entity) = state.sound_entity.take() {
            commands.entity(entity).despawn();
            state.current_tileset = None;
        }
        return;
    }

    let tileset = prepared.terrain_at(pos.x, pos.z).unwrap_or(Tileset::Dirt);
    let running = world_state.player.is_running;

    if state.current_tileset != Some(tileset) {
        debug!("Terrain: {:?}", tileset);
    }

    // Reuse current loop only if both terrain and walk/run mode are unchanged.
    if state.current_tileset == Some(tileset) && state.current_running == running && state.sound_entity.is_some() {
        return;
    }

    if let Some(entity) = state.sound_entity.take() {
        commands.entity(entity).despawn();
    }

    let prefix = if running { "Run" } else { "Walk" };
    let sound_name = format!("{}{}", prefix, terrain_suffix(tileset));
    let Some(sound_id) = sound_manager.dsounds.get_by_name(&sound_name).map(|s| s.sound_id) else {
        warn!("Footstep sound '{}' not found in dsounds", sound_name);
        return;
    };
    let Some(handle) = sound_manager.load_sound(sound_id, &mut audio_sources) else {
        return;
    };

    let entity = commands
        .spawn((
            AudioPlayer(handle),
            PlaybackSettings::LOOP.with_volume(bevy::audio::Volume::Linear(cfg.sfx_volume)),
            InGame,
        ))
        .id();

    state.sound_entity = Some(entity);
    state.current_tileset = Some(tileset);
    state.current_running = running;
}
