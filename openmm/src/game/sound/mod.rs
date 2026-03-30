pub(crate) mod effects;
pub(crate) mod music;

use bevy::prelude::*;
use lod::{dsounds::DSounds, snd::SndArchive};
use std::collections::HashMap;
use std::path::Path;

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

        let info = self.dsounds.get_by_id(sound_id)?;
        let name = info.name()?;
        let wav_bytes = self.snd_archive.get(&name)?;

        // Validate WAV header
        if wav_bytes.len() < 44 {
            warn!("Sound '{}' (id={}) too short: {} bytes", name, sound_id, wav_bytes.len());
            return None;
        }
        if &wav_bytes[0..4] != b"RIFF" || &wav_bytes[8..12] != b"WAVE" {
            warn!("Sound '{}' (id={}) not a valid WAV (header: {:?})", name, sound_id, &wav_bytes[0..12]);
            return None;
        }

        // Check fmt chunk
        let fmt_pos = match wav_bytes.windows(4).position(|w| w == b"fmt ") {
            Some(p) => p,
            None => {
                warn!("Sound '{}' (id={}) missing fmt chunk", name, sound_id);
                return None;
            }
        };
        let audio_fmt = u16::from_le_bytes([wav_bytes[fmt_pos + 8], wav_bytes[fmt_pos + 9]]);
        let channels = u16::from_le_bytes([wav_bytes[fmt_pos + 10], wav_bytes[fmt_pos + 11]]);
        let sample_rate = u32::from_le_bytes([wav_bytes[fmt_pos + 12], wav_bytes[fmt_pos + 13], wav_bytes[fmt_pos + 14], wav_bytes[fmt_pos + 15]]);
        let bits = u16::from_le_bytes([wav_bytes[fmt_pos + 22], wav_bytes[fmt_pos + 23]]);

        if audio_fmt != 1 {
            warn!("Sound '{}' (id={}) unsupported format {} (expected PCM=1)", name, sound_id, audio_fmt);
            return None;
        }
        if channels == 0 || sample_rate == 0 || bits == 0 {
            warn!("Sound '{}' (id={}) invalid params: {}ch {}Hz {}bit", name, sound_id, channels, sample_rate, bits);
            return None;
        }

        // Check data chunk exists and has content
        let data_pos = match wav_bytes.windows(4).position(|w| w == b"data") {
            Some(p) => p,
            None => {
                warn!("Sound '{}' (id={}) missing data chunk", name, sound_id);
                return None;
            }
        };
        let data_size = u32::from_le_bytes([wav_bytes[data_pos + 4], wav_bytes[data_pos + 5], wav_bytes[data_pos + 6], wav_bytes[data_pos + 7]]);
        if data_size == 0 {
            warn!("Sound '{}' (id={}) has empty data chunk", name, sound_id);
            return None;
        }

        info!("Loading sound '{}' (id={}): PCM {}ch {}Hz {}bit, {} data bytes, {} total",
            name, sound_id, channels, sample_rate, bits, data_size, wav_bytes.len());

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
        app.add_plugins((music::MusicPlugin, effects::EffectsPlugin))
            .add_systems(Startup, init_sound_manager);
    }
}

fn init_sound_manager(mut commands: Commands, game_assets: Res<GameAssets>) {
    let dsounds = match DSounds::new(game_assets.lod_manager()) {
        Ok(d) => d,
        Err(e) => {
            warn!("Failed to load dsounds.bin: {e}");
            return;
        }
    };

    let data_path = lod::get_data_path();
    let base = Path::new(&data_path);
    // Audio.snd is at the MM6 root/Sounds/Audio.snd
    // data_path might be mm6/data or mm6, try both
    let snd_path = [
        base.join("../Sounds/Audio.snd"),
        base.join("Sounds/Audio.snd"),
    ]
    .into_iter()
    .find(|p| p.exists());

    let Some(snd_path) = snd_path else {
        warn!("Audio.snd not found — sound effects disabled");
        return;
    };

    let snd_archive = match SndArchive::open(&snd_path) {
        Ok(a) => a,
        Err(e) => {
            warn!("Failed to open Audio.snd: {e}");
            return;
        }
    };

    info!(
        "Sound system initialized: {} descriptors, {} audio files",
        dsounds.items.len(),
        snd_archive.list().len()
    );
    commands.insert_resource(SoundManager {
        dsounds,
        snd_archive,
        cache: HashMap::new(),
    });
}
