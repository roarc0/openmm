use flate2::bufread::ZlibDecoder;
use std::{
    error::Error,
    io::{BufReader, Cursor, Read},
};

pub fn decompress(
    data: &[u8],
    compressed_size: usize,
    uncompressed_size: usize,
) -> Result<Vec<u8>, Box<dyn Error>> {
    check_size(data.len(), compressed_size)?;
    let uncompressed_data = decompress_zlib(data, uncompressed_size)?;
    check_size(uncompressed_data.len(), uncompressed_size)?;
    Ok(uncompressed_data)
}

fn decompress_zlib(data: &[u8], reserve_size: usize) -> Result<Vec<u8>, Box<dyn Error>> {
    let reader: BufReader<_> = BufReader::new(Cursor::new(data));
    let mut z = ZlibDecoder::new(reader);
    let mut buf: Vec<u8> = Vec::with_capacity(reserve_size);
    z.read_to_end(&mut buf)?;
    Ok(buf)
}

fn check_size(size: usize, expected_size: usize) -> Result<(), Box<dyn Error>> {
    if size != expected_size {
        return Err(format!(
            "Expected  data size: {}B, actual size: {}B",
            expected_size, size
        )
        .into());
    }
    Ok(())
}
