use std::{
    collections::HashMap,
    error::Error,
    fs,
    io::{Cursor, Read},
    path::Path,
};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::utils::try_read_name;

struct SndEntry {
    offset: u32,
    size: u32,
    decompressed_size: u32,
}

pub struct SndArchive {
    entries: HashMap<String, SndEntry>,
    data: Vec<u8>,
}

impl SndArchive {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn Error>> {
        let data = fs::read(path)?;
        let mut cursor = Cursor::new(&data);
        let entry_count = cursor.read_u32::<LittleEndian>()?;

        let mut entries = HashMap::with_capacity(entry_count as usize);
        for _ in 0..entry_count {
            let mut name_buf = [0u8; 40];
            cursor.read_exact(&mut name_buf)?;
            let name = try_read_name(&name_buf).unwrap_or_default();
            let offset = cursor.read_u32::<LittleEndian>()?;
            let size = cursor.read_u32::<LittleEndian>()?;
            let decompressed_size = cursor.read_u32::<LittleEndian>()?;

            if !name.is_empty() {
                entries.insert(name, SndEntry { offset, size, decompressed_size });
            }
        }

        Ok(Self { entries, data })
    }

    /// Extract a sound file by name. Returns PCM WAV bytes ready for playback.
    /// IMA-ADPCM encoded WAVs are automatically decoded to 16-bit PCM.
    pub fn get(&self, name: &str) -> Option<Vec<u8>> {
        let entry = self.entries.get(&name.to_lowercase())?;
        let start = entry.offset as usize;
        let end = start + entry.size as usize;
        if end > self.data.len() {
            return None;
        }
        let raw = &self.data[start..end];

        let wav = if entry.decompressed_size > 0 {
            crate::zlib::decompress(raw, entry.size as usize, entry.decompressed_size as usize)
                .ok()?
        } else {
            raw.to_vec()
        };

        // Check if this is IMA-ADPCM (format 17) and decode to PCM
        if wav.len() > 20 {
            let fmt_offset = find_chunk(&wav, b"fmt ")?;
            let audio_format = u16::from_le_bytes([wav[fmt_offset + 8], wav[fmt_offset + 9]]);
            if audio_format == 17 {
                return ima_adpcm_to_pcm(&wav);
            }
        }

        Some(wav)
    }

    /// Extract a sound with optional audio enhancement.
    /// Applies DSP processing to the PCM samples before returning WAV bytes.
    pub fn get_enhanced(&self, name: &str, opts: &AudioEnhance) -> Option<Vec<u8>> {
        let wav = self.get(name)?;
        if opts.is_none() {
            return Some(wav);
        }
        let (mut samples, channels, sample_rate) = pcm_samples_from_wav(&wav)?;
        opts.apply(&mut samples, sample_rate);
        Some(build_pcm_wav(&samples, channels, sample_rate))
    }

    pub fn list(&self) -> Vec<&str> {
        self.entries.keys().map(|s| s.as_str()).collect()
    }

    pub fn exists(&self, name: &str) -> bool {
        self.entries.contains_key(&name.to_lowercase())
    }
}

// ── IMA-ADPCM to PCM conversion ────────────────────────────

/// Find a RIFF chunk by its 4-byte ID. Returns offset to the chunk header.
fn find_chunk(wav: &[u8], id: &[u8; 4]) -> Option<usize> {
    // RIFF header is 12 bytes, then chunks follow
    let mut pos = 12;
    while pos + 8 <= wav.len() {
        if &wav[pos..pos + 4] == id {
            return Some(pos);
        }
        let chunk_size = u32::from_le_bytes([wav[pos + 4], wav[pos + 5], wav[pos + 6], wav[pos + 7]]) as usize;
        pos += 8 + chunk_size;
        // Chunks are word-aligned
        if pos % 2 != 0 {
            pos += 1;
        }
    }
    None
}

/// IMA-ADPCM step size index table
const INDEX_TABLE: [i32; 16] = [-1, -1, -1, -1, 2, 4, 6, 8, -1, -1, -1, -1, 2, 4, 6, 8];

