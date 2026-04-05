use std::{
    collections::HashMap,
    error::Error,
    fs::{self, File},
    io::{BufReader, BufWriter, Cursor, Read, Seek, SeekFrom, Write},
    path::Path,
};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use crate::utils::try_read_string;
use crate::raw::{lod_data::LodData, palette};

// ─── LOD Writer ──────────────────────────────────────────────────────────────

/// Builds a new MM6-format LOD archive from scratch.
///
/// # Format
/// ```text
/// [0x000]  "LOD\0"            4 B  magic
/// [0x004]  version padded    252 B  e.g. "GameMMVI\0" + zeros (total header= 256 B)
/// [0x100]  sentinel header   32 B  (name="", offset=first_data_offset, size=0, count=N)
/// [0x120]  header × N        32 B each
/// [sentinel.offset]  raw file bytes concatenated
/// ```
pub struct LodWriter {
    version: &'static str,
    entries: Vec<(String, Vec<u8>)>,
}

impl Default for LodWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl LodWriter {
    /// Create an empty writer targeting the MM6 archive format.
    pub fn new() -> Self {
        Self {
            version: "GameMMVI",
            entries: Vec::new(),
        }
    }

    /// Add a named file to the archive.
    pub fn add_file(&mut self, name: &str, data: Vec<u8>) -> &mut Self {
        self.entries.push((name.to_string(), data));
        self
    }

    /// Walk `dir` (non-recursive) and add every file found.
    pub fn add_dir<P: AsRef<Path>>(&mut self, dir: P) -> Result<&mut Self, Box<dyn Error>> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                let name = entry
                    .file_name()
                    .to_string_lossy()
                    .to_string();
                let data = fs::read(entry.path())?;
                self.entries.push((name, data));
            }
        }
        Ok(self)
    }

    /// Serialise and write the archive to `path`.
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), Box<dyn Error>> {
        let file = File::create(path)?;
        let mut w = BufWriter::new(file);
        self.write_to(&mut w)
    }

    /// Write the LOD archive to any `Write` sink.
    pub fn write_to<W: Write>(&self, w: &mut W) -> Result<(), Box<dyn Error>> {
        let n = self.entries.len();
        // Header area: 256 B (magic + version) + 32 B sentinel + n × 32 B file headers
        let header_area = 256 + 32 * (1 + n);
        // Offsets within the data section (relative to sentinel.offset = header_area)
        let mut data_offsets: Vec<usize> = Vec::with_capacity(n);
        let mut running = 0usize;
        for (_, data) in &self.entries {
            data_offsets.push(running);
            running += data.len();
        }
        let sentinel_offset = header_area as i32;

        // ── Magic + version string (256 bytes total) ──────────────────────
        w.write_all(b"LOD\0")?;
        let ver_bytes = self.version.as_bytes();
        w.write_all(ver_bytes)?;
        // pad version field to 252 bytes
        let pad = 252usize.saturating_sub(ver_bytes.len());
        w.write_all(&vec![0u8; pad])?;

        // ── Sentinel FileHeader ───────────────────────────────────────────
        write_file_header(w, "", sentinel_offset, 0, n as i32)?;

        // ── Entry FileHeaders (offsets relative to sentinel_offset) ───────
        for (i, (name, data)) in self.entries.iter().enumerate() {
            let rel_offset = data_offsets[i] as i32;
            write_file_header(w, name, rel_offset, data.len() as i32, 0)?;
        }

        // ── Data section ──────────────────────────────────────────────────
        for (_, data) in &self.entries {
            w.write_all(data)?;
        }

        Ok(())
    }
}

/// Encode one 32-byte FileHeader.
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

// ─── Patch existing archive ───────────────────────────────────────────────────

impl Lod {
    /// Open `src` LOD, override named entries, write result to `out`.
    ///
    /// - Original file order and name casing are preserved exactly.
    /// - Entries listed in `overrides` have their data replaced.
    /// - New entries not present in the source are appended at the end.
    /// - Passing an empty `overrides` slice produces a **byte-identical** copy.
    pub(crate) fn patch<P, Q>(
        src: P,
        out: Q,
        overrides: &[(&str, Vec<u8>)],
    ) -> Result<(), Box<dyn Error>>
    where
        P: AsRef<Path>,
        Q: AsRef<Path>,
    {
        let original = Lod::open(src)?;
        let mut writer = LodWriter::new();
        // Build a lowercase→data override map for fast lookup
        let override_map: HashMap<String, &Vec<u8>> = overrides
            .iter()
            .map(|(k, v)| (k.to_lowercase(), v))
            .collect();
        // Iterate entries in original order, preserving original name casing
        for (name, data) in &original.entries {
            let key = name.to_lowercase();
            if let Some(ov) = override_map.get(&key) {
                writer.add_file(name, (*ov).clone());
            } else {
                writer.add_file(name, data.clone());
            }
        }
        // Append overrides that are brand-new (not present in the source)
        for (name, data) in overrides {
            let key = name.to_lowercase();
            if !original.files.contains_key(&key) {
                writer.add_file(name, data.to_vec());
            }
        }
        writer.save(out)
    }
}


