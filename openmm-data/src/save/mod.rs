//! MM6 native save-file parsers (header, party, character, clock).
//!
//! Each sub-module owns one binary chunk from a `.mm6` LOD save archive
//! and provides parse + round-trip serialization.

pub mod character;
pub mod header;
// pub mod party;     // Task 2
// pub mod clock;     // Task 4
// pub mod file;      // Task 4
