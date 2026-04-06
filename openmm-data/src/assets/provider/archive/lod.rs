use std::{
    collections::HashMap,
    error::Error,
    fs::File,
    io::{BufReader, Cursor, Read, Seek, SeekFrom, Write},
    path::Path,
};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use crate::assets::provider::archive::{Archive, ArchiveEntry};

// ── LOD Archive Reader ────────────────────────────────────────────────────────

const FILE_HEADER_SIZE: usize = 32;
const FILE_INDEX_OFFSET: u64 = 256;

#[derive(Debug, Clone)]
pub enum Version {
    MM6,
    MM7,
    MM8,
}

impl Version {
    fn version_str(&self) -> &'static str {
        match self {
            Version::MM6 => "GameMMVI",
            Version::MM7 => "GameMMVII",
            Version::MM8 => "GameMMVIII",
        }
    }
}

impl TryFrom<&str> for Version {
    type Error = &'static str;

    fn try_from(data: &str) -> Result<Self, Self::Error> {
        match data {
            "GameMMVI" | "MMVI" => Ok(Version::MM6),
            "GameMMVII" | "MMVII" => Ok(Version::MM7),
            "GameMMVIII" | "MMVIII" => Ok(Version::MM8),
            _ => Err("Invalid game version"),
        }
    }
}

fn try_read_string(reader: &mut impl Read) -> Result<String, Box<dyn Error>> {
    let mut buf = [0u8; 256];
    let mut i = 0;
    while i < buf.len() {
        let mut byte = [0u8; 1];
        if reader.read(&mut byte)? == 0 {
            break;
        }
        if byte[0] == 0 {
            break;
        }
        buf[i] = byte[0];
        i += 1;
    }
    Ok(String::from_utf8_lossy(&buf[..i]).to_string())
}

#[derive(Debug)]
struct FileHeader {
    name: String,
    offset: i32,
    size: usize,
    count: i32,
}

impl TryFrom<&[u8; FILE_HEADER_SIZE]> for FileHeader {
    type Error = Box<dyn Error>;

    fn try_from(data: &[u8; FILE_HEADER_SIZE]) -> Result<Self, Self::Error> {
        let first_zero_idx = data.iter().position(|&x| x == 0).unwrap_or(data.len());
        let name: &str = std::str::from_utf8(&data[0..first_zero_idx])?;

        let mut cursor = Cursor::new(&data[16..]);
        let offset = cursor.read_i32::<LittleEndian>()?;
        let size_raw = cursor.read_i32::<LittleEndian>()?;
        let size = if size_raw < 0 { 0usize } else { size_raw as usize };
        let _ = cursor.read_i32::<LittleEndian>()?;
        let count = cursor.read_i32::<LittleEndian>()?;
        Ok(FileHeader {
            name: name.to_string(),
            offset,
            size,
            count,
        })
    }
}

/// Read-only structure of an LOD file.
/// Entire file is loaded into RAM at open time; reads are zero-copy slices from that buffer.
pub struct LodArchive {
    data: Vec<u8>,
    pub version: Version,
    entries: Vec<ArchiveEntry>,
    lookup: HashMap<String, usize>,
    _offsets: Vec<usize>, // Matches entries indices perfectly, used internally for offsets
}

impl LodArchive {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn Error>> {
        let data = std::fs::read(path.as_ref())?;
        let mut reader = BufReader::new(Cursor::new(&data));

        let magic = try_read_string(&mut reader)?;
        if magic != "LOD" {
            return Err("Invalid LOD file magic".into());
        }

        let version_str = try_read_string(&mut reader)?;
        let version = Version::try_from(version_str.as_str())?;

        // Read sentinel
        reader.seek(SeekFrom::Start(FILE_INDEX_OFFSET))?;
        let mut buf = [0u8; FILE_HEADER_SIZE];
        reader.read_exact(&mut buf)?;
        let sentinel = FileHeader::try_from(&buf)?;

        let initial_offset = sentinel.offset;
        let num_files = sentinel.count as usize;

        let mut entries = Vec::with_capacity(num_files);
        let mut offsets = Vec::with_capacity(num_files);
        let mut lookup = HashMap::with_capacity(num_files);

        for i in 0..num_files {
            reader.read_exact(&mut buf)?;
            let mut fh = FileHeader::try_from(&buf)?;
            fh.offset += initial_offset;

            // Preserve original case
            let original_name = fh.name.clone();

            entries.push(ArchiveEntry::new(original_name.clone(), fh.size, 0));
            offsets.push(fh.offset as usize);
            // Case-insensitive mapping by default
            lookup.insert(original_name.to_lowercase(), i);
        }

        Ok(Self {
            data,
            version,
            entries,
            lookup,
            _offsets: offsets,
        })
    }

    /// Optional explicitly case-insensitive lookup (e.g. for fallback).
    pub fn get_file_case_insensitive(&self, name: &str) -> Option<Vec<u8>> {
        let lower = name.to_lowercase();
        if let Some(idx) = self.lookup.get(&lower) {
            return self.read_bytes(*idx);
        }
        None
    }