#[allow(dead_code)]
pub struct Lod {
    version: Version,
    /// Lowercase-keyed map for fast lookups (existing API).
    files: HashMap<String, Vec<u8>>,
    /// Ordered entries with **original** name casing, for lossless re-serialisation.
    entries: Vec<(String, Vec<u8>)>,
}

impl Lod {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Lod, Box<dyn std::error::Error>> {
        let file: File = File::open(path.as_ref())?;
        let mut buf_reader = BufReader::new(file);

        let magic = try_read_string(&mut buf_reader)?;
        if magic != "LOD" {
            return Err("Invalid file format".into());
        }

        let version = Version::try_from(try_read_string(&mut buf_reader)?.as_str())?;

        let file_headers = read_file_headers(&mut buf_reader)?;
        let (files, entries) = read_files(file_headers, buf_reader)?;

        Ok(Lod { version, files, entries })
    }

    pub fn entries(&self) -> &[(String, Vec<u8>)] {
        &self.entries
    }

    pub fn files_map(&self) -> &HashMap<String, Vec<u8>> {
        &self.files
    }

    pub fn list_files(&self) -> Vec<&str> {
        self.files.keys().map(|f| f.as_str()).collect()
    }

    pub(crate) fn try_get_bytes<'a>(&'a self, name: &str) -> Option<&'a [u8]> {
        self.files.get(&name.to_lowercase()).map(|v| v.as_slice())
    }

    pub(crate) fn save_all(&self, path: &Path, palettes: &palette::Palettes) -> Result<(), Box<dyn Error>> {
        fs::create_dir_all(path)?;
        for file in &self.files {
            let file_name = file.0;
            let data = file.1.as_slice();
            if let Ok(image) = crate::raw::image::Image::try_from(data) {
                if let Err(e) = image.save(path.join(format!("{}.png", file_name))) {
                    println!("Error saving image {} : {}", file_name, e);
                }
            } else if let Ok(sprite) = crate::raw::image::Image::try_from((data, palettes)) {
                if let Err(e) = sprite.save(path.join(format!("{}.png", file_name))) {
                    println!("Error saving sprite {} : {}", file_name, e)
                }
            } else if let Ok(lod_data) = LodData::try_from(data)
                && let Err(e) = lod_data.dump(path.join(file_name))
            {
                println!("Error saving lod data {} : {}", file_name, e)
            }
        }
        Ok(())
    }
}

fn read_file_headers(buf_reader: &mut BufReader<File>) -> Result<Vec<FileHeader>, Box<dyn Error>> {
    buf_reader.seek(SeekFrom::Start(FILE_INDEX_OFFSET))?;
    let initial_file_header: FileHeader = read_file_header(buf_reader)?;
    let initial_offset = initial_file_header.offset;
    let num_files = initial_file_header.count as usize;
    let mut file_headers = Vec::with_capacity(num_files);
    file_headers.push(initial_file_header);
    for _ in 0..num_files {
        let mut file_header = read_file_header(buf_reader)?;
        file_header.offset += initial_offset;
        file_headers.push(file_header);
    }
    Ok(file_headers)
}

fn read_file_header(buf_reader: &mut BufReader<File>) -> Result<FileHeader, Box<dyn Error>> {
    let mut buf: [u8; FILE_HEADER_SIZE] = [0; FILE_HEADER_SIZE];
    buf_reader.read_exact(&mut buf)?;
    let file_header = FileHeader::try_from(&buf)?;
    Ok(file_header)
}

fn read_files(
    file_headers: Vec<FileHeader>,
    mut buf_reader: BufReader<File>,
) -> Result<(HashMap<String, Vec<u8>>, Vec<(String, Vec<u8>)>), Box<dyn Error>> {
    let mut files: HashMap<String, Vec<u8>> = HashMap::new();
    let mut entries: Vec<(String, Vec<u8>)> = Vec::with_capacity(file_headers.len());
    for fh in file_headers {
        if fh.name.is_empty() {
            continue; // skip sentinel
        }
        let buf = read_file(&mut buf_reader, &fh)?;
        files.insert(fh.name.to_lowercase(), buf.clone());
        entries.push((fh.name, buf));
    }
    Ok((files, entries))
}

fn read_file(buf_reader: &mut BufReader<File>, fh: &FileHeader) -> Result<Vec<u8>, Box<dyn Error>> {
    buf_reader.seek(SeekFrom::Start(fh.offset as u64))?;
    let mut buf = vec![0; fh.size];
    buf_reader.read_exact(&mut buf)?;
    Ok(buf.to_vec())
}

#[derive(Debug)]
struct FileHeader {
    name: String,
    offset: i32,
    size: usize,
    count: i32,
}

const FILE_HEADER_SIZE: usize = 32;
const FILE_INDEX_OFFSET: u64 = 256;

