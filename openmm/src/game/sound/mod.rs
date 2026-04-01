pub(crate) mod effects;
pub(crate) mod footsteps;
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
    /// Cached sound ID for chest-open sound ("openchest0101").
    pub chest_open_sound_id: Option<u32>,
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

        // Validate WAV: RIFF header + PCM format
        if wav_bytes.len() < 44
            || &wav_bytes[0..4] != b"RIFF"
            || &wav_bytes[8..12] != b"WAVE"
        {
            warn!("Sound '{}' (id={}) is not a valid WAV", name, sound_id);
            return None;
        }
        if let Some(fmt_pos) = wav_bytes.windows(4).position(|w| w == b"fmt ") {
            let audio_fmt = u16::from_le_bytes([wav_bytes[fmt_pos + 8], wav_bytes[fmt_pos + 9]]);
            if audio_fmt != 1 {
                warn!("Sound '{}' (id={}) unsupported format {}", name, sound_id, audio_fmt);
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
}

pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        // MM6 world units are large (512 per tile). Scale so ~2 tiles = normal attenuation distance.
        app.add_plugins((music::MusicPlugin, effects::EffectsPlugin, footsteps::FootstepsPlugin))
            .insert_resource(bevy::audio::DefaultSpatialScale(
                bevy::audio::SpatialScale::new(1.0 / 1000.0),
            ))
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
    let chest_open_sound_id = dsounds.get_by_name("openchest0101").map(|s| s.sound_id);
    if chest_open_sound_id.is_none() {
        warn!("Chest open sound 'openchest0101' not found in dsounds");
    }
    commands.insert_resource(SoundManager {
        dsounds,
        snd_archive,
        cache: HashMap::new(),
        chest_open_sound_id,
    });
}
