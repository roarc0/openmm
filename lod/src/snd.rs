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

    // Build PCM WAV
    let pcm_data_size = (pcm_samples.len() * 2) as u32;
    let channels_u16 = channels as u16;
    let byte_rate = sample_rate * channels as u32 * 2;
    let block_align_pcm = channels_u16 * 2;
    let file_size = 36 + pcm_data_size;

    let mut out = Vec::with_capacity(44 + pcm_data_size as usize);
    // RIFF header
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&file_size.to_le_bytes());
    out.extend_from_slice(b"WAVE");
    // fmt chunk
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM format
    out.extend_from_slice(&channels_u16.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align_pcm.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    // data chunk
    out.extend_from_slice(b"data");
    out.extend_from_slice(&pcm_data_size.to_le_bytes());
    for sample in &pcm_samples {
        out.extend_from_slice(&sample.to_le_bytes());
    }

    Some(out)
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