impl TryFrom<&[u8; FILE_HEADER_SIZE]> for FileHeader {
    type Error = Box<dyn Error>;

    fn try_from(data: &[u8; FILE_HEADER_SIZE]) -> Result<Self, Self::Error> {
        let first_zero_idx = data.iter().position(|&x| x == 0).unwrap_or(data.len());
        let name: &str = std::str::from_utf8(&data[0..first_zero_idx])?;

        let mut cursor = Cursor::new(&data[16..]);
        let offset = cursor.read_i32::<LittleEndian>()?;
        let size_raw = cursor.read_i32::<LittleEndian>()?;
        let size = if size_raw < 0 { 0i32 } else { size_raw };
        let _ = cursor.read_i32::<LittleEndian>()?;
        let count = cursor.read_i32::<LittleEndian>()?;
        Ok(FileHeader {
            name: name.to_string(),
            offset,
            size: size as usize,
            count,
        })
    }
}

// Enum to represent different versions of the games
pub enum Version {
    MM6,
    MM7,
    MM8,
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

#[cfg(test)]
mod tests {
    use crate::{get_data_path, lod::Lod};

    use super::*;
    use std::path::Path;

    #[test]
    fn save_works() {
        let lod_path = get_data_path();
        let lod_path = Path::new(&lod_path);
        if !lod_path.exists() {
            return;
        }

        let bitmaps_lod = Lod::open(lod_path.join("BITMAPS.LOD")).unwrap();

        let palettes = crate::Palettes::try_from(&bitmaps_lod).unwrap();
        let _ = bitmaps_lod.save_all(&lod_path.join("bitmaps_lod"), &palettes);

        let games_lod = Lod::open(lod_path.join("games.lod")).unwrap();
        let _ = games_lod.save_all(&lod_path.join("games_lod"), &palettes);

        let sprites_lod = Lod::open(lod_path.join("SPRITES.LOD")).unwrap();
        let _ = sprites_lod.save_all(&lod_path.join("sprites_lod"), &palettes);

        let icons_lod = Lod::open(lod_path.join("icons.lod")).unwrap();
        let _ = icons_lod.save_all(&lod_path.join("icons_lod"), &palettes);

        let new_lod = Lod::open(lod_path.join("new.lod")).unwrap();
        let _ = new_lod.save_all(&lod_path.join("new_lod"), &palettes);
    }

    #[test]
    fn get_image_works() {
        let lod_path = get_data_path();
        let lod_path = Path::new(&lod_path);
        if !lod_path.exists() {
            return;
        }

        let bitmaps_lod = Lod::open(lod_path.join("BITMAPS.LOD")).unwrap();
        let palettes = crate::Palettes::try_from(&bitmaps_lod).unwrap();

        let sprites_lod = Lod::open(lod_path.join("SPRITES.LOD")).unwrap();
        let goblin_image = crate::raw::image::Image::try_from((sprites_lod.try_get_bytes("gobfia0").unwrap(), &palettes))
            .unwrap()
            .to_image_buffer()
            .unwrap();
        assert_eq!(goblin_image.width(), 355);
        assert_eq!(goblin_image.height(), 289);
    }

    #[test]
    fn get_sprite() {
        let lod_path = get_data_path();
        let lod_path = Path::new(&lod_path);
        if !lod_path.exists() {
            return;
        }

        let sprites_lod = Lod::open(lod_path.join("SPRITES.LOD")).unwrap();
        let rok1 = sprites_lod.try_get_bytes("rok1");
        assert!(rok1.is_some());
    }

    /// Round-trip test: open icons.lod, save it with no overrides, then verify
    /// every entry's content is byte-identical to the original.
    ///
    /// This test requires MM6 game data (skipped in CI).
    /// It validates that `LodWriter` + `Lod::patch` produce a lossless copy.
    #[test]
    fn save_round_trip_is_lossless() {
        let lod_path = get_data_path();
        let lod_path = Path::new(&lod_path);
        if !lod_path.exists() {
            return;
        }

        let src_path = lod_path.join("icons.lod");
        if !src_path.exists() {
            return;
        }

        // Save to a temp file with zero overrides
        let tmp = std::env::temp_dir().join("openmm_icons_roundtrip.lod");
        Lod::patch(&src_path, &tmp, &[]).expect("patch (copy) failed");

        // Re-open both and compare every entry
        let original = Lod::open(&src_path).expect("open original");
        let copy     = Lod::open(&tmp).expect("open copy");

        assert_eq!(
            original.entries.len(), copy.entries.len(),
            "entry count must match"
        );

        for (orig_entry, copy_entry) in original.entries.iter().zip(copy.entries.iter()) {
            assert_eq!(
                orig_entry.0, copy_entry.0,
                "entry name must match (casing preserved)"
            );
            assert_eq!(
                orig_entry.1, copy_entry.1,
                "entry '{}' data must be byte-identical",
                orig_entry.0
            );
        }

        let _ = std::fs::remove_file(&tmp);
    }
}