/// IMA-ADPCM step size table
const STEP_TABLE: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45,
    50, 55, 60, 66, 73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209, 230,
    253, 279, 307, 337, 371, 408, 449, 494, 544, 598, 658, 724, 796, 876, 963,
    1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066, 2272, 2499, 2749, 3024, 3327,
    3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630, 9493, 10442,
    11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794,
    32767,
];

/// Decode one IMA-ADPCM nibble, updating predictor and step index in place.
fn decode_nibble(nibble: u8, predictor: &mut i32, step_index: &mut i32) -> i16 {
    let step = STEP_TABLE[*step_index as usize];
    let nibble = nibble as i32;

    // Compute difference
    let mut diff = step >> 3;
    if nibble & 1 != 0 { diff += step >> 2; }
    if nibble & 2 != 0 { diff += step >> 1; }
    if nibble & 4 != 0 { diff += step; }
    if nibble & 8 != 0 { diff = -diff; }

    *predictor = (*predictor + diff).clamp(-32768, 32767);
    *step_index = (*step_index + INDEX_TABLE[nibble as usize]).clamp(0, 88);

    *predictor as i16
}

/// Convert an IMA-ADPCM WAV to 16-bit PCM WAV.
fn ima_adpcm_to_pcm(wav: &[u8]) -> Option<Vec<u8>> {
    let fmt_offset = find_chunk(wav, b"fmt ")?;
    let fmt_size = u32::from_le_bytes([
        wav[fmt_offset + 4], wav[fmt_offset + 5],
        wav[fmt_offset + 6], wav[fmt_offset + 7],
    ]) as usize;

    // Parse IMA-ADPCM fmt chunk
    let fmt = &wav[fmt_offset + 8..fmt_offset + 8 + fmt_size];
    if fmt.len() < 20 { return None; }

    let channels = u16::from_le_bytes([fmt[2], fmt[3]]) as usize;
    let sample_rate = u32::from_le_bytes([fmt[4], fmt[5], fmt[6], fmt[7]]);
    let block_align = u16::from_le_bytes([fmt[12], fmt[13]]) as usize;
    // samples_per_block is at offset 18 in the extended fmt chunk
    let samples_per_block = u16::from_le_bytes([fmt[18], fmt[19]]) as usize;

    if channels == 0 || block_align == 0 || samples_per_block == 0 {
        return None;
    }

    // Find data chunk
    let data_offset = find_chunk(wav, b"data")?;
    let data_size = u32::from_le_bytes([
        wav[data_offset + 4], wav[data_offset + 5],
        wav[data_offset + 6], wav[data_offset + 7],
    ]) as usize;
    let adpcm_data = &wav[data_offset + 8..data_offset + 8 + data_size];

    // Decode all blocks
    let num_blocks = adpcm_data.len() / block_align;
    let total_samples = num_blocks * samples_per_block * channels;
    let mut pcm_samples: Vec<i16> = Vec::with_capacity(total_samples);

    for block_idx in 0..num_blocks {
        let block = &adpcm_data[block_idx * block_align..(block_idx + 1) * block_align];

        // For mono: block header is 4 bytes (predictor i16 + step_index u8 + reserved u8)
        // For stereo: 4 bytes per channel
        let mut predictors = vec![0i32; channels];
        let mut step_indices = vec![0i32; channels];

        for ch in 0..channels {
            let hdr = ch * 4;
            if hdr + 4 > block.len() { return None; }
            predictors[ch] = i16::from_le_bytes([block[hdr], block[hdr + 1]]) as i32;
            step_indices[ch] = (block[hdr + 2] as i32).clamp(0, 88);
            // First sample comes from the header predictor
            pcm_samples.push(predictors[ch] as i16);
        }

        // Decode the rest of the block (nibbles after the header)
        let payload = &block[channels * 4..];

        if channels == 1 {
            // Mono: nibbles are sequential, low nibble first
            for &byte in payload {
                let lo = byte & 0x0F;
                let hi = (byte >> 4) & 0x0F;
                pcm_samples.push(decode_nibble(lo, &mut predictors[0], &mut step_indices[0]));
                pcm_samples.push(decode_nibble(hi, &mut predictors[0], &mut step_indices[0]));
            }
        } else {
            // Stereo: interleaved in 4-byte (8 nibble) chunks per channel
            let mut i = 0;
            while i + channels * 4 <= payload.len() {
                for ch in 0..channels {
                    for b in 0..4 {
                        let byte = payload[i + ch * 4 + b];
                        let lo = byte & 0x0F;
                        let hi = (byte >> 4) & 0x0F;
                        pcm_samples.push(decode_nibble(lo, &mut predictors[ch], &mut step_indices[ch]));
                        pcm_samples.push(decode_nibble(hi, &mut predictors[ch], &mut step_indices[ch]));
                    }
                }
                i += channels * 4;
            }
        }
    }

    // Trim to exact sample count (last block may be partial)
    pcm_samples.truncate(total_samples);

    Some(build_pcm_wav(&pcm_samples, channels as u16, sample_rate))
}

