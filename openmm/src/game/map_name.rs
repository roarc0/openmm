use std::fmt;

use super::odm::OdmName;

/// Unified map identifier for outdoor (ODM) and indoor (BLV) maps.
#[derive(Clone, Debug)]
pub enum MapName {
    Outdoor(OdmName),
    Indoor(String), // e.g. "d01", "sewer"
}

impl MapName {
    /// Returns the filename for this map (e.g. "oute3.odm" or "d01.blv").
    pub fn filename(&self) -> String {
        match self {
            MapName::Outdoor(odm) => odm.to_string(),
            MapName::Indoor(name) => format!("{}.blv", name),
        }
    }

    pub fn is_indoor(&self) -> bool {
        matches!(self, MapName::Indoor(_))
    }

    pub fn is_outdoor(&self) -> bool {
        matches!(self, MapName::Outdoor(_))
    }
}

impl fmt::Display for MapName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.filename())
    }
}

impl TryFrom<&str> for MapName {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        // Strip known extensions
        let name = value
            .strip_suffix(".odm")
            .or_else(|| value.strip_suffix(".blv"))
            .unwrap_or(value);

        // If starts with "out" and length is 5 (e.g. "oute3"), parse as outdoor
        if name.len() == 5 && name.starts_with("out") {
            let odm = OdmName::try_from(value)?;
            Ok(MapName::Outdoor(odm))
        } else {
            Ok(MapName::Indoor(name.to_string()))
        }
    }
}
