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

        let mut entries = Vec::with_capacity(entry_count as usize);
        let mut lookup = HashMap::with_capacity(entry_count as usize);
        let mut _offsets = Vec::with_capacity(entry_count as usize);

        for _ in 0..entry_count {
            let mut name_buf = [0u8; 40];
            cursor.read_exact(&mut name_buf)?;
            let name = try_read_string(&name_buf[..]).unwrap_or_default();
            let offset = cursor.read_u32::<LittleEndian>()? as usize;
            let size = cursor.read_u32::<LittleEndian>()? as usize;
            let decompressed_size = cursor.read_u32::<LittleEndian>()? as usize;

            if !name.is_empty() {
                entries.push(ArchiveEntry {
                    name: name.clone(),
                    size,
                    decompressed_size,
                });
                _offsets.push(offset);
                // Robust case-insensitive mapping: store lowercased key
                lookup.insert(name.to_lowercase(), entries.len() - 1);
            }
        }

        Ok(Self {
            entries,
            lookup,
            data,
            _offsets,
        })
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
        let idx = self.lookup.get(&lower).or_else(|| {
            log::warn!("Sound file not found in archive: '{}' (requested as '{}')", name, lower);
            None
        })?;
        let start = self._offsets[*idx];
        let size = self.entries[*idx].size;

        if start + size <= self.data.len() {
            Some(self.data[start..start + size].to_vec())
        } else {
            log::error!("Sound file data out of bounds: '{}' (idx={})", name, idx);
            None
        }
    }

    fn get_file(&self, name: &str) -> Option<Vec<u8>> {
        let raw = self.get_file_raw(name)?;
        let idx = self.lookup.get(&name.to_lowercase()).unwrap(); // Safe because get_file_raw check
        let entry = &self.entries[*idx];

        if entry.decompressed_size > 0 {
            // Apply zlib decompression since we know it's a zlib compressed payload natively in the archive format
            super::zlib::decompress(&raw, entry.size, entry.decompressed_size).ok()
        } else {
            Some(raw)
        }
    }

    fn contains(&self, name: &str) -> bool {
        self.lookup.contains_key(&name.to_lowercase())
    }
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
