//! Parser for `npcnames.txt` — NPC name pools from the icons LOD.
//!
//! TSV file with 1 header line ("Male\tFemale"), then one name pair per row.
//! Either column may be empty if the lists are different lengths.

use serde::{Deserialize, Serialize};
use std::error::Error;

use crate::Assets;
use crate::LodSerialise;

/// Male and female NPC name pools.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpcNamePools {
    pub male: Vec<String>,
    pub female: Vec<String>,
}

impl NpcNamePools {
    pub fn load(assets: &Assets) -> Result<Self, Box<dyn Error>> {
        let raw = assets.get_bytes("icons/npcnames.txt")?;
        Self::try_from(raw.as_slice())
    }

    /// Pick a name deterministically from the pool using `seed` as the index.
    pub fn name_for(&self, is_female: bool, seed: usize) -> &str {
        let pool = if is_female { &self.female } else { &self.male };
        if pool.is_empty() {
            return "Peasant";
        }
        &pool[seed % pool.len()]
    }

    /// Classify a first name as female (true), male (false), or unknown (None).
    pub fn classify_name(&self, first_name: &str) -> Option<bool> {
        let lower = first_name.to_ascii_lowercase();
        let in_female = self.female.iter().any(|n| n.to_ascii_lowercase() == lower);
        let in_male = self.male.iter().any(|n| n.to_ascii_lowercase() == lower);
        match (in_female, in_male) {
            (true, false) => Some(true),
            (false, true) => Some(false),
            _ => None,
        }
    }

    fn parse(text: &str) -> Self {
        let mut male = Vec::new();
        let mut female = Vec::new();
        for line in text.lines().skip(1) {
            let line = line.trim_end_matches('\r');
            if line.trim().is_empty() {
                continue;
            }
            let mut cols = line.splitn(2, '\t');
            let m = cols.next().unwrap_or("").trim().to_string();
            let f = cols.next().unwrap_or("").trim().to_string();
            if !m.is_empty() {
                male.push(m);
            }
            if !f.is_empty() {
                female.push(f);
            }
        }
        Self { male, female }
    }
}

impl TryFrom<&[u8]> for NpcNamePools {
    type Error = Box<dyn Error>;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        let data = match crate::assets::lod_data::LodData::try_from(data) {
            Ok(d) => d.data,
            Err(_) => data.to_vec(),
        };
        let text = String::from_utf8_lossy(&data);
        Ok(Self::parse(&text))
    }
}

impl LodSerialise for NpcNamePools {
    fn to_bytes(&self) -> Vec<u8> {
        let mut out = String::from("Male\tFemale\r\n");
        let n = self.male.len().max(self.female.len());
        for i in 0..n {
            let m = self.male.get(i).map(|s| s.as_str()).unwrap_or("");
            let f = self.female.get(i).map(|s| s.as_str()).unwrap_or("");
            out.push_str(&format!("{}\t{}\r\n", m, f));
        }
        out.into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::get_data_path;

    fn load() -> Option<NpcNamePools> {
        let assets = crate::Assets::new(get_data_path()).ok()?;
        NpcNamePools::load(&assets).ok()
    }

    #[test]
    fn pools_non_empty() {
        let Some(pools) = load() else { return };
        assert!(!pools.male.is_empty(), "male names empty");
        assert!(!pools.female.is_empty(), "female names empty");
    }

    #[test]
    fn first_male_is_aaron() {
        let Some(pools) = load() else { return };
        assert_eq!(pools.male[0], "Aaron");
    }

    #[test]
    fn name_for_lookup_and_wrap() {
        let data: &[u8] = b"Male\tFemale\nJohn\tJane\nBob\tAlice\n";
        let pool = NpcNamePools::try_from(data).unwrap();
        assert_eq!(pool.name_for(false, 0), "John");
        assert_eq!(pool.name_for(false, 1), "Bob");
        assert_eq!(pool.name_for(true, 0), "Jane");
        assert_eq!(pool.name_for(true, 1), "Alice");
        assert_eq!(pool.name_for(false, 2), "John"); // wraps
    }

    #[test]
    fn classify_name_case_insensitive() {
        let data: &[u8] = b"Male\tFemale\nJohn\tJane\nBob\tAlice\n";
        let pool = NpcNamePools::try_from(data).unwrap();
        assert_eq!(pool.classify_name("John"), Some(false));
        assert_eq!(pool.classify_name("Jane"), Some(true));
        assert_eq!(pool.classify_name("Unknown"), None);
        assert_eq!(pool.classify_name("JOHN"), Some(false));
    }

    #[test]
    fn name_for_empty_pool_returns_fallback() {
        let data: &[u8] = b"Male\tFemale\n";
        let pool = NpcNamePools::try_from(data).unwrap();
        assert_eq!(pool.name_for(false, 0), "Peasant");
        assert_eq!(pool.name_for(true, 0), "Peasant");
    }
}
