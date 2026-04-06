use super::*;

#[test]
fn palette_try_from_wrong_size_errors() {
    let short_data = vec![0u8; 100];
    let result = Palette::try_from(short_data.as_slice());
    assert!(result.is_err());
}

#[test]
fn palette_try_from_correct_size_ok() {
    let data = vec![0u8; PALETTE_DATA_SIZE];
    let palette = Palette::try_from(data.as_slice()).unwrap();
    assert_eq!(palette.data.len(), PALETTE_SIZE);
    assert_eq!(palette.data, [0u8; PALETTE_SIZE]);
}

#[test]
fn palette_try_from_skips_header() {
    let mut data = vec![0u8; PALETTE_DATA_SIZE];
    // Set bytes in the header (should be skipped)
    for i in 0..PALETTE_HEADER_SIZE {
        data[i] = 0xFF;
    }
    // Set first palette color to a known value
    data[PALETTE_HEADER_SIZE] = 42;
    let palette = Palette::try_from(data.as_slice()).unwrap();
    assert_eq!(palette.data[0], 42);
}

#[test]
fn extract_palette_id_valid() {
    assert_eq!(extract_palette_id("PAL123").unwrap(), 123);
    assert_eq!(extract_palette_id("pal001").unwrap(), 1);
    assert_eq!(extract_palette_id("pal000").unwrap(), 0);
    assert_eq!(extract_palette_id("x999").unwrap(), 999);
}

#[test]
fn extract_palette_id_too_short_errors() {
    assert!(extract_palette_id("P1").is_err());
    assert!(extract_palette_id("").is_err());
}

#[test]
fn extract_palette_id_non_numeric_suffix_errors() {
    assert!(extract_palette_id("palXYZ").is_err());
    assert!(extract_palette_id("palABC").is_err());
}