// ── Audio enhancement ───────────────────────────────────────

/// Optional audio enhancements applied to PCM samples after extraction.
/// All fields default to `false` / off.
#[derive(Debug, Clone, Default)]
pub struct AudioEnhance {
    /// Gentle low-pass filter to tame crunchy/fizzy highs.
    /// Cutoff is relative to Nyquist (0.0–1.0). Typical: 0.8 = keep 80% of spectrum.
    pub low_pass: Option<f32>,
    /// Light denoise via noise gate. Threshold in sample amplitude (0–32767).
    /// Samples below threshold are faded toward silence. Typical: 200–600.
    pub denoise_threshold: Option<i16>,
    /// High-shelf EQ cut to reduce harsh upper mids. Gain in dB (negative = cut).
    /// Applied above ~4kHz at 22050Hz sample rate. Typical: -3.0 to -6.0.
    pub high_shelf_db: Option<f32>,
    /// Declip: repair samples at or near max amplitude by interpolation.
    /// Threshold as fraction of max (0.0–1.0). Typical: 0.95.
    pub declip_threshold: Option<f32>,
    /// De-ess: attenuate sibilant frequencies (4–8kHz range).
    /// Amount is reduction in dB when sibilance detected. Typical: -4.0 to -8.0.
    pub deess_db: Option<f32>,
}

impl AudioEnhance {
    /// Returns true if no enhancements are enabled.
    pub fn is_none(&self) -> bool {
        self.low_pass.is_none()
            && self.denoise_threshold.is_none()
            && self.high_shelf_db.is_none()
            && self.declip_threshold.is_none()
            && self.deess_db.is_none()
    }

    /// Preset: gentle cleanup suitable for old game sound effects.
    pub fn gentle() -> Self {
        Self {
            low_pass: Some(0.85),
            denoise_threshold: Some(300),
            high_shelf_db: Some(-3.0),
            declip_threshold: Some(0.95),
            deess_db: None,
        }
    }

    /// Preset: voice cleanup with de-essing.
    pub fn voice() -> Self {
        Self {
            low_pass: Some(0.9),
            denoise_threshold: Some(400),
            high_shelf_db: Some(-2.0),
            declip_threshold: Some(0.95),
            deess_db: Some(-6.0),
        }
    }

    /// Apply all enabled enhancements to PCM samples in order.
    pub fn apply(&self, samples: &mut [i16], sample_rate: u32) {
        if let Some(threshold) = self.declip_threshold {
            dsp_declip(samples, threshold);
        }
        if let Some(threshold) = self.denoise_threshold {
            dsp_denoise(samples, threshold);
        }
        if let Some(cutoff) = self.low_pass {
            dsp_low_pass(samples, cutoff);
        }
        if let Some(db) = self.high_shelf_db {
            dsp_high_shelf(samples, sample_rate, db);
        }
        if let Some(db) = self.deess_db {
            dsp_deess(samples, sample_rate, db);
        }
    }
}

/// Extract raw i16 samples from a PCM WAV. Returns (samples, channels, sample_rate).
fn pcm_samples_from_wav(wav: &[u8]) -> Option<(Vec<i16>, u16, u32)> {
    let fmt_offset = find_chunk(wav, b"fmt ")?;
    let fmt = &wav[fmt_offset + 8..];
    let channels = u16::from_le_bytes([fmt[2], fmt[3]]);
    let sample_rate = u32::from_le_bytes([fmt[4], fmt[5], fmt[6], fmt[7]]);

    let data_offset = find_chunk(wav, b"data")?;
    let data_size = u32::from_le_bytes([
        wav[data_offset + 4], wav[data_offset + 5],
        wav[data_offset + 6], wav[data_offset + 7],
    ]) as usize;
    let pcm_data = &wav[data_offset + 8..data_offset + 8 + data_size];

    let samples: Vec<i16> = pcm_data
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect();

    Some((samples, channels, sample_rate))
}

