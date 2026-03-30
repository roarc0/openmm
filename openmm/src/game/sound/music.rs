use bevy::{ecs::message::{MessageReader, MessageWriter}, prelude::*};

use crate::game::InGame;

/// Strip ID3v2 tag from MP3 data. Symphonia/rodio can struggle with large or
/// unusual ID3v2 headers, causing panics. The actual audio starts after the tag.
fn strip_id3v2(data: Vec<u8>) -> Vec<u8> {
    if data.len() > 10 && &data[0..3] == b"ID3" {
        // ID3v2 tag size is stored in bytes 6-9 as syncsafe integer (7 bits per byte)
        let size = ((data[6] as usize & 0x7F) << 21)
            | ((data[7] as usize & 0x7F) << 14)
            | ((data[8] as usize & 0x7F) << 7)
            | (data[9] as usize & 0x7F);
        let tag_end = 10 + size;
        if tag_end < data.len() {
            return data[tag_end..].to_vec();
        }
    }
    data
}

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
            .add_systems(Update, handle_play_music);
    }
}

fn handle_play_music(
    mut commands: Commands,
    mut audio_sources: ResMut<Assets<AudioSource>>,
    mut events: MessageReader<PlayMusicEvent>,
    existing_music: Query<Entity, With<MapMusic>>,
) {
    for ev in events.read() {
        // Stop any existing music
        for entity in existing_music.iter() {
            commands.entity(entity).despawn();
        }

        if ev.track == 0 || ev.volume <= 0.0 {
            continue;
        }

        let data_path = lod::get_data_path();
        let music_path =
            std::path::Path::new(&data_path).join(format!("Music/{}.mp3", ev.track));

        if let Ok(bytes) = std::fs::read(&music_path) {
            let source = AudioSource {
                bytes: bytes.into(),
            };
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
            info!("Playing music track {} (vol={:.1})", ev.track, ev.volume);
        } else {
            warn!("Music file not found: {:?}", music_path);
        }
    }
}
