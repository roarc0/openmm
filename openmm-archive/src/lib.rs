pub mod zlib;
pub mod lod;
pub mod snd;
pub mod vid;



/// High-level classification of what an archive file might contain.
/// This enum is for heuristic tagging and deliberately does not enforce rigid semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ArchiveFileType {
    Bitmap,
    Palette,
    Sprite,
    Text,
    Binary,
    Sound,
    Video,
    SaveData,
    Event,
    #[default]
    Unknown,
}

impl ArchiveFileType {
    /// Basic heuristic classification based on file name or extension.
    pub fn from_name(name: &str) -> Self {
        let lower = name.to_lowercase();
        if lower.ends_with(".bmp") || lower.ends_with(".pcx") || lower.ends_with(".jpg") {
            ArchiveFileType::Bitmap
        } else if lower.ends_with(".pal") || lower.ends_with(".act") {
            ArchiveFileType::Palette
        } else if lower.starts_with("sp") || lower.starts_with("ro") || lower.starts_with("ob") {
            // Rough heuristic for MM6 sprites which often lack clear extensions or follow specific prefixes
            // More refined logic could be placed here if a known extension is documented.
            ArchiveFileType::Sprite
        } else if lower.ends_with(".txt") {
            ArchiveFileType::Text
        } else if lower.ends_with(".bin") {
            ArchiveFileType::Binary
        } else if lower.ends_with(".wav") || lower.ends_with(".snd") {
            ArchiveFileType::Sound
        } else if lower.ends_with(".vid") || lower.ends_with(".smk") {
            ArchiveFileType::Video
        } else if lower.ends_with(".mm6") {
            ArchiveFileType::SaveData
        } else if lower.ends_with(".evt") {
            ArchiveFileType::Event
        } else {
            ArchiveFileType::Unknown
        }
    }
}

/// Metadata for a single file contained within an archive.
#[derive(Debug, Clone)]
pub struct ArchiveEntry {
    /// Original case-preserved filename from the archive.
    pub name: String,
    /// Size in bytes of the file content in the archive (potentially compressed).
    pub size: usize,
    /// Uncompressed size, if known/stored (0 if unknown or uncompressed).
    pub decompressed_size: usize,
    /// Heuristically determined file type.
    pub file_type: ArchiveFileType,
}

impl ArchiveEntry {
    pub fn new(name: String, size: usize, decompressed_size: usize) -> Self {
        let file_type = ArchiveFileType::from_name(&name);
        Self {
            name,
            size,
            decompressed_size,
            file_type,
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
