use std::{
    error::Error,
    io::{BufRead, Cursor, Read, Seek},
};

pub(super) fn try_read_string<R>(r: &mut R) -> Result<String, Box<dyn Error>>
where
    R: Read + BufRead,
{
    let mut buffer = Vec::new();
    let _ = r.read_until(b'\0', &mut buffer);
    if !buffer.is_empty() {
        _ = buffer.pop();
    }
    Ok(String::from_utf8(buffer)?)
}

pub(super) fn try_read_name(name: &[u8]) -> Option<String> {
    let mut cursor = Cursor::new(name);
    try_read_string(&mut cursor).map(|s| s.to_lowercase()).ok()
}

pub(super) fn try_read_string_block(cursor: &mut Cursor<&[u8]>, size: usize) -> Result<String, Box<dyn Error>> {
    let pos = cursor.position();
    let s = try_read_string(cursor)?;
    cursor.seek(std::io::SeekFrom::Start(pos + size as u64))?;
    Ok(s)
}

/// On Linux, files and directories are case-sensitive. MM6 assets often mix cases.
/// This helper resolves a relative path from a base directory by searching the 
/// filesystem case-insensitively for each component.
pub fn find_path_case_insensitive(base: &std::path::Path, relative: &str) -> Option<std::path::PathBuf> {
    let mut current = base.to_path_buf();
    for component in std::path::Path::new(relative).components() {
        match component {
            std::path::Component::Normal(target) => {
                let target_str = target.to_str()?.to_lowercase();
                let mut found = None;
                if let Ok(entries) = std::fs::read_dir(&current) {
                    for entry in entries.flatten() {
                        if entry.file_name().to_string_lossy().to_lowercase() == target_str {
                            found = Some(entry.path());
                            break;
                        }
                    }
                }
                current = found?;
            }
            std::path::Component::RootDir => current = std::path::PathBuf::from("/"),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => { current.pop(); }
            _ => return None,
        }
    }
    Some(current)
}

#[cfg(test)]
pub fn test_lod() -> Option<crate::LodManager> {
    crate::LodManager::new(crate::get_data_path()).ok()
}

#[cfg(test)]
#[path = "utils_tests.rs"]
mod tests;
