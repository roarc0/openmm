use std::collections::HashMap;
use std::error::Error;
use crate::Assets;

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

impl TryFrom<&Assets> for Palettes {
    type Error = Box<dyn Error>;

    fn try_from(assets: &Assets) -> Result<Self, Self::Error> {
        let palette_files: Vec<_> = assets
            .files_in("bitmaps")
            .ok_or("No bitmaps archive found")?
            .into_iter()
            .filter(|f| f.to_lowercase().starts_with("pal") && f.len() == 6)
            .collect();

        let mut palettes: HashMap<u16, Palette> = HashMap::new();
        for file_name in palette_files {
            let path = format!("bitmaps/{}", file_name);
            let bytes = assets.get_bytes(&path)?;
            let palette = Palette::try_from(bytes.as_slice())?;
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
#[path = "palette_tests.rs"]
mod tests;
