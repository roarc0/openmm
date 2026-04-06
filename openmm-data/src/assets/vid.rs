use openmm_archive::Archive;
pub use openmm_archive::vid::*;

pub use crate::assets::smk::{SmkInfo, parse_smk_info};

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
