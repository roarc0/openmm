/// Parser for MM6 VID archives (Anims/Anims1.vid, Anims/Anims2.vid).
///
/// Format:
///   u32 num_files
///   entries[num_files]:
///     name:   [u8; 40]  null-terminated, zero-padded (e.g. "ArmMid\0smk\0...")
///     offset: u32 LE    byte offset into the VID file where the SMK data starts
///
/// Size of each embedded file = next_offset - this_offset (last entry: eof - offset).
///
/// Each embedded file is a Smacker video (SMK2/SMK4 magic).
use std::error::Error;
use std::path::Path;

const ENTRY_NAME_LEN: usize = 40;
const ENTRY_LEN: usize = ENTRY_NAME_LEN + 4; // name + u32 offset

/// A single entry in a VID archive.
#[derive(Debug, Clone)]
pub struct VidEntry {
    /// File name without extension (e.g. "ArmMid").
    pub name: String,
    /// Byte offset of the SMK data within the VID file.
    pub offset: usize,
    /// Size in bytes of the SMK data.
    pub size: usize,
}

/// Parsed VID archive: index of all embedded SMK files.
pub struct Vid {
    data: Vec<u8>,
    pub entries: Vec<VidEntry>,
}

impl Vid {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn Error>> {
        let data = std::fs::read(path)?;
        let entries = parse_entries(&data)?;
        Ok(Self { data, entries })
    }

    /// Return the raw SMK bytes for an entry by index.
    pub fn smk_bytes(&self, index: usize) -> &[u8] {
        let e = &self.entries[index];
        &self.data[e.offset..e.offset + e.size]
    }

    /// Return the raw SMK bytes for a named entry (case-insensitive).
    pub fn smk_by_name(&self, name: &str) -> Option<&[u8]> {
        let idx = self.entries.iter().position(|e| e.name.eq_ignore_ascii_case(name))?;
        Some(self.smk_bytes(idx))
    }
}

// ── VidWriter ───────────────────────────────────────────────────────────────

/// Builder for .vid archives. Use this to pack individual SMK files into a
/// single game-compatible video library.
#[derive(Default)]
pub struct VidWriter {
    entries: Vec<(String, Vec<u8>)>,
}

impl VidWriter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an SMK file to the archive. `name` should be the filename without
    /// extension, max 39 characters.
    pub fn add(&mut self, name: &str, data: Vec<u8>) {
        self.entries.push((name.to_string(), data));
    }

    pub fn save<P: AsRef<Path>>(&self, path: &P) -> Result<(), Box<dyn Error>> {
        let mut buf = Vec::new();

        let count = self.entries.len() as u32;
        buf.extend_from_slice(&count.to_le_bytes());

        let mut offset = 4 + count * (40 + 4);

        for (name, data) in &self.entries {
            let mut name_buf = [0u8; 40];
            let name_bytes = name.as_bytes();
            let n = name_bytes.len().min(39);
            name_buf[..n].copy_from_slice(&name_bytes[..n]);
            buf.extend_from_slice(&name_buf);
            buf.extend_from_slice(&(offset as u32).to_le_bytes());
            offset += data.len() as u32;
        }

        for (_, data) in &self.entries {
            buf.extend_from_slice(data);
        }

        std::fs::write(path, buf)?;
        Ok(())
    }
}

fn parse_entries(data: &[u8]) -> Result<Vec<VidEntry>, Box<dyn Error>> {
    if data.len() < 4 {
        return Err("VID file too short".into());
    }
    let num_files = u32::from_le_bytes(data[0..4].try_into()?) as usize;
    let table_end = 4 + num_files * ENTRY_LEN;
    if data.len() < table_end {
        return Err(format!("VID table truncated: need {table_end}, have {}", data.len()).into());
    }

    let mut entries = Vec::with_capacity(num_files);
    for i in 0..num_files {
        let base = 4 + i * ENTRY_LEN;
        let name_bytes = &data[base..base + ENTRY_NAME_LEN];
        let null = name_bytes.iter().position(|&b| b == 0).unwrap_or(ENTRY_NAME_LEN);
        let name = String::from_utf8_lossy(&name_bytes[..null]).into_owned();
        let offset = u32::from_le_bytes(data[base + ENTRY_NAME_LEN..base + ENTRY_LEN].try_into()?) as usize;
        entries.push(VidEntry { name, offset, size: 0 });
    }

    // Compute sizes from offset differences.
    for i in 0..entries.len() {
        let next = if i + 1 < entries.len() {
            entries[i + 1].offset
        } else {
            data.len()
        };
        entries[i].size = next.saturating_sub(entries[i].offset);
    }

    Ok(entries)
}

/// Basic SMK header info extracted without a full decoder.
#[derive(Debug)]
pub struct SmkInfo {
    pub magic: [u8; 4],
    pub width: u32,
    pub height: u32,
    pub frames: u32,
    /// Raw frame_rate field. Positive = ms/frame, negative = fps, 0 = default (10 fps).
    pub frame_rate: i32,
}

impl SmkInfo {
    pub fn fps(&self) -> f32 {
        match self.frame_rate {
            // Positive: milliseconds per frame.
            // Negative: 1/100th-millisecond units per frame (100_000 units = 1 second).
            0 => 10.0,
            r if r > 0 => 1000.0 / r as f32,
            r => 100_000.0 / (-r) as f32,
        }
    }
}

pub fn parse_smk_info(smk: &[u8]) -> Option<SmkInfo> {
    if smk.len() < 24 {
        return None;
    }
    let magic: [u8; 4] = smk[0..4].try_into().ok()?;
    if &magic != b"SMK2" && &magic != b"SMK4" {
        return None;
    }
    let width = u32::from_le_bytes(smk[4..8].try_into().ok()?);
    let height = u32::from_le_bytes(smk[8..12].try_into().ok()?);
    let frames = u32::from_le_bytes(smk[12..16].try_into().ok()?);
    let frame_rate = i32::from_le_bytes(smk[16..20].try_into().ok()?);
    Some(SmkInfo {
        magic,
        width,
        height,
        frames,
        frame_rate,
    })
}
