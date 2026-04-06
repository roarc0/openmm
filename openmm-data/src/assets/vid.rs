use openmm_archive::Archive;
pub use openmm_archive::vid::*;

pub type Vid = VidArchive;

pub trait VidExt {
    fn smk_bytes(&self, index: usize) -> Option<&[u8]>;
    fn smk_by_name(&self, name: &str) -> Option<Vec<u8>>;
}

impl VidExt for VidArchive {
    fn smk_bytes(&self, _index: usize) -> Option<&[u8]> {
        // Warning: This implies full loads or slices against some struct.
        // We altered VidArchive to not provide a native `[u8]` slice trivially,
        // since `get_file_raw` returns `Vec<u8>`. For legacy `smk_bytes` usage,
        // we probably shouldn't return `&[u8]` directly unless it's static or self-owned.
        // Returning `Vec<u8>` is safer, or we use `get_file_raw(&name.to_string())`.
        None
    }

    fn smk_by_name(&self, name: &str) -> Option<Vec<u8>> {
        self.get_file(name) // Archive trait implementation already supports this!
    }
}

// ── Decoder tools preserved for data ──

/// Basic SMK header info extracted without a full decoder.
#[derive(Debug)]
pub struct SmkInfo {
    pub magic: [u8; 4],
    pub width: u32,
    pub height: u32,
    pub frames: u32,
    pub frame_rate: i32,
}

impl SmkInfo {
    pub fn fps(&self) -> f32 {
        match self.frame_rate {
            0 => 10.0,
            r if r > 0 => 1000.0 / r as f32,
            r => 100_000.0 / (-r) as f32,
        }
    }
}

pub fn parse_smk_info(smk: &[u8]) -> Option<SmkInfo> {
    if smk.len() < 24 {
        return None;
    }
    let magic: [u8; 4] = smk[0..4].try_into().ok()?;
    if &magic != b"SMK2" && &magic != b"SMK4" {
        return None;
    }
    let width = u32::from_le_bytes(smk[4..8].try_into().ok()?);
    let height = u32::from_le_bytes(smk[8..12].try_into().ok()?);
    let frames = u32::from_le_bytes(smk[12..16].try_into().ok()?);
    let frame_rate = i32::from_le_bytes(smk[16..20].try_into().ok()?);
    Some(SmkInfo {
        magic,
        width,
        height,
        frames,
        frame_rate,
    })
}
