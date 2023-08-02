use flate2::bufread::ZlibDecoder;
use std::{
    error::Error,
    io::{BufReader, Cursor, Read},
};

pub fn decompress(data: &[u8], uncompressed_size: usize) -> Result<Vec<u8>, Box<dyn Error>> {
    let reader: BufReader<_> = BufReader::new(Cursor::new(data));
    let mut z = ZlibDecoder::new(reader);
    let mut buf: Vec<u8> = Vec::with_capacity(uncompressed_size);
    z.read_to_end(&mut buf)?;
    Ok(buf)
}

pub fn check_size(size: usize, expected_size: usize) -> Result<(), Box<dyn Error>> {
    if size != expected_size {
        return Err(format!(
            "Expected  data size: {}B, actual size: {}B",
            expected_size, size
        )
        .into());
    }
    Ok(())
}
