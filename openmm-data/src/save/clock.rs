//! Raw passthrough for `clock.bin` (40 bytes) inside an MM6 save LOD.
//!
//! No fields parsed for gameplay — just preserves raw bytes for round-trip.

/// Size of `clock.bin` in bytes.
pub const CLOCK_SIZE: usize = 40;

/// Raw clock data from an MM6 save file. Opaque passthrough.
#[derive(Debug, Clone)]
pub struct SaveClock {
    raw: Vec<u8>,
}

impl SaveClock {
    /// Parse `clock.bin` blob. Pads or truncates to [`CLOCK_SIZE`].
    pub fn parse(data: &[u8]) -> Self {
        let mut raw = vec![0u8; CLOCK_SIZE];
        let len = data.len().min(CLOCK_SIZE);
        raw[..len].copy_from_slice(&data[..len]);
        Self { raw }
    }

    /// Serialize back to raw bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        self.raw.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_roundtrip() {
        let save = crate::save::file::SaveFile::open("../data/mm6/data/new.lod").expect("failed to open new.lod");
        let data = save.get_file("clock.bin").expect("clock.bin missing");

        let clock = SaveClock::parse(&data);
        let bytes = clock.to_bytes();
        assert_eq!(bytes.len(), CLOCK_SIZE);
        assert_eq!(
            &bytes[..data.len().min(CLOCK_SIZE)],
            &data[..data.len().min(CLOCK_SIZE)]
        );
    }

    #[test]
    fn parse_short_input() {
        let clock = SaveClock::parse(&[1, 2, 3]);
        let bytes = clock.to_bytes();
        assert_eq!(bytes.len(), CLOCK_SIZE);
        assert_eq!(&bytes[..3], &[1, 2, 3]);
        assert!(bytes[3..].iter().all(|&b| b == 0));
    }
}
