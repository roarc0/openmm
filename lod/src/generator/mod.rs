//! Code-generation utilities — build LOD data from scratch without MM6 game files.
//!
//! These modules are entirely independent of game data and can be used to
//! produce a fully synthetic test environment renderable by the engine.

/// Deterministic heightmap generator for ODM outdoor maps.
pub mod terrain;

/// Minimal BSP model builder (axis-aligned boxes).
pub mod bsp;

/// OBJ → BSP model importer.
/// Export a mesh from any 3D tool as Wavefront OBJ and use this to
/// produce a `BSPModel` that can be placed in an ODM map.
pub mod obj;

/// Compress data using zlib. Used for generating LOD bitmaps/sprites.
pub fn zlib_compress(data: &[u8]) -> Vec<u8> {
    crate::raw::zlib::compress(data)
}
