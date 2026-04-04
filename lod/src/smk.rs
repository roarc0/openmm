//! Safe Rust wrapper around the vendored libsmacker C library.
//!
//! libsmacker decodes Smacker (SMK2/SMK4) video files frame by frame.
//! The C library keeps a pointer into the raw data buffer, so `SmkDecoder`
//! owns the data for its entire lifetime.

use std::os::raw::{c_char, c_ulong};

// ── FFI declarations ────────────────────────────────────────────────────────

/// Opaque smk handle (C: `struct smk_t *`).
#[repr(C)]
struct SmkT {
    _private: [u8; 0],
}
type SmkHandle = *mut SmkT;

const SMK_DONE: c_char = 0;
const SMK_ERROR: c_char = -1;

#[link(name = "smacker", kind = "static")]
unsafe extern "C" {
    fn smk_open_memory(buffer: *const u8, size: c_ulong) -> SmkHandle;
    fn smk_close(object: SmkHandle);
    fn smk_info_all(
        object: SmkHandle,
        frame: *mut c_ulong,
        frame_count: *mut c_ulong,
        usf: *mut f64,
    ) -> c_char;
    fn smk_info_video(
        object: SmkHandle,
        w: *mut c_ulong,
        h: *mut c_ulong,
        y_scale_mode: *mut u8,
    ) -> c_char;
    fn smk_enable_video(object: SmkHandle, enable: u8) -> c_char;
    fn smk_enable_audio(object: SmkHandle, track: u8, enable: u8) -> c_char;
    fn smk_first(object: SmkHandle) -> c_char;
    fn smk_next(object: SmkHandle) -> c_char;
    fn smk_get_palette(object: SmkHandle) -> *const u8;
    fn smk_get_video(object: SmkHandle) -> *const u8;
}

// ── Error type ──────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum SmkError {
    OpenFailed,
    InfoFailed,
}

impl std::fmt::Display for SmkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SmkError::OpenFailed => write!(f, "smk_open_memory failed"),
            SmkError::InfoFailed => write!(f, "smk_info_all/video failed"),
        }
    }
}

impl std::error::Error for SmkError {}

// ── Safe wrapper ────────────────────────────────────────────────────────────

/// Decodes an SMK video frame by frame, yielding RGBA pixel data.
///
/// The `_data` field keeps the raw SMK bytes alive — libsmacker holds a pointer
/// into this buffer and reads from it during decoding.
pub struct SmkDecoder {
    handle: SmkHandle,
    _data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub frame_count: u32,
    /// Frames per second derived from the `usf` (microseconds per frame) field.
    pub fps: f32,
    started: bool,
    done: bool,
}

// Safety: SmkDecoder owns its data and handle exclusively; never shared across threads.
unsafe impl Send for SmkDecoder {}

impl SmkDecoder {
    /// Create a decoder from raw SMK bytes (e.g. from `Vid::smk_bytes(...).to_vec()`).
    pub fn new(data: Vec<u8>) -> Result<Self, SmkError> {
        let handle = unsafe { smk_open_memory(data.as_ptr(), data.len() as c_ulong) };
        if handle.is_null() {
            return Err(SmkError::OpenFailed);
        }

        let (frame_count, fps) = unsafe {
            let mut _frame: c_ulong = 0;
            let mut count: c_ulong = 0;
            let mut usf: f64 = 0.0;
            if smk_info_all(handle, &mut _frame, &mut count, &mut usf) == SMK_ERROR {
                smk_close(handle);
                return Err(SmkError::InfoFailed);
            }
            let fps = if usf > 0.0 { 1_000_000.0 / usf as f32 } else { 10.0 };
            (count as u32, fps)
        };

        let (width, height) = unsafe {
            let mut w: c_ulong = 0;
            let mut h: c_ulong = 0;
            let mut _yscale: u8 = 0;
            if smk_info_video(handle, &mut w, &mut h, &mut _yscale) == SMK_ERROR {
                smk_close(handle);
                return Err(SmkError::InfoFailed);
            }
            (w as u32, h as u32)
        };

        // Enable video; disable all 7 audio tracks (audio deferred).
        unsafe {
            smk_enable_video(handle, 1);
            for track in 0..7u8 {
                smk_enable_audio(handle, track, 0);
            }
        }

        Ok(Self {
            handle,
            _data: data,
            width,
            height,
            frame_count,
            fps,
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

        let result = unsafe {
            if !self.started {
                self.started = true;
                smk_first(self.handle)
            } else {
                smk_next(self.handle)
            }
        };

        if result == SMK_ERROR || result == SMK_DONE {
            self.done = true;
            return None;
        }

        // SMK_MORE (1) or SMK_LAST (2) — we have a frame. Mark done on LAST.
        if result == 2 {
            self.done = true;
        }

        Some(self.decode_current_frame())
    }

    fn decode_current_frame(&self) -> Vec<u8> {
        let pixels = self.width as usize * self.height as usize;
        let mut rgba = vec![0u8; pixels * 4];
        unsafe {
            let palette = smk_get_palette(self.handle);
            let video = smk_get_video(self.handle);
            for i in 0..pixels {
                let idx = *video.add(i) as usize;
                rgba[i * 4] = *palette.add(idx * 3);
                rgba[i * 4 + 1] = *palette.add(idx * 3 + 1);
                rgba[i * 4 + 2] = *palette.add(idx * 3 + 2);
                rgba[i * 4 + 3] = 255;
            }
        }
        rgba
    }
}

impl Drop for SmkDecoder {
    fn drop(&mut self) {
        unsafe { smk_close(self.handle) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vid::Vid;

    #[test]
    fn smk_decoder_reads_3dologo() {
        let data_path = crate::get_data_path();
        let vid_path = std::path::Path::new(&data_path).join("Anims/Anims2.vid");
        if !vid_path.exists() {
            eprintln!("test: MM6 Anims not found — skipping");
            return;
        }
        let vid = Vid::open(&vid_path).expect("open Anims2.vid");
        let idx = vid
            .entries
            .iter()
            .position(|e| e.name.eq_ignore_ascii_case("3dologo"))
            .expect("3dologo not found in Anims2.vid");
        let bytes = vid.smk_bytes(idx).to_vec();

        let mut dec = SmkDecoder::new(bytes).expect("SmkDecoder::new");
        assert_eq!(dec.width, 640);
        assert_eq!(dec.height, 480);
        assert!(dec.frame_count > 0, "should have frames");
        assert!(dec.fps > 0.0, "fps should be positive");

        let frame = dec.next_frame().expect("first frame should exist");
        assert_eq!(frame.len(), (dec.width * dec.height * 4) as usize);
        // All alpha values must be 255
        assert!(frame.iter().skip(3).step_by(4).all(|&a| a == 255));
    }
}
