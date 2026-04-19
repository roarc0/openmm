pub(crate) mod actor_sounds;
pub(crate) mod decoration_sounds;
pub(crate) mod effects;
pub(crate) mod footsteps;
pub(crate) mod music;

use bevy::prelude::*;
use openmm_data::{Archive, dsounds::DSounds, snd::SndArchive, snd::SndExt};
use std::collections::{HashMap, HashSet};

use crate::assets::GameAssets;

/// Central resource for all audio: music and sound effects.
#[derive(Resource)]
pub struct SoundManager {
    pub dsounds: DSounds,
    pub snd_archive: SndArchive,
    cache: HashMap<u32, Handle<AudioSource>>,
    /// Cache for generic tracks like music or video audio, by string name.
    track_cache: HashMap<String, Handle<AudioSource>>,
    /// Sound IDs that failed to load — never retry or re-warn.
    failed: HashSet<u32>,
}

impl SoundManager {
    /// Build a SoundManager from already-loaded game assets.
    /// Returns None if dsounds or Audio.snd are missing.
    pub fn from_game_assets(game_assets: &GameAssets) -> Option<Self> {
        let dsounds = game_assets.dsounds()?.clone();
        let snd_archive = game_assets.get_snd("audio")?.clone();

        Some(Self {
            dsounds,
            snd_archive,
            cache: HashMap::new(),
            track_cache: HashMap::new(),
            failed: HashSet::new(),
        })
    }

    /// Load a sound by dsounds ID into Bevy assets, caching the handle.
    /// Returns None if the sound ID doesn't exist or the WAV can't be extracted.
    /// Failed IDs are cached so we warn exactly once per bad ID.
    pub fn load_sound(
        &mut self,
        sound_id: u32,
        audio_sources: &mut Assets<AudioSource>,
    ) -> Option<Handle<AudioSource>> {
        if let Some(handle) = self.cache.get(&sound_id) {
            return Some(handle.clone());
        }
        if self.failed.contains(&sound_id) {
            return None;
        }

        let info = self.dsounds.get_by_id(sound_id).or_else(|| {
            warn!("Sound ID {} not found in dsounds.bin", sound_id);
            None
        });
        let Some(info) = info else {
            self.failed.insert(sound_id);
            return None;
        };
        let name = info.name().or_else(|| {
            warn!("Sound ID {} has no associated name", sound_id);
            None
        });
        let Some(name) = name else {
            self.failed.insert(sound_id);
            return None;
        };
        if name.is_empty() {
            warn!("Sound ID {} has an empty name entry in dsounds.bin", sound_id);
            self.failed.insert(sound_id);
            return None;
        }
        let Some(wav_bytes) = self.snd_archive.get(&name).or_else(|| {
            warn!("Sound '{}' (id={}) not found in Audio.snd", name, sound_id);
            None
        }) else {
            self.failed.insert(sound_id);
            return None;
        };

        // Validate WAV: RIFF header + PCM format
        if wav_bytes.len() < 44 || &wav_bytes[0..4] != b"RIFF" || &wav_bytes[8..12] != b"WAVE" {
            warn!("Sound '{}' (id={}) is not a valid WAV header", name, sound_id);
            self.failed.insert(sound_id);
            return None;
        }
        if let Some(fmt_pos) = wav_bytes.windows(4).position(|w| w == b"fmt ")
            && wav_bytes.len() > fmt_pos + 9
        {
            let audio_fmt = u16::from_le_bytes([wav_bytes[fmt_pos + 8], wav_bytes[fmt_pos + 9]]);
            if audio_fmt != 1 && audio_fmt != 17 {
                // 1=PCM, 17=IMA ADPCM (handled by SndExt)
                warn!("Sound '{}' (id={}) unsupported format {}", name, sound_id, audio_fmt);
                self.failed.insert(sound_id);
                return None;
            }
        }

        debug!("Loaded sound '{}' (id={}): {} bytes", name, sound_id, wav_bytes.len());

        let source = AudioSource {
            bytes: wav_bytes.into(),
        };
        let handle = audio_sources.add(source);
        self.cache.insert(sound_id, handle.clone());
        Some(handle)
    }

    /// Retrieve or extract audio bytes for an SMK video, caching the Bevy handle.
    pub fn get_video_audio(
        &mut self,
        video: &str,
        game_assets: &GameAssets,
        audio_sources: &mut Assets<AudioSource>,
    ) -> Option<Handle<AudioSource>> {
        let key = format!("video/{}", video);
        if let Some(handle) = self.track_cache.get(&key) {
            return Some(handle.clone());
        }

        if let Some(wav) = game_assets.smk_audio(video)
            && !wav.is_empty()
        {
            let handle = audio_sources.add(AudioSource { bytes: wav.into() });
            self.track_cache.insert(key, handle.clone());
            return Some(handle);
        }
        None
    }

    /// Retrieve or read music bytes for a track, caching the Bevy handle.
    pub fn get_music(
        &mut self,
        track: &str,
        game_assets: &GameAssets,
        audio_sources: &mut Assets<AudioSource>,
    ) -> Option<Handle<AudioSource>> {
        let key = format!("music/{}", track);
        if let Some(handle) = self.track_cache.get(&key) {
            return Some(handle.clone());
        }

        if let Some(bytes) = game_assets.music_bytes(track)
            && !bytes.is_empty()
        {
            let handle = audio_sources.add(AudioSource { bytes: bytes.into() });
            self.track_cache.insert(key, handle.clone());
            return Some(handle);
        }
        None
    }
}

pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        // MM6 world units are large (512 per tile). Scale so ~2 tiles = normal attenuation distance.
        app.add_plugins((
            music::MusicPlugin,
            effects::EffectsPlugin,
            footsteps::FootstepsPlugin,
            actor_sounds::ActorSoundsPlugin,
            decoration_sounds::DecorationSoundsPlugin,
        ))
        .insert_resource(bevy::audio::DefaultSpatialScale(bevy::audio::SpatialScale::new(
            1.0 / 800.0,
        )))
        .add_systems(Startup, init_sound_manager);
    }
}

fn init_sound_manager(mut commands: Commands, game_assets: Res<GameAssets>) {
    let Some(dsounds) = game_assets.dsounds().cloned() else {
        warn!("dsounds.bin not loaded — sound effects disabled");
        return;
    };

    let Some(snd_archive) = game_assets.get_snd("audio").cloned() else {
        warn!("Audio.snd not found in loaded archives — sound effects disabled");
        return;
    };

    info!(
        "Sound system initialized: {} descriptors, {} audio files",
        dsounds.items.len(),
        snd_archive.list_files().len()
    );
    commands.insert_resource(SoundManager {
        dsounds,
        snd_archive,
        cache: HashMap::new(),
        track_cache: HashMap::new(),
        failed: HashSet::new(),
    });
}
