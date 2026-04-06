pub(crate) mod actor_sounds;
pub(crate) mod decoration_sounds;
pub(crate) mod effects;
pub(crate) mod footsteps;
pub(crate) mod music;

use bevy::prelude::*;
use openmm_data::{dsounds::DSounds, snd::{SndArchive, SndExt}, Archive};
use std::collections::HashMap;

use crate::assets::GameAssets;

/// Central resource for all audio: music and sound effects.
#[derive(Resource)]
pub struct SoundManager {
    pub dsounds: DSounds,
    pub snd_archive: SndArchive,
    cache: HashMap<u32, Handle<AudioSource>>,
}

impl SoundManager {
    /// Load a sound by dsounds ID into Bevy assets, caching the handle.
    /// Returns None if the sound ID doesn't exist or the WAV can't be extracted.
    pub fn load_sound(
        &mut self,
        sound_id: u32,
        audio_sources: &mut Assets<AudioSource>,
    ) -> Option<Handle<AudioSource>> {
        if let Some(handle) = self.cache.get(&sound_id) {
            return Some(handle.clone());
        }

        let info = self.dsounds.get_by_id(sound_id).or_else(|| {
            warn!("Sound ID {} not found in dsounds.bin", sound_id);
            None
        })?;
        let name = info.name().or_else(|| {
            warn!("Sound ID {} has no associated name", sound_id);
            None
        })?;
        if name.is_empty() {
            warn!("Sound ID {} has an empty name entry in dsounds.bin", sound_id);
            return None;
        }
        let wav_bytes = self.snd_archive.get(&name).or_else(|| {
            warn!("Sound '{}' (id={}) not found in Audio.snd", name, sound_id);
            None
        })?;

        // Validate WAV: RIFF header + PCM format
        if wav_bytes.len() < 44 || &wav_bytes[0..4] != b"RIFF" || &wav_bytes[8..12] != b"WAVE" {
            warn!("Sound '{}' (id={}) is not a valid WAV header", name, sound_id);
            return None;
        }
        if let Some(fmt_pos) = wav_bytes.windows(4).position(|w| w == b"fmt ") {
            if wav_bytes.len() > fmt_pos + 9 {
                let audio_fmt = u16::from_le_bytes([wav_bytes[fmt_pos + 8], wav_bytes[fmt_pos + 9]]);
                if audio_fmt != 1 && audio_fmt != 17 { // 1=PCM, 17=IMA ADPCM (handled by SndExt)
                    warn!("Sound '{}' (id={}) unsupported format {}", name, sound_id, audio_fmt);
                    return None;
                }
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
    let dsounds = match DSounds::load(game_assets.lod_manager()) {
        Ok(d) => d,
        Err(e) => {
            warn!("Failed to load dsounds.bin: {e}");
            return;
        }
    };

    let data_path = openmm_data::get_data_path();
    let base = std::path::Path::new(&data_path);
    let parent = base.parent().unwrap_or(base);
    
    // Use robust path resolution for Audio.snd
    let snd_path = openmm_data::find_path_case_insensitive(parent, "Sounds/Audio.snd")
        .or_else(|| openmm_data::find_path_case_insensitive(base, "Audio.snd"));

    let Some(snd_path) = snd_path else {
        warn!("Audio.snd not found in {:?}/Sounds or {:?} — sound effects disabled", parent, base);
        // List directory to help debug
        if let Ok(entries) = std::fs::read_dir(parent) {
            let names: Vec<_> = entries.flatten().map(|e| e.file_name()).collect();
            debug!("Directory contents of {:?}: {:?}", parent, names);
        }
        return;
    };

    info!("Opening Audio.snd at {:?}", snd_path);
    let snd_archive = match SndArchive::open(&snd_path) {
        Ok(a) => a,
        Err(e) => {
            warn!("Failed to open Audio.snd at {:?}: {e}", snd_path);
            return;
        }
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
    });
}
