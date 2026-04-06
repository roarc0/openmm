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
        app.add_message::<PlayMusicEvent>()
            .add_systems(Update, (handle_play_music, sync_music_volume));
    }
}

fn handle_play_music(
    mut commands: Commands,
    mut audio_sources: ResMut<Assets<AudioSource>>,
    mut events: MessageReader<PlayMusicEvent>,
    existing_music: Query<Entity, With<MapMusic>>,
) {
    for ev in events.read() {
        for entity in existing_music.iter() {
            commands.entity(entity).despawn();
        }

        if ev.track == 0 || ev.volume <= 0.0 {
            continue;
        }

        let data_path = openmm_data::get_data_path();
        let base_dir = std::path::Path::new(&data_path).parent().unwrap_or(std::path::Path::new(&data_path));
        let track_name = format!("Music/{}.mp3", ev.track);
        
        let music_path = openmm_data::find_path_case_insensitive(base_dir, &track_name);

        if let Some(path) = music_path {
            if let Ok(bytes) = std::fs::read(&path) {
                let source = AudioSource { bytes: bytes.into() };
                let handle = audio_sources.add(source);
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
                info!("Playing music track {} (vol={:.1}) from {:?}", ev.track, ev.volume, path);
            } else {
                warn!("Failed to read music file: {:?}", path);
            }
        } else {
            warn!("Music track not found: {} (searched in {:?})", track_name, base_dir);
        }
    }
}

/// Sync music volume with config changes (from console commands).
fn sync_music_volume(cfg: Res<crate::config::GameConfig>, mut music_sinks: Query<&mut AudioSink, With<MapMusic>>) {
    if !cfg.is_changed() {
        return;
    }
    for mut sink in music_sinks.iter_mut() {
        sink.set_volume(bevy::audio::Volume::Linear(cfg.music_volume));
    }
}
