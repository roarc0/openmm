use byteorder::{LittleEndian, ReadBytesExt};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::error::Error;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Cursor, Read, Seek, SeekFrom};
use std::path::Path;

pub mod image;
pub mod odm;
pub mod palette;
pub mod raw;
mod zlib;

#[allow(dead_code)]
pub struct Lod {
    pub version: Version,
    files: HashMap<String, Vec<u8>>,
}

impl Lod {
    pub fn open<P>(path: P) -> Result<Lod, Box<dyn std::error::Error>>
    where
        P: AsRef<Path>,
    {
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

    pub fn get<'a, T: TryFrom<&'a [u8], Error = Box<dyn Error>>>(
        &'a self,
        name: &str,
    ) -> Result<T, Box<dyn Error>> {
        T::try_from(
            self.get_raw(name)
                .ok_or_else(|| "Entry not found".to_string())?,
        )
    }

    pub fn get_raw<'a>(&'a self, name: &str) -> Option<&'a [u8]> {
        self.files.get(name).map(|v| v.as_slice())
    }

    pub fn save(&self, path: &Path, palettes: &palette::Palettes) -> Result<(), Box<dyn Error>> {
        fs::create_dir_all(path)?;
        for file_name in self.files() {
            let data = self
                .get_raw(file_name)
                .ok_or_else(|| format!("Error reading file {}", file_name))?;
            if let Ok(image) = image::Image::try_from(data) {
                if let Err(e) = image.save(path.join(format!("{}.png", file_name))) {
                    println!("Error saving image {} : {}", file_name, e);
                }
            } else if let Ok(sprite) = image::Image::try_from((data, palettes)) {
                if let Err(e) = sprite.save(path.join(format!("{}.png", file_name))) {
                    println!("Error saving sprite {} : {}", file_name, e)
                }
            } else if let Ok(raw) = raw::Raw::try_from(data) {
                if let Err(e) = raw.dump(path.join(file_name)) {
                    println!("Error saving raw {} : {}", file_name, e)
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
        files.insert(fh.name, buf);
    }
    Ok(files)
}

fn read_file(buf_reader: &mut BufReader<File>, fh: &FileHeader) -> Result<Vec<u8>, Box<dyn Error>> {
    buf_reader.seek(SeekFrom::Start(fh.offset as u64))?;
    let mut buf = Vec::new();
    buf.resize(fh.size, 0);
    buf_reader.read_exact(&mut buf)?;
    Ok(buf)
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
