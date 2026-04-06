//! SMK video decoder — thin wrapper around the `smk` crate.

use ::smk::{FrameStatus, Smk};

pub use ::smk::SmkError;

/// Audio properties for the active track in an SMK file.
#[derive(Debug, Clone, Copy)]
pub struct SmkAudioInfo {
    pub track: u8,
    pub rate: u32,
    pub channels: u8,
    pub bitdepth: u8,
}

/// Decodes an SMK video frame by frame, yielding RGBA pixel data.
pub struct SmkDecoder {
    inner: Smk,
    pub width: u32,
    pub height: u32,
    pub frame_count: u32,
    /// Frames per second derived from the `usf` (microseconds per frame) field.
    pub fps: f32,
    /// Active audio track and its properties, if any audio is present.
    pub audio: Option<SmkAudioInfo>,
    started: bool,
    done: bool,
}

impl SmkDecoder {
    /// Create a decoder from raw SMK bytes.
    pub fn new(data: Vec<u8>) -> Result<Self, SmkError> {
        let mut inner = Smk::open_memory(&data)?;

        let info = inner.info();
        let frame_count = info.frame_count;
        let fps = if info.microseconds_per_frame > 0.0 {
            1_000_000.0 / info.microseconds_per_frame as f32
        } else {
            10.0
        };

        let video = inner.info_video();
        let (width, height) = (video.width, video.height);

        let ai = inner.info_audio();
        let audio = (0..7u8).find(|&t| ai.track_mask & (1 << t) != 0).map(|t| SmkAudioInfo {
            track: t,
            rate: ai.rate[t as usize],
            channels: ai.channels[t as usize],
            bitdepth: ai.bitdepth[t as usize],
        });

        inner.enable_video(true);
        for t in 0..7u8 {
            inner.enable_audio(t, audio.is_some_and(|a| a.track == t));
        }

        Ok(Self {
            inner,
            width,
            height,
            frame_count,
            fps,
            audio,
            started: false,
            done: false,
        })
    }

    /// Decode and return the next frame as RGBA pixels (`width * height * 4` bytes).
    /// Returns `None` when all frames are exhausted.
    pub fn next_frame(&mut self) -> Option<Vec<u8>> {
        if self.done {
            return None;
        }

        let result = if !self.started {
            self.started = true;
            self.inner.first_frame()
        } else {
            self.inner.next_frame()
        };

        match result {
            Err(_) | Ok(FrameStatus::Done) => {
                self.done = true;
                None
            }
            Ok(FrameStatus::Last) => {
                self.done = true;
                Some(self.decode_current_frame())
            }
            Ok(FrameStatus::More) => Some(self.decode_current_frame()),
        }
    }

    /// Return raw PCM bytes for the current frame's audio chunk.
    /// Returns an empty vec if no audio track is active or the chunk is empty.
    pub fn decode_current_audio(&self) -> Vec<u8> {
        let Some(info) = self.audio else { return Vec::new() };
        self.inner
            .audio_data(info.track)
            .map(|d| d.to_vec())
            .unwrap_or_default()
    }

    fn decode_current_frame(&self) -> Vec<u8> {
        let pixels = self.width as usize * self.height as usize;
        let palette = self.inner.palette();
        let video = self.inner.video_data();
        let mut rgba = vec![0u8; pixels * 4];
        for i in 0..pixels {
            let [r, g, b] = palette[video[i] as usize];
            rgba[i * 4] = r;
            rgba[i * 4 + 1] = g;
            rgba[i * 4 + 2] = b;
            rgba[i * 4 + 3] = 255;
        }
        rgba
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openmm_archive::Archive;
    use crate::assets::vid::Vid;

    #[test]
    fn smk_decoder_reads_3dologo() {
        let data_path = crate::get_data_path();
        let vid_path = std::path::Path::new(&data_path).join("Anims/Anims2.vid");
        if !vid_path.exists() {
            eprintln!("test: MM6 Anims not found — skipping");
            return;
        }
        let vid = Vid::open(&vid_path).expect("open Anims2.vid");
        let bytes = vid
            .list_files()
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case("3dologo"))
            .and_then(|e| vid.get_file(&e.name))
            .expect("3dologo not found in Anims2.vid");

        let mut dec = SmkDecoder::new(bytes).expect("SmkDecoder::new");
        assert_eq!(dec.width, 640);
        assert_eq!(dec.height, 480);
        assert!(dec.frame_count > 0, "should have frames");
        assert!(dec.fps > 0.0, "fps should be positive");

        let frame = dec.next_frame().expect("first frame should exist");
        assert_eq!(frame.len(), (dec.width * dec.height * 4) as usize);
        assert!(frame.iter().skip(3).step_by(4).all(|&a| a == 255));
    }
}
