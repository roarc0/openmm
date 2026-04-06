/// Metadata for a single file contained within an archive.
#[derive(Debug, Clone)]
pub struct ArchiveEntry {
    /// Original case-preserved filename from the archive.
    pub name: String,
    /// Size in bytes of the file content in the archive (potentially compressed).
    pub size: usize,
    /// Uncompressed size, if known/stored (0 if unknown or uncompressed).
    pub decompressed_size: usize,
}

impl ArchiveEntry {
    pub fn new(name: String, size: usize, decompressed_size: usize) -> Self {
        Self {
            name,
            size,
            decompressed_size,
        }
    }
}

/// A common interface for read-only access to an archive.
pub trait Archive {
    /// Return the list of files in their original stored order.
    fn list_files(&self) -> &[ArchiveEntry];

    /// Retrieve decompressed file bytes if supported, or raw bytes otherwise.
    fn get_file(&self, name: &str) -> Option<Vec<u8>>;

    /// Retrieve raw, unaltered bytes exactly as stored in the archive.
    fn get_file_raw(&self, name: &str) -> Option<Vec<u8>>;

    /// Fast check if a file exists (should be exact match / case-sensitive optionally as defined by implementation).
    fn contains(&self, name: &str) -> bool;
}

pub mod lod;
pub mod smk;
pub mod snd;
pub mod zlib;
