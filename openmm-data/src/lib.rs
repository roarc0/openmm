use std::env;

pub use crate::assets::provider::archive::Archive;

/// Serialise a parsed LOD structure back to its binary/text wire format.
pub trait LodSerialise {
    fn to_bytes(&self) -> Vec<u8>;
}

pub mod assets;
pub use assets::provider::Assets;
pub use assets::*;


pub mod generator;
pub mod utils;
pub use utils::find_path_case_insensitive;

pub const ENV_OPENMM_6_PATH: &str = "OPENMM_6_PATH";

pub fn get_data_path() -> String {
    if let Ok(p) = env::var(ENV_OPENMM_6_PATH) {
        format!("{}/data", p)
    } else {
        String::from("data/mm6/data")
    }
}
