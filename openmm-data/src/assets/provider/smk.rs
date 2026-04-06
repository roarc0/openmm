//! SMK video support — archive extensions and frame-by-frame decoding.

use crate::assets::provider::archive::Archive;
pub use crate::assets::provider::archive::smk::{SmkArchive, SmkWriter};
pub use crate::assets::smk::{SmkAudioInfo, SmkDecoder, SmkError, SmkInfo, parse_smk_info};

pub trait SmkExt {
    fn smk_bytes(&self, index: usize) -> Option<&[u8]>;
    fn smk_by_name(&self, name: &str) -> Option<Vec<u8>>;
}

impl SmkExt for SmkArchive {
    fn smk_bytes(&self, _index: usize) -> Option<&[u8]> {
        // Legacy support if needed, but Archive trait is preferred.
        None
    }

    fn smk_by_name(&self, name: &str) -> Option<Vec<u8>> {
        self.get_file(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn smk_decoder_reads_3dologo() {
        // This test requires actual game data.
        // We'll skip it if not found, but preserve the logic.
        let data_path = "../../data"; // Likely location from crate root
        let vid_path = Path::new(data_path).join("Anims/Anims2.vid");
        if !vid_path.exists() {
            return;
        }
        let archive = SmkArchive::open(&vid_path).expect("open Anims2.vid");
        let bytes = archive
            .list_files()
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case("3dologo"))
            .and_then(|e| archive.get_file(&e.name))
            .expect("3dologo not found in Anims2.vid");

        let mut dec = SmkDecoder::new(bytes).expect("SmkDecoder::new");
        assert_eq!(dec.width, 640);
        assert_eq!(dec.height, 480);
        assert!(dec.frame_count > 0);

        let frame = dec.next_frame().expect("first frame should exist");
        assert_eq!(frame.len(), (dec.width * dec.height * 4) as usize);
    }
}
