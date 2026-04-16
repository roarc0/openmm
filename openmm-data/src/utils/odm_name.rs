use serde::{Deserialize, Serialize};
use std::fmt::Display;

/// Grid coordinate for outdoor maps. Columns a-e, rows 1-3.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct OdmName {
    pub x: char,
    pub y: char,
}

impl Default for OdmName {
    fn default() -> Self {
        Self { x: 'e', y: '3' }
    }
}

impl Display for OdmName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(Self::map_name(self.x, self.y).as_str())
    }
}

impl TryFrom<&str> for OdmName {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let x = value.as_bytes().get(3).copied().ok_or("invalid map name")? as char;
        let y = value.as_bytes().get(4).copied().ok_or("invalid map name")? as char;

        let x = Self::validate_x(x).ok_or("invalid map x coordinate")?;
        let y = Self::validate_y(y).ok_or("invalid map y coordinate")?;

        Ok(Self { x, y })
    }
}

impl OdmName {
    pub fn go_north(&self) -> Option<OdmName> {
        let y = Self::validate_y((self.y as u8 - 1) as char)?;
        Some(Self { x: self.x, y })
    }

    pub fn go_west(&self) -> Option<OdmName> {
        let x = Self::validate_x((self.x as u8 - 1) as char)?;
        Some(Self { x, y: self.y })
    }

    pub fn go_south(&self) -> Option<OdmName> {
        let y = Self::validate_y((self.y as u8 + 1) as char)?;
        Some(Self { x: self.x, y })
    }

    pub fn go_east(&self) -> Option<OdmName> {
        let x = Self::validate_x((self.x as u8 + 1) as char)?;
        Some(Self { x, y: self.y })
    }

    /// Iterate over all valid outdoor map coordinates (a1..e3).
    pub fn all() -> impl Iterator<Item = OdmName> {
        ('a'..='e').flat_map(|x| ('1'..='3').map(move |y| OdmName { x, y }))
    }

    /// Base name without extension (e.g. `"oute3"`).
    pub fn base_name(&self) -> String {
        format!("out{}{}", self.x, self.y)
    }

    fn map_name(x: char, y: char) -> String {
        format!("out{}{}.odm", x, y)
    }

    fn validate_x(c: char) -> Option<char> {
        match c {
            'a'..='e' => Some(c),
            _ => None,
        }
    }
    fn validate_y(c: char) -> Option<char> {
        match c {
            '1'..='3' => Some(c),
            _ => None,
        }
    }
}
