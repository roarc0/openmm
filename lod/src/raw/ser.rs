//! `LodSerialise` — write LOD-parseable bytes from in-memory structures.
//!
//! Each implementation is the inverse of the corresponding `parse()` /
//! `load()` function, enabling round-trip tests and synthetic asset generation.

/// Serialise a parsed LOD structure back to its binary/text wire format.
pub trait LodSerialise {
    fn to_bytes(&self) -> Vec<u8>;
}
