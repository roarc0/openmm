use std::collections::HashMap;
use std::error::Error;

use crate::lod;

const PALETTE_HEADER_SIZE: usize = 48;
const PALETTE_SIZE: usize = 768 + 48;

#[allow(dead_code)]
#[derive(Debug)]
pub struct Palette {
    pub data: [u8; 768],
}

impl TryFrom<Vec<u8>> for Palette {
    type Error = Box<dyn Error>;

    fn try_from(data: Vec<u8>) -> Result<Self, Self::Error> {
        Self::try_from(data.as_slice())
    }
}

impl TryFrom<&[u8]> for Palette {
    type Error = Box<dyn Error>;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        if data.len() != PALETTE_SIZE {
            return Err("malformed palette".into());
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
    pub map: HashMap<String, Palette>,
}

impl Palettes {
    pub fn get_palettes(lod: &lod::Lod) -> Result<Palettes, Box<dyn Error>> {
        let palette_files: Vec<_> = lod
            .files()
            .iter()
            .filter_map(|f| {
                if f.to_ascii_lowercase().starts_with("pal") && f.len() == 6 {
                    Some(f.to_string())
                } else {
                    None
                }
            })
            .collect();

        let mut palettes = HashMap::new();
        for pf in palette_files {
            let palette = lod.get::<Palette>(&pf)?;
            palettes.insert(pf.to_ascii_lowercase(), palette);
        }

        Ok(Palettes { map: palettes })
    }
}
