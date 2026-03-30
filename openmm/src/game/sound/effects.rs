use bevy::{ecs::message::MessageReader, prelude::*};

use super::SoundManager;
use crate::game::InGame;

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

pub struct EffectsPlugin;

impl Plugin for EffectsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PlaySoundEvent>()
            .add_message::<PlayUiSoundEvent>()
            .add_systems(Update, (handle_play_sound, handle_play_ui_sound));
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

        commands.spawn((
            AudioPlayer(handle),
            PlaybackSettings::LOOP
                .with_spatial(true)
                .with_volume(bevy::audio::Volume::Linear(cfg.sfx_volume)),
            Transform::from_translation(ev.position),
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
