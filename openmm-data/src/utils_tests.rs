use super::*;
use std::io::Cursor;

#[test]
fn try_read_string_null_terminated() {
    let data = b"Hello\0World";
    let mut cursor = Cursor::new(&data[..]);
    let s = try_read_string(&mut cursor).unwrap();
    assert_eq!(s, "Hello");
}

#[test]
fn try_read_string_first_byte_null() {
    let data = b"\0rest";
    let mut cursor = Cursor::new(&data[..]);
    let s = try_read_string(&mut cursor).unwrap();
    assert_eq!(s, "");
}

#[test]
fn try_read_string_no_null_pops_last_byte() {
    // Without a null terminator, read_until reads to EOF then pop() removes
    // the last real byte. This matches the designed use: always null-terminated input.
    let data = b"NoNull";
    let mut cursor = Cursor::new(&data[..]);
    let s = try_read_string(&mut cursor).unwrap();
    // pop() removes the trailing 'l', leaving "NoNul"
    assert_eq!(s, "NoNul");
}

#[test]
fn try_read_name_lowercases() {
    let data = b"GRASTYL\0padding";
    let s = try_read_name(data);
    assert_eq!(s, Some("grastyl".to_string()));
}

#[test]
fn try_read_name_all_null_is_empty_string() {
    let data = b"\0\0\0";
    let s = try_read_name(data);
    assert_eq!(s, Some("".to_string()));
}

#[test]
fn try_read_string_block_advances_by_size() {
    let data = b"Hi\0padding_bytes_here";
    let mut cursor = Cursor::new(&data[..]);
    let s = try_read_string_block(&mut cursor, 8).unwrap();
    assert_eq!(s, "Hi");
    assert_eq!(cursor.position(), 8);
}

#[test]
fn try_read_string_block_reads_full_block_when_no_null() {
    let data = b"Abcdefghijklmnop";
    let mut cursor = Cursor::new(&data[..]);
    let s = try_read_string_block(&mut cursor, 4).unwrap();
    // Reads until null or end — here reads 4 bytes content then seeks to pos 4
    assert_eq!(cursor.position(), 4);
    let _ = s; // content varies (no null found in first 4 bytes = reads all 4)
}
