pub use crate::assets::provider::archive::snd::*;

use crate::assets::provider::archive::Archive;

pub trait SndExt {
    fn list(&self) -> Vec<String>;
    fn get(&self, name: &str) -> Option<Vec<u8>>;
    fn get_enhanced(&self, name: &str, opts: &AudioEnhance) -> Option<Vec<u8>>;
}

impl SndExt for SndArchive {
    fn list(&self) -> Vec<String> {
        self.list_files().iter().map(|e| e.name.clone()).collect()
    }

    fn get(&self, name: &str) -> Option<Vec<u8>> {
        let wav = self.get_file(name)?;

        // Check if this is IMA-ADPCM (format 17) and decode to PCM
        if wav.len() > 20
            && let Some(fmt_offset) = find_chunk(&wav, b"fmt ")
            && fmt_offset + 9 < wav.len()
        {
            let audio_format = u16::from_le_bytes([wav[fmt_offset + 8], wav[fmt_offset + 9]]);
            if audio_format == 17 {
                return ima_adpcm_to_pcm(&wav);
            }
        }

        Some(wav)
    }

    fn get_enhanced(&self, name: &str, opts: &AudioEnhance) -> Option<Vec<u8>> {
        let wav = self.get(name)?;
        if opts.is_none() {
            return Some(wav);
        }
        let (mut samples, channels, sample_rate) = pcm_samples_from_wav(&wav)?;
        opts.apply(&mut samples, sample_rate);
        Some(build_pcm_wav(&samples, channels, sample_rate))
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

const INDEX_TABLE: [i32; 16] = [-1, -1, -1, -1, 2, 4, 6, 8, -1, -1, -1, -1, 2, 4, 6, 8];
const STEP_TABLE: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66, 73, 80, 88, 97, 107,
    118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408, 449, 494, 544, 598, 658, 724, 796, 876, 963,
    1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066, 2272, 2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894,
    6484, 7132, 7845, 8630, 9493, 10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794,
    32767,
];

fn decode_nibble(nibble: u8, predictor: &mut i32, step_index: &mut i32) -> i16 {
    let step = STEP_TABLE[*step_index as usize];
    let nibble = nibble as i32;
    let mut diff = step >> 3;
    if nibble & 1 != 0 {
        diff += step >> 2;
    }
    if nibble & 2 != 0 {
        diff += step >> 1;
    }
    if nibble & 4 != 0 {
        diff += step;
    }
    if nibble & 8 != 0 {
        diff = -diff;
    }
    *predictor = (*predictor + diff).clamp(-32768, 32767);
    *step_index = (*step_index + INDEX_TABLE[nibble as usize]).clamp(0, 88);
    *predictor as i16
}

pub fn ima_adpcm_to_pcm(wav: &[u8]) -> Option<Vec<u8>> {
    let fmt_offset = find_chunk(wav, b"fmt ")?;
    let fmt_size = u32::from_le_bytes([
        wav[fmt_offset + 4],
        wav[fmt_offset + 5],
        wav[fmt_offset + 6],
        wav[fmt_offset + 7],
    ]) as usize;
    let fmt = &wav[fmt_offset + 8..fmt_offset + 8 + fmt_size];
    if fmt.len() < 20 {
        return None;
    }
    let channels = u16::from_le_bytes([fmt[2], fmt[3]]) as usize;
    let sample_rate = u32::from_le_bytes([fmt[4], fmt[5], fmt[6], fmt[7]]);
    let block_align = u16::from_le_bytes([fmt[12], fmt[13]]) as usize;
    let samples_per_block = u16::from_le_bytes([fmt[18], fmt[19]]) as usize;
    if channels == 0 || block_align == 0 || samples_per_block == 0 {
        return None;
    }

    let data_offset = find_chunk(wav, b"data")?;
    let data_size = u32::from_le_bytes([
        wav[data_offset + 4],
        wav[data_offset + 5],
        wav[data_offset + 6],
        wav[data_offset + 7],
    ]) as usize;
    let adpcm_data = &wav[data_offset + 8..data_offset + 8 + data_size];

    let num_full_blocks = adpcm_data.len() / block_align;
    let has_partial = adpcm_data.len() % block_align > (channels * 4);
    let num_blocks = num_full_blocks + if has_partial { 1 } else { 0 };

    let total_samples = num_blocks * samples_per_block * channels;
    let mut pcm_samples: Vec<i16> = Vec::with_capacity(total_samples);

    for block_idx in 0..num_blocks {
        let start = block_idx * block_align;
        let end = ((block_idx + 1) * block_align).min(adpcm_data.len());
        let block = &adpcm_data[start..end];

        let mut predictors = vec![0i32; channels];
        let mut step_indices = vec![0i32; channels];
        for ch in 0..channels {
            let hdr = ch * 4;
            if hdr + 4 > block.len() {
                break; // Should not happen given has_partial check
            }
            predictors[ch] = i16::from_le_bytes([block[hdr], block[hdr + 1]]) as i32;
            step_indices[ch] = (block[hdr + 2] as i32).clamp(0, 88);
            pcm_samples.push(predictors[ch] as i16);
        }
        let payload = if block.len() > channels * 4 {
            &block[channels * 4..]
        } else {
            &[]
        };

        if channels == 1 {
            for &byte in payload {
                let lo = byte & 0x0F;
                let hi = (byte >> 4) & 0x0F;
                pcm_samples.push(decode_nibble(lo, &mut predictors[0], &mut step_indices[0]));
                pcm_samples.push(decode_nibble(hi, &mut predictors[0], &mut step_indices[0]));
            }
        } else {
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
    pcm_samples.truncate(total_samples);
    Some(build_pcm_wav(&pcm_samples, channels as u16, sample_rate))
}

// ── Audio enhancement ───────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct AudioEnhance {
    pub low_pass: Option<f32>,
    pub denoise_threshold: Option<i16>,
    pub high_shelf_db: Option<f32>,
    pub declip_threshold: Option<f32>,
    pub deess_db: Option<f32>,
}

impl AudioEnhance {
    pub fn is_none(&self) -> bool {
        self.low_pass.is_none()
            && self.denoise_threshold.is_none()
            && self.high_shelf_db.is_none()
            && self.declip_threshold.is_none()
            && self.deess_db.is_none()
    }
    pub fn gentle() -> Self {
        Self {
            low_pass: Some(0.85),
            denoise_threshold: Some(300),
            high_shelf_db: Some(-3.0),
            declip_threshold: Some(0.95),
            deess_db: None,
        }
    }
    pub fn voice() -> Self {
        Self {
            low_pass: Some(0.9),
            denoise_threshold: Some(400),
            high_shelf_db: Some(-2.0),
            declip_threshold: Some(0.95),
            deess_db: Some(-6.0),
        }
    }
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

fn pcm_samples_from_wav(wav: &[u8]) -> Option<(Vec<i16>, u16, u32)> {
    let fmt_offset = find_chunk(wav, b"fmt ")?;
    let fmt = &wav[fmt_offset + 8..];
    let channels = u16::from_le_bytes([fmt[2], fmt[3]]);
    let sample_rate = u32::from_le_bytes([fmt[4], fmt[5], fmt[6], fmt[7]]);
    let data_offset = find_chunk(wav, b"data")?;
    let data_size = u32::from_le_bytes([
        wav[data_offset + 4],
        wav[data_offset + 5],
        wav[data_offset + 6],
        wav[data_offset + 7],
    ]) as usize;
    let pcm_data = &wav[data_offset + 8..data_offset + 8 + data_size];
    let samples: Vec<i16> = pcm_data
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect();
    Some((samples, channels, sample_rate))
}

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

fn dsp_low_pass(samples: &mut [i16], cutoff: f32) {
    let cutoff = cutoff.clamp(0.01, 1.0);
    let rc = 1.0 / (cutoff * std::f32::consts::PI);
    let alpha = 1.0 / (1.0 + rc);
    let mut prev = samples.first().copied().unwrap_or(0) as f32;
    for s in samples.iter_mut() {
        let x = *s as f32;
        prev += alpha * (x - prev);
        *s = prev.round().clamp(-32768.0, 32767.0) as i16;
    }
}

fn dsp_denoise(samples: &mut [i16], threshold: i16) {
    let thresh = threshold.unsigned_abs() as f32;
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

fn dsp_high_shelf(samples: &mut [i16], sample_rate: u32, gain_db: f32) {
    let freq = 4000.0;
    let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate as f32;
    let gain = 10.0_f32.powf(gain_db / 20.0);
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

fn dsp_declip(samples: &mut [i16], threshold: f32) {
    let clip_level = (32767.0 * threshold.clamp(0.5, 1.0)) as i16;
    let len = samples.len();
    if len < 3 {
        return;
    }
    let clipped: Vec<bool> = samples.iter().map(|&s| s.abs() >= clip_level).collect();
    let mut i = 0;
    while i < len {
        if clipped[i] {
            let start = i;
            while i < len && clipped[i] {
                i += 1;
            }
            let end = i;
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

fn dsp_deess(samples: &mut [i16], sample_rate: u32, reduction_db: f32) {
    let reduction = 10.0_f32.powf(reduction_db / 20.0);
    let center_freq = 6000.0_f32.min(sample_rate as f32 * 0.45);
    let w0 = 2.0 * std::f32::consts::PI * center_freq / sample_rate as f32;
    let q = 1.0;
    let alpha = w0.sin() / (2.0 * q);
    let b0 = alpha / (1.0 + alpha);
    let b1 = 0.0;
    let b2 = -alpha / (1.0 + alpha);
    let a1 = -2.0 * w0.cos() / (1.0 + alpha);
    let a2 = (1.0 - alpha) / (1.0 + alpha);
    let attack = (-1.0 / (sample_rate as f32 * 0.001)).exp();
    let release = (-1.0 / (sample_rate as f32 * 0.050)).exp();
    let mut x1 = 0.0_f32;
    let mut x2 = 0.0_f32;
    let mut y1 = 0.0_f32;
    let mut y2 = 0.0_f32;
    let mut envelope = 0.0_f32;
    let rms: f32 = (samples.iter().map(|&s| (s as f32).powi(2)).sum::<f32>() / samples.len().max(1) as f32).sqrt();
    let threshold = rms * 1.5;
    for s in samples.iter_mut() {
        let x = *s as f32;
        let bp = b0 * x + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
        x2 = x1;
        x1 = x;
        y2 = y1;
        y1 = bp;
        let abs_bp = bp.abs();
        let coeff = if abs_bp > envelope { attack } else { release };
        envelope = coeff * envelope + (1.0 - coeff) * abs_bp;
        if envelope > threshold {
            let excess = (envelope - threshold) / threshold;
            let gain = 1.0 + excess * (reduction - 1.0);
            let gain = gain.clamp(reduction, 1.0);
            *s = (x * gain).round().clamp(-32768.0, 32767.0) as i16;
        }
    }
}