    fn read_bytes(&self, index: usize) -> Option<Vec<u8>> {
        let offset = self._offsets[index];
        let size = self.entries[index].size;
        let end = offset + size;
        if end > self.data.len() {
            log::error!(
                "LOD entry out of bounds: index={} offset={} size={} data_len={}",
                index,
                offset,
                size,
                self.data.len()
            );
            return None;
        }
        Some(self.data[offset..end].to_vec())
    }
}

impl Archive for LodArchive {
    fn list_files(&self) -> &[ArchiveEntry] {
        &self.entries
    }

    fn get_file_raw(&self, name: &str) -> Option<Vec<u8>> {
        if name.is_empty() {
            log::warn!("Attempted to fetch file with empty name from LOD archive");
            return None;
        }
        let lower = name.to_lowercase();
        let idx = self.lookup.get(&lower)?;
        self.read_bytes(*idx)
    }

    fn get_file(&self, name: &str) -> Option<Vec<u8>> {
        let raw = self.get_file_raw(name)?;
        // We do *not* magically decompress automatically unless metadata explicitly flags it.
        // MM6 LOD doesn't have compression flags in the header. The file format itself handles 8-byte/48-byte.
        // Therefore, we just return the raw bytes. Higher level parsers (like OpenMM-Data) will do `LodData::try_from`.
        Some(raw)
    }

    fn contains(&self, name: &str) -> bool {
        self.lookup.contains_key(&name.to_lowercase())
    }
}

// ── LOD Writer ──────────────────────────────────────────────────────────────

/// Builds a new LOD archive.
/// Allows extracting from existing `LodArchive` and applying overrides.
pub struct LodWriter {
    version: Version,
    entries: Vec<(String, Vec<u8>)>,
}

impl LodWriter {
    pub fn new(version: Version) -> Self {
        Self {
            version,
            entries: Vec::new(),
        }
    }

    pub fn add_file(&mut self, name: &str, data: Vec<u8>) -> &mut Self {
        self.entries.push((name.to_string(), data));
        self
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), Box<dyn Error>> {
        let mut file = File::create(path)?;
        self.write_to(&mut file)
    }

    pub fn write_to<W: Write>(&self, w: &mut W) -> Result<(), Box<dyn Error>> {
        let n = self.entries.len();
        let header_area = 256 + 32 * (1 + n);
        let mut data_offsets: Vec<usize> = Vec::with_capacity(n);
        let mut running = 0usize;
        for (_, data) in &self.entries {
            data_offsets.push(running);
            running += data.len();
        }
        let sentinel_offset = header_area as i32;

        w.write_all(b"LOD\0")?;
        let ver_str = self.version.version_str();
        let ver_bytes = ver_str.as_bytes();
        w.write_all(ver_bytes)?;
        let pad = 252usize.saturating_sub(ver_bytes.len());
        w.write_all(&vec![0u8; pad])?;

        write_file_header(w, "", sentinel_offset, 0, n as i32)?;

        for (i, (name, data)) in self.entries.iter().enumerate() {
            let rel_offset = data_offsets[i] as i32;
            write_file_header(w, name, rel_offset, data.len() as i32, 0)?;
        }

        for (_, data) in &self.entries {
            w.write_all(data)?;
        }

        Ok(())
    }

    /// Open `src` LOD, override named entries, write result to `out`.
    pub fn patch<P, Q>(src: P, out: Q, overrides: &[(&str, Vec<u8>)]) -> Result<(), Box<dyn Error>>
    where
        P: AsRef<Path>,
        Q: AsRef<Path>,
    {
        let original = LodArchive::open(src)?;
        let mut writer = LodWriter::new(original.version.clone());

        let override_map: HashMap<String, &Vec<u8>> = overrides
            .iter()
            .map(|(k, v)| (k.to_string(), v)) // Note: Case sensitive!
            .collect();

        for (i, entry) in original.entries.iter().enumerate() {
            if let Some(ov) = override_map.get(&entry.name) {
                writer.add_file(&entry.name, (*ov).clone());
            } else {
                writer.add_file(&entry.name, original.read_bytes(i).unwrap());
            }
        }

        let original_keys: std::collections::HashSet<&String> = original.lookup.keys().collect();

        for (name, data) in overrides {
            if !original_keys.contains(&name.to_lowercase()) {
                writer.add_file(name, data.to_vec());
            }
        }
        writer.save(out)
    }
}

fn write_file_header<W: Write>(
    w: &mut W,
    name: &str,
    offset: i32,
    size: i32,
    count: i32,
) -> Result<(), Box<dyn Error>> {
    let mut name_buf = [0u8; 16];
    let bytes = name.as_bytes();
    let copy_len = bytes.len().min(15);
    name_buf[..copy_len].copy_from_slice(&bytes[..copy_len]);
    w.write_all(&name_buf)?;
    w.write_i32::<LittleEndian>(offset)?;
    w.write_i32::<LittleEndian>(size)?;
    w.write_i32::<LittleEndian>(0)?; // _skip
    w.write_i32::<LittleEndian>(count)?;
    Ok(())
}
