use byteorder::{LittleEndian, ReadBytesExt};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::error::Error;
use std::fs::{self, File};
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::path::Path;

pub mod image;
pub mod odm;
pub mod palette;
pub mod raw;
pub mod raw_unpacked;
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
        let mut file: File = File::open(path)?;

        let magic = read_until_zero_byte::<std::io::Error>(&mut file)?;
        let magic = String::from_utf8_lossy(&magic);
        if magic != "LOD" {
            return Err("Invalid file format".into());
        }

        let version =
            Version::try_from(read_until_zero_byte::<std::io::Error>(&mut file)?.as_slice())?;

        let file_headers = read_file_headers(&mut file)?;
        let files = read_files(file_headers, file)?;

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

    pub fn dump(&self, path: &Path, palettes: &palette::Palettes) {
        fs::create_dir_all(path).unwrap();
        for file_name in self.files() {
            match self.get::<raw::Raw>(file_name) {
                Ok(raw) => {
                    let data = raw.data;
                    if let Ok(image) = image::Image::try_from(data) {
                        if let Err(e) = image.dump(path.join(format!("{}.png", file_name))) {
                            println!("Error saving image {} : {}", file_name, e);
                        }
                    } else if let Ok(sprite) = image::Image::try_from((data, palettes)) {
                        if let Err(e) = sprite.dump(path.join(format!("{}.png", file_name))) {
                            println!("Error saving sprite {} : {}", file_name, e)
                        }
                    } else if let Ok(odm) = odm::Odm::try_from(data) {
                        if let Err(e) = odm.dump(path.join(file_name)) {
                            println!("Error saving odm {} : {}", file_name, e)
                        }
                    } else if let Ok(raw_unpacked) = raw_unpacked::RawUnpacked::try_from(data) {
                        if let Err(e) = raw_unpacked.dump(path.join(file_name)) {
                            println!("Error saving raw_unpacked {} : {}", file_name, e)
                        }
                    } else if let Err(e) = raw.dump(path.join(file_name)) {
                        println!("Error saving raw {} : {}", file_name, e);
                    }
                }
                Err(e) => println!("Error extracting file {} : {}", file_name, e),
            }
        }
    }
}

fn read_file_headers(file: &mut File) -> Result<Vec<FileHeader>, Box<dyn Error>> {
    file.seek(SeekFrom::Start(FILE_INDEX_OFFSET))?;
    let initial_file_header: FileHeader = read_file_header(file)?;
    let initial_offset = initial_file_header.offset;
    let num_files = initial_file_header.count as usize;
    let mut file_headers = Vec::with_capacity(num_files);
    file_headers.push(initial_file_header);
    for _ in 0..num_files {
        let mut file_header = read_file_header(file)?;
        file_header.offset += initial_offset;
        file_headers.push(file_header);
    }
    Ok(file_headers)
}

fn read_file_header(file: &mut File) -> Result<FileHeader, Box<dyn Error>> {
    let mut buf: [u8; FILE_HEADER_SIZE] = [0; FILE_HEADER_SIZE];
    file.read_exact(&mut buf)?;
    let file_header = FileHeader::try_from(&buf)?;
    Ok(file_header)
}

fn read_files(
    file_headers: Vec<FileHeader>,
    mut file: File,
) -> Result<HashMap<String, Vec<u8>>, Box<dyn Error>> {
    let mut files: HashMap<String, Vec<u8>> = HashMap::new();
    for fh in file_headers {
        let buf = read_file(&mut file, &fh)?;
        files.insert(fh.name, buf);
    }
    Ok(files)
}

fn read_file(file: &mut File, fh: &FileHeader) -> Result<Vec<u8>, Box<dyn Error>> {
    file.seek(SeekFrom::Start(fh.offset as u64))?;
    let mut buf = Vec::new();
    buf.resize(fh.size, 0);
    file.read_exact(&mut buf)?;
    Ok(buf)
}

#[allow(dead_code)]
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

impl TryFrom<&[u8]> for Version {
    type Error = &'static str;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        match data {
            b"GameMMVI" | b"MMVI" => Ok(Version::MM6),
            b"GameMMVII" | b"MMVII" => Ok(Version::MM7),
            b"GameMMVIII" | b"MMVIII" => Ok(Version::MM8),
            _ => Err("Invalid game version"),
        }
    }
}

fn read_until_zero_byte<E>(r: &mut dyn Read) -> Result<Vec<u8>, E>
where
    E: From<std::io::Error>,
{
    let mut buffer = Vec::new();
    while let Some(byte) = r.bytes().next() {
        let byte = byte?;
        if byte == 0 {
            break;
        }
        buffer.push(byte);
    }
    Ok(buffer)
}