/// Build a PCM WAV file from raw i16 samples.
fn build_pcm_wav(samples: &[i16], channels: u16, sample_rate: u32) -> Vec<u8> {
    let pcm_data_size = (samples.len() * 2) as u32;
    let byte_rate = sample_rate * channels as u32 * 2;
    let block_align = channels * 2;
    let file_size = 36 + pcm_data_size;

    let mut out = Vec::with_capacity(44 + pcm_data_size as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&file_size.to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&pcm_data_size.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

// ── DSP functions ───────────────────────────────────────────

/// Single-pole low-pass filter. `cutoff` is 0.0–1.0 relative to Nyquist.
fn dsp_low_pass(samples: &mut [i16], cutoff: f32) {
    let cutoff = cutoff.clamp(0.01, 1.0);
    // RC filter coefficient: higher cutoff = less filtering
    let rc = 1.0 / (cutoff * std::f32::consts::PI);
    // dt = 1.0 (normalized)
    let alpha = 1.0 / (1.0 + rc);

    let mut prev = samples.first().copied().unwrap_or(0) as f32;
    for s in samples.iter_mut() {
        let x = *s as f32;
        prev += alpha * (x - prev);
        *s = prev.round().clamp(-32768.0, 32767.0) as i16;
    }
}

/// Noise gate with soft knee. Samples below threshold fade toward zero.
fn dsp_denoise(samples: &mut [i16], threshold: i16) {
    let thresh = threshold.unsigned_abs() as f32;
    // Soft knee: full gate below thresh/2, linear ramp to thresh
    let knee_low = thresh * 0.5;

    for s in samples.iter_mut() {
        let abs = (*s as f32).abs();
        if abs < knee_low {
            *s = 0;
        } else if abs < thresh {
            let gain = (abs - knee_low) / (thresh - knee_low);
            *s = (*s as f32 * gain).round() as i16;
        }
    }
}

/// High-shelf filter: attenuate frequencies above ~4kHz.
/// Uses a first-order shelf approximation.
fn dsp_high_shelf(samples: &mut [i16], sample_rate: u32, gain_db: f32) {
    // Shelf frequency at ~4kHz
    let freq = 4000.0;
    let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate as f32;
    let gain = 10.0_f32.powf(gain_db / 20.0);

    // First-order shelf coefficients
    let tan_w = (w0 / 2.0).tan();
    let a = (tan_w + gain) / (tan_w + 1.0);
    let b = (tan_w - gain) / (tan_w + 1.0);
    let c = (tan_w - 1.0) / (tan_w + 1.0);

    let mut x_prev = 0.0_f32;
    let mut y_prev = 0.0_f32;

    for s in samples.iter_mut() {
        let x = *s as f32;
        let y = a * x + b * x_prev - c * y_prev;
        x_prev = x;
        y_prev = y;
        *s = y.round().clamp(-32768.0, 32767.0) as i16;
    }
}

/// Declip: detect clipped samples and interpolate from neighbors.
fn dsp_declip(samples: &mut [i16], threshold: f32) {
    let clip_level = (32767.0 * threshold.clamp(0.5, 1.0)) as i16;
    let len = samples.len();
    if len < 3 {
        return;
    }

    // Mark clipped regions
    let clipped: Vec<bool> = samples.iter().map(|&s| s.abs() >= clip_level).collect();

    // Interpolate clipped samples from nearest unclipped neighbors
    let mut i = 0;
    while i < len {
        if clipped[i] {
            // Find the clipped run
            let start = i;
            while i < len && clipped[i] {
                i += 1;
            }
            let end = i;

            // Get boundary values
            let left = if start > 0 { samples[start - 1] as f32 } else { 0.0 };
            let right = if end < len { samples[end] as f32 } else { 0.0 };
            let run_len = (end - start) as f32 + 1.0;

            for (j, sample) in samples[start..end].iter_mut().enumerate() {
                let t = (j + 1) as f32 / run_len;
                *sample = (left + t * (right - left)).round().clamp(-32768.0, 32767.0) as i16;
            }
        } else {
            i += 1;
        }
    }
}

/// De-esser: detect sibilant energy (4–8kHz) and attenuate when it spikes.
/// Uses a bandpass detector and gain reduction.
fn dsp_deess(samples: &mut [i16], sample_rate: u32, reduction_db: f32) {
    let reduction = 10.0_f32.powf(reduction_db / 20.0);

    // Bandpass filter coefficients for sibilant detection (~6kHz center)
    let center_freq = 6000.0_f32.min(sample_rate as f32 * 0.45);
    let w0 = 2.0 * std::f32::consts::PI * center_freq / sample_rate as f32;
    let q = 1.0; // moderate Q
    let alpha = w0.sin() / (2.0 * q);

    let b0 = alpha;
    let b1 = 0.0;
    let b2 = -alpha;
    let a0 = 1.0 + alpha;
    let a1 = -2.0 * w0.cos();
    let a2 = 1.0 - alpha;

    // Normalize
    let b0 = b0 / a0;
    let b1 = b1 / a0;
    let b2 = b2 / a0;
    let a1 = a1 / a0;
    let a2 = a2 / a0;

    // Envelope follower parameters
    let attack = (-1.0 / (sample_rate as f32 * 0.001)).exp(); // 1ms attack
    let release = (-1.0 / (sample_rate as f32 * 0.050)).exp(); // 50ms release

    let mut x1 = 0.0_f32;
    let mut x2 = 0.0_f32;
    let mut y1 = 0.0_f32;
    let mut y2 = 0.0_f32;
    let mut envelope = 0.0_f32;

    // Compute RMS of the full signal for threshold
    let rms: f32 = (samples.iter().map(|&s| (s as f32).powi(2)).sum::<f32>()
        / samples.len().max(1) as f32)
        .sqrt();
    let threshold = rms * 1.5; // trigger when sibilant band exceeds 1.5x RMS

    for s in samples.iter_mut() {
        let x = *s as f32;

        // Bandpass filter for sibilant detection
        let bp = b0 * x + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
        x2 = x1;
        x1 = x;
        y2 = y1;
        y1 = bp;

        // Envelope follower
        let abs_bp = bp.abs();
        let coeff = if abs_bp > envelope { attack } else { release };
        envelope = coeff * envelope + (1.0 - coeff) * abs_bp;

        // Apply gain reduction when sibilant energy exceeds threshold
        if envelope > threshold {
            let excess = (envelope - threshold) / threshold;
            let gain = 1.0 + excess * (reduction - 1.0); // blend toward reduction
            let gain = gain.clamp(reduction, 1.0);
            *s = (x * gain).round().clamp(-32768.0, 32767.0) as i16;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SndArchive;
    use std::path::Path;

    fn snd_path() -> String {
        let data_path = crate::get_data_path();
        let base = Path::new(&data_path);
        for candidate in &[
            base.join("../Sounds/Audio.snd"),
            base.join("Sounds/Audio.snd"),
        ] {
            if candidate.exists() {
                return candidate.to_string_lossy().to_string();
            }
        }
        String::from("../target/mm6/Sounds/Audio.snd")
    }

    #[test]
    fn read_snd_archive_works() {
        let archive = SndArchive::open(snd_path()).unwrap();
        let entries = archive.list();
        assert!(
            entries.len() > 1000,
            "should have >1000 sound entries, got {}",
            entries.len()
        );
        assert!(archive.exists("01archera_attack"), "should find 01archera_attack");
    }

    #[test]
    fn extract_wav_works() {
        let archive = SndArchive::open(snd_path()).unwrap();
        let wav = archive.get("01archera_attack").expect("should extract sound");
        assert!(wav.len() > 44, "WAV should be longer than header");
        assert_eq!(&wav[0..4], b"RIFF", "should start with RIFF");
        assert_eq!(&wav[8..12], b"WAVE", "should contain WAVE marker");
    }
}
