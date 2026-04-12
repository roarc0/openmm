//! Pre-decoded media cache for video audio and music.
//!
//! Sits between the raw archive layer ([`Assets`]) and the game engine.
//! Avoids repeated decoding of video audio and redundant disk reads for music
//! files by caching results after the first request.
//!
//! # Usage
//!
//! ```ignore
//! let mut media = MediaCache::new();
//!
//! // Eagerly preload assets for a known upcoming screen:
//! media.preload_video_audio("jvc", &assets);
//! media.preload_music("13", &assets);
//!
//! // Later, retrieve cached data (instant):
//! let wav = media.video_audio_wav("jvc", &assets);
//! let mp3 = media.music_bytes("13", &assets);
//! ```

use std::collections::HashMap;

use crate::Assets;

/// Pre-decoded media assets ready for playback.
///
/// - **Video audio**: WAV bytes decoded from SMK videos via
///   [`crate::assets::SmkDecoder::extract_audio_wav`].  Keyed by video name (e.g. `"3dologo"`).
/// - **Music**: Raw MP3 bytes read from the `Music/` directory.
///   Keyed by track name (e.g. `"13"`).
///
/// All methods accept `&Assets` for raw data access, and cache the result
/// internally.  A cache miss triggers I/O + decoding; a hit returns instantly.
#[derive(Default)]
pub struct MediaCache {
    audio_wav: HashMap<String, Vec<u8>>,
    music: HashMap<String, Vec<u8>>,
}

impl MediaCache {
    pub fn new() -> Self {
        Self::default()
    }

    // ── Video audio ─────────────────────────────────────────────────────

    /// Eagerly decode and cache audio for the named SMK video.
    ///
    /// Does nothing if the video is already cached, has no audio, or is not
    /// found in any loaded VID archive.
    pub fn preload_video_audio(&mut self, name: &str, assets: &Assets) {
        if self.audio_wav.contains_key(name) {
            return;
        }
        if let Some(wav) = assets.video_audio_wav(name) {
            log::info!("media_cache: preloaded audio for '{}'", name);
            self.audio_wav.insert(name.to_string(), wav);
        }
    }

    /// Get WAV audio for an SMK video (cached or decoded on demand).
    ///
    /// Returns `None` if the video has no audio track or is not found.
    pub fn video_audio_wav(&mut self, name: &str, assets: &Assets) -> Option<Vec<u8>> {
        if let Some(wav) = self.audio_wav.remove(name) {
            return Some(wav);
        }
        assets.video_audio_wav(name)
    }

    /// Check whether audio is already cached for the given video.
    pub fn has_video_audio(&self, name: &str) -> bool {
        self.audio_wav.contains_key(name)
    }

    // ── Music ───────────────────────────────────────────────────────────

    /// Eagerly read and cache a music file by track name.
    ///
    /// Does nothing if the track is already cached or the file is not found.
    pub fn preload_music(&mut self, track: &str, assets: &Assets) {
        if self.music.contains_key(track) {
            return;
        }
        match assets.get_music(track) {
            Ok(bytes) => {
                log::info!("media_cache: preloaded music '{}'", track);
                self.music.insert(track.to_string(), bytes);
            }
            Err(e) => {
                log::warn!("media_cache: music '{}' not found: {}", track, e);
            }
        }
    }

    /// Get music bytes for a track (cached or read on demand).
    ///
    /// Returns `None` if the music file is not found.
    pub fn music_bytes(&mut self, track: &str, assets: &Assets) -> Option<Vec<u8>> {
        if let Some(bytes) = self.music.remove(track) {
            return Some(bytes);
        }
        assets.get_music(track).ok()
    }

    /// Check whether music is already cached for the given track.
    pub fn has_music(&self, track: &str) -> bool {
        self.music.contains_key(track)
    }

    /// Drop all cached data (useful on state transitions).
    pub fn clear(&mut self) {
        self.audio_wav.clear();
        self.music.clear();
    }
}
