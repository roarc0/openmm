use bevy::{ecs::message::MessageReader, prelude::*};

use super::SoundManager;
use crate::game::InGame;
use crate::game::player::Player;

/// Marker for looping spatial sounds that should be distance-culled.
#[derive(Component)]
struct LoopingSpatialSound;

/// Max distance (world units) for looping spatial sounds. Beyond this,
/// the sound is paused to save CPU. At spatial scale 1/800, a sound at
/// 2000 units is ~2.5x reference distance — practically inaudible.
const SOUND_CULL_DISTANCE: f32 = 2000.0;

/// Message to play a sound effect at a 3D position.
#[derive(Message)]
pub struct PlaySoundEvent {
    pub sound_id: u32,
    pub position: Vec3,
}

/// Message to play a non-positional UI sound.
#[derive(Message)]
pub struct PlayUiSoundEvent {
    pub sound_id: u32,
}

/// Message to play a one-shot spatial sound (plays once then despawns).
#[derive(Message)]
pub struct PlayOnceSoundEvent {
    pub sound_id: u32,
    pub position: Vec3,
}

pub struct EffectsPlugin;

impl Plugin for EffectsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PlaySoundEvent>()
            .add_message::<PlayUiSoundEvent>()
            .add_message::<PlayOnceSoundEvent>()
            .add_systems(
                Update,
                (handle_play_sound, handle_play_ui_sound, handle_play_once_sound),
            )
            .add_systems(
                Update,
                cull_distant_looping_sounds
                    .run_if(in_state(crate::GameState::Game)),
            );
    }
}

/// Pause/resume looping spatial sounds based on distance from the player.
/// Sounds beyond SOUND_CULL_DISTANCE are paused so Bevy's audio system
/// skips spatial attenuation calculations for them. Avoids hundreds of
/// inaudible ambient sounds eating CPU (causes ALSA underruns).
fn cull_distant_looping_sounds(
    player_q: Query<&GlobalTransform, With<Player>>,
    mut sounds: Query<(&GlobalTransform, &mut PlaybackSettings), With<LoopingSpatialSound>>,
) {
    let Ok(player_gt) = player_q.single() else { return };
    let player_pos = player_gt.translation();
    let cull_sq = SOUND_CULL_DISTANCE * SOUND_CULL_DISTANCE;

    for (sound_gt, mut settings) in sounds.iter_mut() {
        let dist_sq = sound_gt.translation().distance_squared(player_pos);
        let should_play = dist_sq < cull_sq;
        let is_paused = settings.paused;
        if should_play && is_paused {
            settings.paused = false;
        } else if !should_play && !is_paused {
            settings.paused = true;
        }
    }
}

fn handle_play_sound(
    mut commands: Commands,
    mut sound_manager: Option<ResMut<SoundManager>>,
    mut audio_sources: ResMut<Assets<AudioSource>>,
    mut events: MessageReader<PlaySoundEvent>,
    cfg: Res<crate::config::GameConfig>,
) {
    let Some(ref mut sound_manager) = sound_manager else {
        for _ in events.read() {}
        return;
    };

    if cfg.sfx_volume <= 0.0 {
        for _ in events.read() {}
        return;
    }

    for ev in events.read() {
        let Some(handle) = sound_manager.load_sound(ev.sound_id, &mut audio_sources) else {
            debug!("Sound id {} not found or failed to load", ev.sound_id);
            continue;
        };

        // Spawn paused — cull_distant_looping_sounds will unpause
        // nearby ones. This avoids hundreds of active spatial audio
        // sources slamming the audio thread during lazy spawn.
        commands.spawn((
            AudioPlayer(handle),
            PlaybackSettings::LOOP
                .with_spatial(true)
                .with_volume(bevy::audio::Volume::Linear(cfg.sfx_volume))
                .paused(),
            Transform::from_translation(ev.position),
            LoopingSpatialSound,
            InGame,
        ));
    }
}

fn handle_play_ui_sound(
    mut commands: Commands,
    mut sound_manager: Option<ResMut<SoundManager>>,
    mut audio_sources: ResMut<Assets<AudioSource>>,
    mut events: MessageReader<PlayUiSoundEvent>,
) {
    let Some(ref mut sound_manager) = sound_manager else {
        for _ in events.read() {}
        return;
    };

    for ev in events.read() {
        let Some(handle) = sound_manager.load_sound(ev.sound_id, &mut audio_sources) else {
            debug!("UI sound id {} not found", ev.sound_id);
            continue;
        };

        commands.spawn((
            AudioPlayer(handle),
            PlaybackSettings {
                mode: bevy::audio::PlaybackMode::Despawn,
                ..default()
            },
            InGame,
        ));
    }
}

fn handle_play_once_sound(
    mut commands: Commands,
    mut sound_manager: Option<ResMut<SoundManager>>,
    mut audio_sources: ResMut<Assets<AudioSource>>,
    mut events: MessageReader<PlayOnceSoundEvent>,
    cfg: Res<crate::config::GameConfig>,
) {
    let Some(ref mut sound_manager) = sound_manager else {
        for _ in events.read() {}
        return;
    };

    if cfg.sfx_volume <= 0.0 {
        for _ in events.read() {}
        return;
    }

    for ev in events.read() {
        let Some(handle) = sound_manager.load_sound(ev.sound_id, &mut audio_sources) else {
            continue;
        };
        commands.spawn((
            AudioPlayer(handle),
            PlaybackSettings {
                mode: bevy::audio::PlaybackMode::Despawn,
                spatial: true,
                volume: bevy::audio::Volume::Linear(cfg.sfx_volume),
                ..default()
            },
            Transform::from_translation(ev.position),
            InGame,
        ));
    }
}
