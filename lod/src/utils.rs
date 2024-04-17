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

pub(super) fn try_read_string_block(
    cursor: &mut Cursor<&[u8]>,
    size: usize,
) -> Result<String, Box<dyn Error>> {
    let pos = cursor.position();
    let s = try_read_string(cursor)?;
    cursor.seek(std::io::SeekFrom::Start(pos + size as u64))?;
    Ok(s)
}
