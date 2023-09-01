use std::{
    collections::HashMap,
    error::Error,
    fs::{self, File},
    io::{BufReader, Cursor, Read, Seek, SeekFrom},
    path::Path,
};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::{lod_data::LodData, palette, utils::try_read_string};

#[allow(dead_code)]
pub(super) struct Lod {
    version: Version,
    files: HashMap<String, Vec<u8>>,
}

impl Lod {
    pub(super) fn open<P: AsRef<Path>>(path: P) -> Result<Lod, Box<dyn std::error::Error>> {
        let file: File = File::open(path)?;
        let mut buf_reader = BufReader::new(file);

        let magic = try_read_string(&mut buf_reader)?;
        if magic != "LOD" {
            return Err("Invalid file format".into());
        }

        let version = Version::try_from(try_read_string(&mut buf_reader)?.as_str())?;

        let file_headers = read_file_headers(&mut buf_reader)?;
        let files = read_files(file_headers, buf_reader)?;

        Ok(Lod { version, files })
    }

    pub(super) fn files(&self) -> Vec<&str> {
        self.files.keys().map(|f| f.as_str()).collect()
    }

    pub(super) fn try_get_bytes<'a>(&'a self, name: &str) -> Option<&'a [u8]> {
        self.files.get(name).map(|v| v.as_slice())
    }

    fn save_all(&self, path: &Path, palettes: &palette::Palettes) -> Result<(), Box<dyn Error>> {
        fs::create_dir_all(path)?;
        for file in &self.files {
            let file_name = file.0;
            let data = file.1.as_slice();
            if let Ok(image) = crate::image::Image::try_from(data) {
                if let Err(e) = image.save(path.join(format!("{}.png", file_name))) {
                    println!("Error saving image {} : {}", file_name, e);
                }
            } else if let Ok(sprite) = crate::image::Image::try_from((data, palettes)) {
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
        files.insert(fh.name.to_lowercase(), buf);
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
enum Version {
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
    use crate::{get_lod_path, lod::Lod};

    use super::*;
    use std::path::Path;

    #[test]
    fn save_works() {
        let lod_path = get_lod_path();
        let lod_path = Path::new(&lod_path);

        let bitmaps_lod = Lod::open(lod_path.join("BITMAPS.LOD")).unwrap();

        let palettes = palette::Palettes::try_from(&bitmaps_lod).unwrap();
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
        let lod_path = get_lod_path();
        let lod_path = Path::new(&lod_path);

        let bitmaps_lod = Lod::open(lod_path.join("BITMAPS.LOD")).unwrap();
        let palettes = palette::Palettes::try_from(&bitmaps_lod).unwrap();

        let sprites_lod = Lod::open(lod_path.join("SPRITES.LOD")).unwrap();
        let goblin_image = crate::image::Image::try_from((
            sprites_lod.try_get_bytes("gobfia0").unwrap(),
            &palettes,
        ))
        .unwrap()
        .to_image_buffer()
        .unwrap();
        assert_eq!(goblin_image.width(), 355);
        assert_eq!(goblin_image.height(), 289);
    }

    #[test]
    fn get_sprite() {
        let lod_path = get_lod_path();
        let lod_path = Path::new(&lod_path);

        let sprites_lod = Lod::open(lod_path.join("SPRITES.LOD")).unwrap();
        let rock01 = sprites_lod.try_get_bytes("rock01");
        assert!(rock01.is_some());
    }
}
