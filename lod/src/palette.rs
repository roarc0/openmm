use std::collections::HashMap;
use std::error::Error;

use super::Lod;

const PALETTE_HEADER_SIZE: usize = 48;
const PALETTE_SIZE: usize = 768;
const PALETTE_DATA_SIZE: usize = PALETTE_SIZE + PALETTE_HEADER_SIZE;

#[allow(dead_code)]
#[derive(Debug, Eq, PartialEq, Hash)]
pub struct Palette {
    pub data: [u8; PALETTE_SIZE],
}

impl TryFrom<&[u8]> for Palette {
    type Error = Box<dyn Error>;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        if data.len() != PALETTE_DATA_SIZE {
            return Err("Malformed palette, expected size is 768B".into());
        }
        let palette = &data[PALETTE_HEADER_SIZE..];
        Ok(Self {
            data: palette.try_into()?,
        })
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct Palettes {
    palettes: HashMap<u16, Palette>,
}

impl TryFrom<&Lod> for Palettes {
    type Error = Box<dyn Error>;

    fn try_from(lod: &Lod) -> Result<Self, Self::Error> {
        let palette_files: Vec<_> = lod
            .files()
            .iter()
            .filter_map(|f| {
                if f.to_lowercase().starts_with("pal") && f.len() == 6 {
                    Some(f.to_string())
                } else {
                    None
                }
            })
            .collect();

        let mut palettes: HashMap<u16, Palette> = HashMap::new();
        for file_name in palette_files {
            let palette = Palette::try_from(lod.try_get_bytes(&file_name).ok_or("expected file")?)?;
            let id = extract_palette_id(&file_name)?;
            palettes.insert(id, palette);
        }
        Ok(Palettes { palettes })
    }
}

impl Palettes {
    pub fn get(&self, id: u16) -> Option<&Palette> {
        self.palettes.get(&id)
    }
}

fn extract_palette_id(s: &str) -> Result<u16, Box<dyn Error>> {
    if s.len() < 3 {
        return Err("String is too short to contain a palette id".into());
    }
    let num_str = &s[s.len() - 3..];
    match num_str.parse::<u16>() {
        Ok(num) => Ok(num),
        _ => Err("Invalid u16 value".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_try_from_wrong_size_errors() {
        let short_data = vec![0u8; 100];
        let result = Palette::try_from(short_data.as_slice());
        assert!(result.is_err());
    }

    #[test]
    fn palette_try_from_correct_size_ok() {
        let data = vec![0u8; PALETTE_DATA_SIZE];
        let palette = Palette::try_from(data.as_slice()).unwrap();
        assert_eq!(palette.data.len(), PALETTE_SIZE);
        assert_eq!(palette.data, [0u8; PALETTE_SIZE]);
    }

    #[test]
    fn palette_try_from_skips_header() {
        let mut data = vec![0u8; PALETTE_DATA_SIZE];
        // Set bytes in the header (should be skipped)
        for i in 0..PALETTE_HEADER_SIZE {
            data[i] = 0xFF;
        }
        // Set first palette color to a known value
        data[PALETTE_HEADER_SIZE] = 42;
        let palette = Palette::try_from(data.as_slice()).unwrap();
        assert_eq!(palette.data[0], 42);
    }

    #[test]
    fn extract_palette_id_valid() {
        assert_eq!(extract_palette_id("PAL123").unwrap(), 123);
        assert_eq!(extract_palette_id("pal001").unwrap(), 1);
        assert_eq!(extract_palette_id("pal000").unwrap(), 0);
        assert_eq!(extract_palette_id("x999").unwrap(), 999);
    }

    #[test]
    fn extract_palette_id_too_short_errors() {
        assert!(extract_palette_id("P1").is_err());
        assert!(extract_palette_id("").is_err());
    }

    #[test]
    fn extract_palette_id_non_numeric_suffix_errors() {
        assert!(extract_palette_id("palXYZ").is_err());
        assert!(extract_palette_id("palABC").is_err());
    }
}
