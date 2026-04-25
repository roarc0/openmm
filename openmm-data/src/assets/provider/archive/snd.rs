use std::{
    collections::HashMap,
    error::Error,
    fs,
    io::{Cursor, Read, Write},
    path::Path,
};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use crate::assets::provider::archive::{Archive, ArchiveEntry};

pub fn try_read_string(data: &[u8]) -> Option<String> {
    let len = data.iter().position(|&x| x == 0).unwrap_or(data.len());
    if len == 0 {
        return None;
    }
    Some(String::from_utf8_lossy(&data[..len]).to_string())
}

#[derive(Clone, Debug)]
pub struct SndArchive {
    entries: Vec<ArchiveEntry>,
    lookup: HashMap<String, usize>,
    data: Vec<u8>,
    _offsets: Vec<usize>,
}

impl SndArchive {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn Error>> {
        let data = fs::read(path)?;
        let mut cursor = Cursor::new(&data);
        let entry_count = cursor.read_u32::<LittleEndian>()?;

        // Robustness: Detect entry size by looking at the first file's offset.
        // Standard MM6/7 formats use 48, 52, or 56 bytes per entry after the 4-byte count.
        let mut entry_size = 52; // Default
        if entry_count > 0 {
            let mut temp_cursor = Cursor::new(&data[4..]);
            let mut _name = [0u8; 40];
            temp_cursor.read_exact(&mut _name)?;
            let first_offset = temp_cursor.read_u32::<LittleEndian>()? as usize;
            if first_offset > 4 && (first_offset - 4).is_multiple_of(entry_count as usize) {
                let detected = (first_offset - 4) / entry_count as usize;
                if detected == 48 || detected == 52 || detected == 56 || detected == 64 {
                    entry_size = detected;
                }
            }
        }

        let mut entries = Vec::with_capacity(entry_count as usize);
        let mut lookup = HashMap::with_capacity(entry_count as usize);
        let mut _offsets = Vec::with_capacity(entry_count as usize);

        for i in 0..entry_count {
            let entry_start = 4 + i as usize * entry_size;
            cursor.set_position(entry_start as u64);

            let mut name_buf = [0u8; 40];
            cursor.read_exact(&mut name_buf)?;
            let name = try_read_string(&name_buf[..]).unwrap_or_default();

            let offset = cursor.read_u32::<LittleEndian>()? as usize;
            let size = cursor.read_u32::<LittleEndian>()? as usize;
            let decomp_size = if entry_size >= 52 {
                cursor.read_u32::<LittleEndian>()? as usize
            } else {
                0
            };

            if !name.is_empty() {
                entries.push(ArchiveEntry {
                    name: name.clone(),
                    size,
                    decompressed_size: decomp_size,
                    name_tail: [0; 4],
                });
                _offsets.push(offset);
                lookup.insert(name.to_lowercase(), entries.len() - 1);
            }
        }

        log::info!(
            "SND Archive opened: {} entries, entry_size={}B",
            entries.len(),
            entry_size
        );

        let archive = Self {
            entries,
            lookup,
            data,
            _offsets,
        };

        archive.check_gaps();

        Ok(archive)
    }

