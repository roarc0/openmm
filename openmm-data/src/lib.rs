use std::env;
use std::path::{Path, PathBuf};

/// Serialise a parsed LOD structure back to its binary/text wire format.
pub trait LodSerialise {
    fn to_bytes(&self) -> Vec<u8>;
}

pub mod assets;
pub use assets::provider::Assets;
pub use assets::*;

pub mod generator;
pub mod save;
pub mod utils;
pub use utils::find_path_case_insensitive;

pub const ENV_OPENMM_PATH_MM6: &str = "OPENMM_PATH_MM6";

fn has_mm6_lods(path: &Path) -> bool {
    ["games.lod", "Games.lod", "GAMES.LOD"]
        .iter()
        .any(|name| path.join(name).exists())
}

pub fn get_data_path() -> String {
    if let Ok(p) = env::var(ENV_OPENMM_PATH_MM6) {
        let env_path = PathBuf::from(p);
        if has_mm6_lods(&env_path) {
            return env_path.to_string_lossy().into_owned();
        }

        let with_data = env_path.join("data");
        if has_mm6_lods(&with_data) {
            return with_data.to_string_lossy().into_owned();
        }

        // Keep legacy behavior as a last resort for callers that pass a game root path.
        return with_data.to_string_lossy().into_owned();
    }

    // Try common layouts from current working directory first.
    for rel in ["mm6/data", "data/mm6/data", "../data/mm6/data"] {
        let candidate = Path::new(rel);
        if has_mm6_lods(candidate) {
            return rel.to_string();
        }
    }

    // Then try paths relative to the crate manifest directory.
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    for rel in ["mm6/data", "../data/mm6/data"] {
        let candidate = manifest_dir.join(rel);
        if has_mm6_lods(&candidate) {
            return candidate.to_string_lossy().into_owned();
        }
    }

    // Final fallback keeps previous default for callers that prepare this layout later.
    String::from("data/mm6/data")
}
