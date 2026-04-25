//! Parser for `header.bin` (100 bytes) inside an MM6 save LOD.
//!
//! Layout:
//! - 0x00..0x14: save_name  (20 bytes, null-terminated ASCII)
//! - 0x14..0x28: map_name   (20 bytes, null-terminated ASCII)
//! - 0x28..0x30: playing_time (i64, game ticks -- 128 ticks/real second)
//! - 0x30..0x64: padding    (52 bytes, zeroed)

/// Total size of header.bin in bytes.
pub const HEADER_SIZE: usize = 100;

const SAVE_NAME_OFFSET: usize = 0x00;
const SAVE_NAME_LEN: usize = 20;
const MAP_NAME_OFFSET: usize = 0x14;
const MAP_NAME_LEN: usize = 20;
const PLAYING_TIME_OFFSET: usize = 0x28;

/// Parsed `header.bin` from an MM6 save file.
///
/// Keeps a raw byte copy for lossless round-trip: parsed fields are patched
/// back into the raw buffer on `to_bytes()`.
#[derive(Debug, Clone)]
pub struct SaveHeader {
    /// Player-visible save name (bytes 0-19, null-terminated). Empty in dev saves.
    pub save_name: String,
    /// Current map filename, e.g. `"oute3.odm"` (bytes 20-39, null-terminated).
    pub map_name: String,
    /// Elapsed game ticks (128 ticks per real second).
    pub playing_time: i64,
    /// Raw 100-byte buffer for round-trip fidelity.
    raw: [u8; HEADER_SIZE],
}

impl Default for SaveHeader {
    fn default() -> Self {
        Self {
            save_name: String::new(),
            map_name: String::new(),
            playing_time: 0,
            raw: [0u8; HEADER_SIZE],
        }
    }
}

impl SaveHeader {
    /// Parse a `header.bin` blob. Panics if `data` is shorter than [`HEADER_SIZE`].
    pub fn parse(data: &[u8]) -> Self {
        assert!(
            data.len() >= HEADER_SIZE,
            "header.bin too short: {} bytes, need {}",
            data.len(),
            HEADER_SIZE,
        );

        let mut raw = [0u8; HEADER_SIZE];
        raw.copy_from_slice(&data[..HEADER_SIZE]);

        let save_name = read_fixed_str(&raw[SAVE_NAME_OFFSET..SAVE_NAME_OFFSET + SAVE_NAME_LEN]);
        let map_name = read_fixed_str(&raw[MAP_NAME_OFFSET..MAP_NAME_OFFSET + MAP_NAME_LEN]);
        let playing_time = i64::from_le_bytes(
            raw[PLAYING_TIME_OFFSET..PLAYING_TIME_OFFSET + 8]
                .try_into()
                .unwrap(),
        );

        Self {
            save_name,
            map_name,
            playing_time,
            raw,
        }
    }

    /// Serialize back to 100 bytes, patching parsed fields into the raw copy.
    pub fn to_bytes(&self) -> [u8; HEADER_SIZE] {
        let mut out = self.raw;
        write_fixed_str(&mut out[SAVE_NAME_OFFSET..SAVE_NAME_OFFSET + SAVE_NAME_LEN], &self.save_name);
        write_fixed_str(&mut out[MAP_NAME_OFFSET..MAP_NAME_OFFSET + MAP_NAME_LEN], &self.map_name);
        out[PLAYING_TIME_OFFSET..PLAYING_TIME_OFFSET + 8]
            .copy_from_slice(&self.playing_time.to_le_bytes());
        out
    }

    /// Map stem without extension, e.g. `"oute3"` from `"oute3.odm"`.
    pub fn map_stem(&self) -> &str {
        self.map_name
            .rsplit_once('.')
            .map(|(stem, _)| stem)
            .unwrap_or(&self.map_name)
    }
}

/// Read a null-terminated string from a fixed-size buffer.
fn read_fixed_str(buf: &[u8]) -> String {
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).into_owned()
}

/// Write a string into a fixed-size buffer, null-padding the remainder.
fn write_fixed_str(buf: &mut [u8], s: &str) {
    let bytes = s.as_bytes();
    let len = bytes.len().min(buf.len());
    buf[..len].copy_from_slice(&bytes[..len]);
    // zero-fill the rest
    for b in &mut buf[len..] {
        *b = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_new_lod_header() {
        let save = crate::assets::save::SaveFile::open("../data/mm6/data/new.lod")
            .expect("failed to open new.lod");
        let data = save.get_file("header.bin").expect("header.bin missing");
        assert_eq!(data.len(), HEADER_SIZE);

        let header = SaveHeader::parse(&data);
        assert_eq!(header.save_name, "");
        assert_eq!(header.map_name, "oute3.odm");
        assert_eq!(header.map_stem(), "oute3");
    }

    #[test]
    fn round_trip() {
        let save = crate::assets::save::SaveFile::open("../data/mm6/data/new.lod")
            .expect("failed to open new.lod");
        let data = save.get_file("header.bin").expect("header.bin missing");
        let header = SaveHeader::parse(&data);

        let bytes = header.to_bytes();
        assert_eq!(&bytes[..], &data[..HEADER_SIZE], "round-trip must be lossless");
    }

    #[test]
    fn round_trip_after_mutation() {
        let save = crate::assets::save::SaveFile::open("../data/mm6/data/new.lod")
            .expect("failed to open new.lod");
        let data = save.get_file("header.bin").expect("header.bin missing");
        let mut header = SaveHeader::parse(&data);

        header.save_name = "My Save".to_string();
        header.map_name = "outb2.odm".to_string();
        header.playing_time = 12345;

        let bytes = header.to_bytes();
        let reparsed = SaveHeader::parse(&bytes);
        assert_eq!(reparsed.save_name, "My Save");
        assert_eq!(reparsed.map_name, "outb2.odm");
        assert_eq!(reparsed.map_stem(), "outb2");
        assert_eq!(reparsed.playing_time, 12345);
    }

    #[test]
    fn map_stem_no_extension() {
        let mut header = SaveHeader::default();
        header.map_name = "noext".to_string();
        assert_eq!(header.map_stem(), "noext");
    }

    #[test]
    #[should_panic(expected = "header.bin too short")]
    fn parse_too_short() {
        SaveHeader::parse(&[0u8; 50]);
    }
}