    fn check_gaps(&self) {
        let mut ranges = Vec::new();
        // Archive length field
        ranges.push((0, 4));

        // Index range
        let entry_count = self.entries.len();
        if entry_count > 0 {
            let first_offset = self._offsets.first().cloned().unwrap_or(0);
            ranges.push((4, first_offset));
        }

        for (idx, entry) in self.entries.iter().enumerate() {
            let start = self._offsets[idx];
            let end = start + entry.size;
            ranges.push((start, end));
        }

        // Sort by start
        ranges.sort_by_key(|r| r.0);

        // Merge and find gaps
        let mut current_pos = 0;
        for (start, end) in ranges {
            if start > current_pos {
                let gap_size = start - current_pos;
                let gap_data = &self.data[current_pos..start];

                // Scan gap for music headers
                let magics = [b"MThd", b"XMID", b"XDIR", b"RMID"];
                for magic in magics {
                    if let Some(pos) = gap_data.windows(magic.len()).position(|window| window == magic) {
                        log::warn!(
                            "MUSIC SIGNATURE '{:?}' FOUND IN SND GAP AT 0x{:x}!",
                            String::from_utf8_lossy(magic),
                            current_pos + pos
                        );
                    }
                }

                let preview = if gap_size >= 16 {
                    format!("{:02x?}", &gap_data[0..16])
                } else {
                    format!("{:02x?}", gap_data)
                };

                log::info!(
                    "SND Gap found: 0x{:x} to 0x{:x} (size={}B) data={}",
                    current_pos,
                    start,
                    gap_size,
                    preview
                );
            }
            current_pos = current_pos.max(end);
        }

        if current_pos < self.data.len() {
            let gap_size = self.data.len() - current_pos;
            log::info!(
                "SND Trailing Gap found: 0x{:x} to 0x{:x} (size={}B)",
                current_pos,
                self.data.len(),
                gap_size
            );
        }
    }
}

impl Archive for SndArchive {
    fn list_files(&self) -> &[ArchiveEntry] {
        &self.entries
    }

    fn get_file_raw(&self, name: &str) -> Option<Vec<u8>> {
        if name.is_empty() {
            return None;
        }
        let lower = name.to_lowercase();
        let idx = self.lookup.get(&lower)?;
        let start = self._offsets[*idx];
        let size = self.entries[*idx].size;

        if start + size <= self.data.len() {
            Some(self.data[start..start + size].to_vec())
        } else {
            log::error!(
                "SND Archive read out of bounds: name='{}' offset=0x{:x} + size={} > file_len={}",
                name,
                start,
                size,
                self.data.len()
            );
            None
        }
    }

    fn get_file(&self, name: &str) -> Option<Vec<u8>> {
        let raw = self.get_file_raw(name)?;
        let idx = self.lookup.get(&name.to_lowercase()).unwrap(); // Safe because get_file_raw check
        let entry = &self.entries[*idx];

        let mut data = raw;

        // Robustness: only decompress if it actually looks like a zlib stream.
        // Some MM6 entries have decompressed_size != size but are NOT zlib (e.g. 29_02).
        if entry.decompressed_size > 0 && entry.decompressed_size != entry.size {
            let matches_zlib =
                data.starts_with(&[0x78, 0x9c]) || data.starts_with(&[0x78, 0x01]) || data.starts_with(&[0x78, 0xda]);

            if matches_zlib {
                match super::zlib::decompress(&data, entry.size, entry.decompressed_size) {
                    Ok(decompressed) => {
                        data = decompressed;
                    }
                    Err(e) => {
                        log::warn!("SND Zlib decompression failed for {}: {}. Returning raw data.", name, e);
                    }
                }
            } else {
                log::info!(
                    "SND detected non-zlib mismatch for {}: size={}B decomp={}B data={:02x?}. Attempting raw deflate.",
                    name,
                    entry.size,
                    entry.decompressed_size,
                    &data[..data.len().min(8)]
                );

                // Fallback: try raw deflate (no zlib header)
                if let Ok(decompressed) = decompress_deflate(&data, entry.decompressed_size) {
                    data = decompressed;
                    log::info!("SND Raw Deflate success for {}", name);
                }
            }
        }

        // Robustness: If the file is a RIFF WAV but seems truncated (only header),
        // try to read the full size from the archive if the archive data allows it.
        if data.len() == 44 && data.starts_with(b"RIFF") {
            let riff_size = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;
            let full_size = riff_size + 8;
            if full_size > data.len() {
                let offset = self._offsets[*idx];
                if offset + full_size <= self.data.len() {
                    log::info!(
                        "SND detected truncated RIFF for {}: header says {}B, reading more from archive.",
                        name,
                        full_size
                    );
                    data = self.data[offset..offset + full_size].to_vec();
                } else {
                    log::warn!(
                        "SND truncated RIFF for {}: header says {}B but exceeds archive bounds.",
                        name,
                        full_size
                    );
                }
            }
        }

        Some(data)
    }

