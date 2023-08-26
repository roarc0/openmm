use byteorder::{LittleEndian, ReadBytesExt};
use lod_data::LodData;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::env;
use std::error::Error;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Cursor, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

pub mod dtile;
pub mod image;
pub mod lod_data;
pub mod odm;
pub mod palette;
mod zlib;

pub const ENV_OMM_LOD_PATH: &str = "OMM_LOD_PATH";
pub const ENV_OMM_DUMP_PATH: &str = "OMM_DUMP_PATH";

#[allow(dead_code)]
pub struct Lod {
    version: Version,
    files: HashMap<String, Vec<u8>>,
}

impl Lod {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Lod, Box<dyn std::error::Error>> {
        let file: File = File::open(path)?;
        let mut buf_reader = BufReader::new(file);

        let magic = read_string(&mut buf_reader)?;
        if magic != "LOD" {
            return Err("Invalid file format".into());
        }

        let version = Version::try_from(read_string(&mut buf_reader)?.as_str())?;

        let file_headers = read_file_headers(&mut buf_reader)?;
        let files = read_files(file_headers, buf_reader)?;

        Ok(Lod { version, files })
    }

    pub fn files(&self) -> Vec<&str> {
        self.files.keys().map(|f| f.as_str()).collect()
    }

    pub fn try_get_bytes<'a>(&'a self, name: &str) -> Option<&'a [u8]> {
        self.files.get(name).map(|v| v.as_slice())
    }

    pub fn save_all(
        &self,
        path: &Path,
        palettes: &palette::Palettes,
    ) -> Result<(), Box<dyn Error>> {
        fs::create_dir_all(path)?;
        for file in &self.files {
            let file_name = file.0;
            let data = file.1.as_slice();
            if let Ok(image) = image::Image::try_from(data) {
                if let Err(e) = image.save(path.join(format!("{}.png", file_name))) {
                    println!("Error saving image {} : {}", file_name, e);
                }
            } else if let Ok(sprite) = image::Image::try_from((data, palettes)) {
                if let Err(e) = sprite.save(path.join(format!("{}.png", file_name))) {
                    println!("Error saving sprite {} : {}", file_name, e)
                }
            } else if let Ok(lod_data) = LodData::try_from(data) {
                if let Err(e) = lod_data.dump(path.join(file_name)) {
                    println!("Error saving lod data {} : {}", file_name, e)
                }
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
) -> Result<HashMap<String, Vec<u8>>, Box<dyn Error>> {
    let mut files: HashMap<String, Vec<u8>> = HashMap::new();
    for fh in file_headers {
        let buf = read_file(&mut buf_reader, &fh)?;
        files.insert(fh.name.to_ascii_lowercase(), buf);
    }
    Ok(files)
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
        let size = cursor.read_i32::<LittleEndian>()?;
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

fn read_string<R>(r: &mut R) -> Result<String, Box<dyn Error>>
where
    R: Read + BufRead,
{
    let mut buffer = Vec::new();
    let _ = r.read_until(b'\0', &mut buffer);
    _ = buffer.pop();
    Ok(String::from_utf8(buffer)?)
}

#[allow(dead_code)]
pub struct LodManager {
    lods: HashMap<String, Lod>,
}

impl LodManager {
    pub fn new<P>(path: P) -> Result<Self, Box<dyn Error>>
    where
        P: AsRef<Path>,
    {
        let lod_files = Self::list_lod_files(path)?;
        let lod_map = Self::create_lod_file_map(lod_files)?;
        Ok(LodManager { lods: lod_map })
    }

    fn list_lod_files<P>(path: P) -> Result<Vec<PathBuf>, std::io::Error>
    where
        P: AsRef<Path>,
    {
        let mut lod_files = Vec::new();
        let entries = fs::read_dir(&path)?;

        for entry in entries {
            let entry = entry?;
            let file_name = entry.file_name();
            if let Some(name) = file_name.to_str() {
                if name.to_lowercase().ends_with(".lod") {
                    lod_files.push(Path::join(path.as_ref(), name));
                }
            }
        }

        Ok(lod_files)
    }

    fn create_lod_file_map(
        lod_files: Vec<PathBuf>,
    ) -> Result<HashMap<String, Lod>, Box<dyn Error>> {
        let mut lod_file_map: HashMap<String, Lod> = HashMap::new();

        for path in lod_files.iter() {
            let lod = Lod::open(path)?;
            let key = path
                .file_stem()
                .ok_or("file should have a .lod extension")?
                .to_string_lossy()
                .to_ascii_lowercase();
            lod_file_map.insert(key, lod);
        }

        Ok(lod_file_map)
    }

    pub fn try_get_bytes<P: AsRef<Path>>(&self, path: P) -> Result<&[u8], Box<dyn Error>> {
        let lod_file: String = path
            .as_ref()
            .parent()
            .ok_or("invalid path")?
            .to_string_lossy()
            .to_string();
        let lod = self.lods.get(&lod_file).ok_or("lod file not found")?;
        let lod_entry: String = path
            .as_ref()
            .file_name()
            .ok_or("invalid lod entry")?
            .to_string_lossy()
            .to_string();
        let lod_data = lod
            .try_get_bytes(&lod_entry)
            .ok_or("unable to open lod entry")?;
        Ok(lod_data)
    }
}

pub fn get_lod_path() -> String {
    env::var(ENV_OMM_LOD_PATH).unwrap_or("./target/mm6/data".into())
}

pub fn get_dump_path() -> String {
    env::var(ENV_OMM_DUMP_PATH).unwrap_or("./target/assets".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn lod_manager_works() {
        let lod_path = get_lod_path();
        let lod_manager = LodManager::new(lod_path).unwrap();
        let grastyl = lod_manager.try_get_bytes("bitmaps/grastyl");
        assert_eq!(17676, grastyl.unwrap().len());
    }

    #[test]
    fn save_works() {
        let lod_path = get_lod_path();
        let lod_path = Path::new(&lod_path);
        let dump_path = get_dump_path();
        let dump_path = Path::new(&dump_path);

        let bitmaps_lod = Lod::open(lod_path.join("BITMAPS.LOD")).unwrap();

        let palettes = palette::Palettes::try_from(&bitmaps_lod).unwrap();
        let _ = bitmaps_lod.save_all(&dump_path.join("bitmaps_lod"), &palettes);

        let games_lod = Lod::open(lod_path.join("games.lod")).unwrap();
        let _ = games_lod.save_all(&dump_path.join("games_lod"), &palettes);

        let sprites_lod = Lod::open(lod_path.join("SPRITES.LOD")).unwrap();
        let _ = sprites_lod.save_all(&dump_path.join("sprites_lod"), &palettes);

        let icons_lod = Lod::open(lod_path.join("icons.lod")).unwrap();
        let _ = icons_lod.save_all(&dump_path.join("icons_lod"), &palettes);

        let new_lod = Lod::open(lod_path.join("new.lod")).unwrap();
        let _ = new_lod.save_all(&dump_path.join("new_lod"), &palettes);
    }

    #[test]
    fn get_image_works() {
        let lod_path = get_lod_path();
        let lod_path = Path::new(&lod_path);

        let bitmaps_lod = Lod::open(lod_path.join("BITMAPS.LOD")).unwrap();
        let palettes = palette::Palettes::try_from(&bitmaps_lod).unwrap();

        let sprites_lod = Lod::open(lod_path.join("SPRITES.LOD")).unwrap();
        let goblin_image =
            image::Image::try_from((sprites_lod.try_get_bytes("gobfia0").unwrap(), &palettes))
                .unwrap()
                .to_image_buffer()
                .unwrap();
        assert_eq!(goblin_image.width(), 355);
        assert_eq!(goblin_image.height(), 289);
    }
}
