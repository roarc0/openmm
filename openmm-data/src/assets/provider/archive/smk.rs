use std::{collections::HashMap, error::Error, fs, path::Path};

use crate::assets::provider::archive::{Archive, ArchiveEntry};

const ENTRY_NAME_LEN: usize = 40;
const ENTRY_LEN: usize = ENTRY_NAME_LEN + 4; // name + u32 offset

/// An archive format used for Smacker videos (historically .vid).
pub struct SmkArchive {
    data: Vec<u8>,
    entries: Vec<ArchiveEntry>,
    lookup: HashMap<String, usize>,
    _offsets: Vec<usize>,
}

impl SmkArchive {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn Error>> {
        let data = fs::read(path)?;
        if data.len() < 4 {
            return Err("SMK archive too short".into());
        }

        let num_files = u32::from_le_bytes(data[0..4].try_into()?) as usize;
        let table_end = 4 + num_files * ENTRY_LEN;
        if data.len() < table_end {
            return Err(format!("SMK table truncated: need {table_end}, have {}", data.len()).into());
        }

        let mut entries = Vec::with_capacity(num_files);
        let mut lookup = HashMap::with_capacity(num_files);
        let mut offsets = Vec::with_capacity(num_files);

        for i in 0..num_files {
            let base = 4 + i * ENTRY_LEN;
            let name_bytes = &data[base..base + ENTRY_NAME_LEN];
            let null = name_bytes.iter().position(|&b| b == 0).unwrap_or(ENTRY_NAME_LEN);
            let name = String::from_utf8_lossy(&name_bytes[..null]).into_owned();
            let offset = u32::from_le_bytes(data[base + ENTRY_NAME_LEN..base + ENTRY_LEN].try_into()?) as usize;

            entries.push(ArchiveEntry {
                name: name.clone(),
                size: 0, // Calculated correctly next step
                decompressed_size: 0,
            });
            offsets.push(offset);
        }

        // Compute sizes and verify SMK magic headers
        for i in 0..num_files {
            let next = if i + 1 < num_files { offsets[i + 1] } else { data.len() };
            let start = offsets[i];
            let size = next.saturating_sub(start);
            entries[i].size = size;

            if size > 0 && start + 3 <= data.len() {
                let magic = &data[start..start + 3];
                if magic != b"SMK" {
                    let preview = if start + 16 <= data.len() {
                        format!("{:02x?}", &data[start..start + 16])
                    } else {
                        "N/A".to_string()
                    };
                    log::warn!(
                        "SMK magic mismatch for '{}' (idx={}) at 0x{:x}: expected 'SMK', found {:?} (data={})",
                        entries[i].name,
                        i,
                        start,
                        String::from_utf8_lossy(magic),
                        preview
                    );
                }
            }

            // Case-insensitive mapping BY DEFAULT
            lookup.insert(entries[i].name.to_lowercase(), i);
        }

        Ok(Self {
            data,
            entries,
            lookup,
            _offsets: offsets,
        })
    }
}

impl Archive for SmkArchive {
    fn list_files(&self) -> &[ArchiveEntry] {
        &self.entries
    }

    fn get_file_raw(&self, name: &str) -> Option<Vec<u8>> {
        let lower = name.to_lowercase();
        let idx = self.lookup.get(&lower).or_else(|| {
            log::warn!("SMK file not found in archive: '{}' (requested as '{}')", name, lower);
            None
        })?;
        let start = self._offsets[*idx];
        let size = self.entries[*idx].size;

        if start + size <= self.data.len() {
            Some(self.data[start..start + size].to_vec())
        } else {
            log::error!("SMK file data out of bounds: '{}' (idx={})", name, idx);
            None
        }
    }

    fn get_file(&self, name: &str) -> Option<Vec<u8>> {
        self.get_file_raw(name)
    }

    fn contains(&self, name: &str) -> bool {
        self.lookup.contains_key(&name.to_lowercase())
    }
}

// ── SmkWriter ───────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct SmkWriter {
    entries: Vec<(String, Vec<u8>)>,
}

impl SmkWriter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, name: &str, data: Vec<u8>) {
        self.entries.push((name.to_string(), data));
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), Box<dyn Error>> {
        let mut buf = Vec::new();

        let count = self.entries.len() as u32;
        buf.extend_from_slice(&count.to_le_bytes());

        let mut offset = 4 + count as usize * (40 + 4);

        for (name, data) in &self.entries {
            let mut name_buf = [0u8; 40];
            let name_bytes = name.as_bytes();
            let n = name_bytes.len().min(39);
            name_buf[..n].copy_from_slice(&name_bytes[..n]);
            buf.extend_from_slice(&name_buf);
            buf.extend_from_slice(&(offset as u32).to_le_bytes());
            offset += data.len();
        }

        for (_, data) in &self.entries {
            buf.extend_from_slice(data);
        }

        fs::write(path, buf)?;
        Ok(())
    }
}