    fn contains(&self, name: &str) -> bool {
        self.lookup.contains_key(&name.to_lowercase())
    }
}

fn decompress_deflate(data: &[u8], reserve_size: usize) -> Result<Vec<u8>, Box<dyn Error>> {
    use flate2::bufread::DeflateDecoder;
    use std::io::Read;
    let mut z = DeflateDecoder::new(Cursor::new(data));
    let mut buf = Vec::with_capacity(reserve_size);
    z.read_to_end(&mut buf)?;
    Ok(buf)
}

// ── SndWriter ───────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct SndWriter {
    entries: Vec<(String, Vec<u8>, usize)>, // name, data, decompressed_size
}

impl SndWriter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, name: &str, data: Vec<u8>, decompressed_size: usize) {
        self.entries.push((name.to_string(), data, decompressed_size));
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), Box<dyn Error>> {
        let mut buf = Vec::new();
        let mut cursor = Cursor::new(&mut buf);

        let count = self.entries.len() as u32;
        cursor.write_u32::<LittleEndian>(count)?;

        let mut offset = 4 + count as usize * (40 + 4 + 4 + 4);

        for (name, data, decomp_size) in &self.entries {
            let mut name_buf = [0u8; 40];
            let name_bytes = name.as_bytes();
            let n = name_bytes.len().min(39);
            name_buf[..n].copy_from_slice(&name_bytes[..n]);
            cursor.write_all(&name_buf)?;
            cursor.write_u32::<LittleEndian>(offset as u32)?;
            cursor.write_u32::<LittleEndian>(data.len() as u32)?;
            cursor.write_u32::<LittleEndian>(*decomp_size as u32)?;
            offset += data.len();
        }

        for (_, data, _) in &self.entries {
            cursor.write_all(data)?;
        }

        fs::write(path, buf)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{SndArchive, SndWriter};
    use crate::assets::provider::archive::Archive;

    /// Tiny valid PCM WAV (silence) for round-trip tests.
    fn tiny_pcm_wav() -> Vec<u8> {
        let samples: [i16; 2] = [0, 0];
        let pcm_data_size = (samples.len() * 2) as u32;
        let channels: u16 = 1;
        let sample_rate: u32 = 8000;
        let byte_rate = sample_rate * channels as u32 * 2;
        let block_align = channels * 2;
        let file_size = 36 + pcm_data_size;
        let mut out = Vec::new();
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

    #[test]
    fn snd_equal_compressed_and_decompressed_size_is_raw_wav() {
        let wav = tiny_pcm_wav();
        let path = std::env::temp_dir().join(format!("openmm_snd_eq_{:?}.snd", std::thread::current().id()));
        let _ = std::fs::remove_file(&path);
        let mut w = SndWriter::new();
        w.add("TestEqSize", wav.clone(), wav.len());
        w.save(&path).expect("save snd");
        let arch = SndArchive::open(&path).expect("open snd");
        let got = arch
            .get_file("TestEqSize")
            .expect("get_file must succeed when size == decompressed_size (raw storage)");
        let _ = std::fs::remove_file(&path);
        assert_eq!(got, wav);
    }

    #[test]
    fn snd_riff_header_only_in_index_reads_full_data() {
        let wav = tiny_pcm_wav();
        let path = std::env::temp_dir().join(format!("openmm_snd_riff_{:?}.snd", std::thread::current().id()));
        let _ = std::fs::remove_file(&path);

        // Write the full wav to the archive data area,
        // but tell the index it's only 44 bytes (the header).
        let mut w = SndWriter::new();
        w.add("ShortIndex", wav.clone(), 44);
        w.save(&path).expect("save snd");

        let arch = SndArchive::open(&path).expect("open snd");
        let got = arch.get_file("ShortIndex").expect("must return data");

        let _ = std::fs::remove_file(&path);
        assert_eq!(
            got.len(),
            wav.len(),
            "Extraction should have read beyond the 44B index size using RIFF header"
        );
        assert_eq!(got, wav);
    }
}
