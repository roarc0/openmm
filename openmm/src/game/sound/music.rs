use bevy::{ecs::message::MessageReader, prelude::*};

use crate::game::InGame;

/// Marker for the map music entity, so we can despawn it on map change.
#[derive(Component)]
pub struct MapMusic;

/// Message to request music playback. Sent by odm.rs or blv.rs when a map loads.
#[derive(Message)]
pub struct PlayMusicEvent {
    pub track: u8,
    pub volume: f32,
}

pub struct MusicPlugin;

impl Plugin for MusicPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PlayMusicEvent>().add_systems(
            Update,
            (
                handle_play_music.run_if(resource_exists::<super::SoundManager>),
                sync_music_volume,
            ),
        );
    }
}

fn handle_play_music(
    mut commands: Commands,
    mut audio_sources: ResMut<Assets<AudioSource>>,
    mut events: MessageReader<PlayMusicEvent>,
    existing_music: Query<Entity, With<MapMusic>>,
    game_assets: Res<crate::assets::GameAssets>,
    mut sound_manager: ResMut<super::SoundManager>,
) {
    for ev in events.read() {
        for entity in existing_music.iter() {
            commands.entity(entity).despawn();
        }

        if ev.track == 0 || ev.volume <= 0.0 {
            continue;
        }

        let track = ev.track.to_string();
        let handle = sound_manager.get_music(&track, &game_assets, &mut audio_sources);

        if let Some(handle) = handle {
            commands.spawn((
                AudioPlayer(handle),
                PlaybackSettings {
                    mode: bevy::audio::PlaybackMode::Loop,
                    volume: bevy::audio::Volume::Linear(ev.volume),
                    ..default()
                },
                MapMusic,
                InGame,
            ));
            info!("Playing music track {} (vol={:.1})", ev.track, ev.volume);
        } else {
            warn!("Music track {} not found", ev.track);
        }
    }
}

/// Sync music volume with config changes (from console commands).
fn sync_music_volume(cfg: Res<crate::system::config::GameConfig>, mut music_sinks: Query<&mut AudioSink, With<MapMusic>>) {
    if !cfg.is_changed() {
        return;
    }
    for mut sink in music_sinks.iter_mut() {
        sink.set_volume(bevy::audio::Volume::Linear(cfg.music_volume));
    }
}
