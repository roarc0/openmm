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

    pub fn get(&self, name: &str) -> Option<Vec<u8>> {
        let entry = self.entries.get(&name.to_lowercase())?;
        let start = entry.offset as usize;
        let end = start + entry.size as usize;
        if end > self.data.len() {
            return None;
        }
        let raw = &self.data[start..end];

        if entry.decompressed_size > 0 {
            crate::zlib::decompress(raw, entry.size as usize, entry.decompressed_size as usize)
                .ok()
        } else {
            Some(raw.to_vec())
        }
    }

    pub fn list(&self) -> Vec<&str> {
        self.entries.keys().map(|s| s.as_str()).collect()
    }

    pub fn exists(&self, name: &str) -> bool {
        self.entries.contains_key(&name.to_lowercase())
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
