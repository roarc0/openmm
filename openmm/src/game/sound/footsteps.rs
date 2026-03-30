use bevy::prelude::*;
use lod::dtile::Tileset;

use crate::game::InGame;
use crate::game::player::Player;
use crate::states::loading::PreparedWorld;
use super::SoundManager;

/// Sound ID for walking on a terrain type (from OpenEnroth SoundEnums.h).
fn walk_sound_id(tileset: Tileset) -> u32 {
    match tileset {
        Tileset::Grass => 93,
        Tileset::Dirt => 92,
        Tileset::Desert => 91,
        Tileset::Snow => 97,
        Tileset::Water => 101,
        Tileset::Badlands => 88,
        Tileset::Swamp => 100,
        Tileset::Road => 96,
    }
}

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

    if state.last_position == Vec3::ZERO && state.sound_entity.is_none() {
        state.last_position = pos;
        return;
    }

    let delta = Vec3::new(pos.x - state.last_position.x, 0.0, pos.z - state.last_position.z);
    let moving = delta.length_squared() > 1.0;
    state.last_position = pos;

    if !moving || cfg.sfx_volume <= 0.0 {
        if let Some(entity) = state.sound_entity.take() {
            commands.entity(entity).despawn();
            state.current_tileset = None;
        }
        return;
    }

    let tileset = lod::terrain::tileset_at(&prepared.map, pos.x, pos.z)
        .unwrap_or(Tileset::Dirt);

    if state.current_tileset != Some(tileset) {
        info!("Terrain: {:?}", tileset);
    }

    if state.current_tileset == Some(tileset) && state.sound_entity.is_some() {
        return;
    }

    if let Some(entity) = state.sound_entity.take() {
        commands.entity(entity).despawn();
    }

    let Some(handle) = sound_manager.load_sound(walk_sound_id(tileset), &mut audio_sources) else {
        return;
    };

    let entity = commands
        .spawn((
            AudioPlayer(handle),
            PlaybackSettings::LOOP
                .with_volume(bevy::audio::Volume::Linear(cfg.sfx_volume * 5.0)),
            InGame,
        ))
        .id();

    state.sound_entity = Some(entity);
    state.current_tileset = Some(tileset);
}
