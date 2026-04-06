//! Generate a complete synthetic LOD set — no MM6 game data required.
//!
//! Produces a directory containing all .lod files the engine needs to boot:
//!
//!   bitmaps.lod  — one stub palette (pal001) + one solid-green 128×128 bitmap tile
//!   sprites.lod  — empty archive (sprites loaded on demand; skipped in test)
//!   icons.lod    — mapstats.txt, items.txt, dtile.bin, dpft.bin, dchest.bin
//!   games.lod    — test.odm (sine-wave heightmap + one box building)
//!   new.lod      — empty archive
//!
//! Usage:
//!   cargo run -p lod --example create_test_lod -- [OUTPUT_DIR]
//!
//! OUTPUT_DIR defaults to `lod/testdata/generated/`.
//! After running, point OPENMM_6_PATH to that directory to use it as a game source.

use byteorder::{LittleEndian, WriteBytesExt};
use openmm_archive::Archive;
use openmm_data::{LodWriter, Lod, generator::terrain::TerrainGen};
use std::{error::Error, io::Write, path::{Path, PathBuf}};

fn main() -> Result<(), Box<dyn Error>> {
    let out_dir: PathBuf = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("lod/testdata/generated"));

    std::fs::create_dir_all(&out_dir)?;
    println!("Writing synthetic LOD set to {}", out_dir.display());

    build_bitmaps_lod(&out_dir)?;
    build_sprites_lod(&out_dir)?;
    build_icons_lod(&out_dir)?;
    build_games_lod(&out_dir)?;
    build_new_lod(&out_dir)?;

    // ─── Dump phase ──────────────────────────────────────────────────────────
    let dump_dir = out_dir.parent().unwrap_or(&out_dir).join("generated_dump");
    println!("\nDumping archive contents to {}...", dump_dir.display());
    let _ = std::fs::remove_dir_all(&dump_dir);
    std::fs::create_dir_all(&dump_dir)?;

    for archive in &["bitmaps", "sprites", "icons", "games", "new"] {
        let lod_path = out_dir.join(format!("{}.lod", archive));
        if lod_path.exists() {
            dump_lod(&lod_path, &dump_dir.join(archive))?;
        }
    }

    println!("\nDone. Run with OPENMM_6_PATH={}", out_dir.display());
    println!("Then: cargo run -p lod --example lod_roundtrip");
    Ok(())
}

fn dump_lod(lod_path: &Path, target_dir: &Path) -> Result<(), Box<dyn Error>> {
    let lod = Lod::open(lod_path)?;
    std::fs::create_dir_all(target_dir)?;
    
    for entry in lod.list_files() {
        let data = lod.get_file(&entry.name).unwrap_or_default();
        let file_path = target_dir.join(entry.name.to_lowercase());
        std::fs::write(file_path, data)?;
    }
    let n = lod.list_files().len();
    println!("  extracted {} — {} files", lod_path.file_name().unwrap().to_string_lossy(), n);
    Ok(())
}

// ─── bitmaps.lod ─────────────────────────────────────────────────────────────
//
// Palettes: files named "pal001..palNNN" — 48 byte header + 768 byte RGB table.
// Bitmaps: files like "grastyl" — 48 byte header + zlib pixels + 768 palette.

fn build_bitmaps_lod(out: &PathBuf) -> Result<(), Box<dyn Error>> {
    let mut lod = LodWriter::new();

    // Palette 001 — a simple grayscale ramp (used by the stub tile below)
    lod.add_file("pal001", make_palette([128u8, 160, 96])   /* green tint */);
    lod.add_file("pal002", make_palette([80, 80, 80])        /* gray */);

    // One terrain tile "grastyl" — solid green, 128×128, palette-indexed
    lod.add_file("grastyl", make_bitmap_tile(128, 128, 4 /* green-ish palette index */)?);
    // Water tile — solid cyan-marked tile (engine detects cyan → animated water)
    lod.add_file("wtrtyl", make_bitmap_tile_rgb(128, 128, [0, 255, 255])?);

    lod.save(out.join("bitmaps.lod"))?;
    println!("  bitmaps.lod — 4 entries (2 palettes, 2 tiles)");
    Ok(())
}

/// Build a 816-byte palette entry: 48 zero-byte header + 768 B RGB table.
/// All 256 colours are set to variants of `base_rgb`.
fn make_palette(base_rgb: [u8; 3]) -> Vec<u8> {
    let mut data = vec![0u8; 48]; // header — ignored by palette parser
    for i in 0u8..=255 {
        let scale = i as f32 / 255.0;
        data.push((base_rgb[0] as f32 * scale) as u8);
        data.push((base_rgb[1] as f32 * scale) as u8);
        data.push((base_rgb[2] as f32 * scale) as u8);
    }
    data // 48 + 768 = 816 bytes
}

/// Build a bitmap tile: MM6 image format (palette-indexed, zlib compressed).
/// All pixels use palette index `pal_idx`.
fn make_bitmap_tile(w: u16, h: u16, pal_idx: u8) -> Result<Vec<u8>, Box<dyn Error>> {
    let pixels = vec![pal_idx; w as usize * h as usize];
    let compressed = openmm_data::generator::zlib_compress(&pixels);
    make_bitmap_bytes(w, h, &compressed, &pixels, None)
}

/// Build a bitmap tile with a specific solid RGB colour (packed into the embedded palette).
fn make_bitmap_tile_rgb(w: u16, h: u16, rgb: [u8; 3]) -> Result<Vec<u8>, Box<dyn Error>> {
    // All pixels use palette index 1; colour 1 in the palette = rgb
    let pixels = vec![1u8; w as usize * h as usize];
    let compressed = openmm_data::generator::zlib_compress(&pixels);
    
    let mut palette = [0u8; 768];
    for i in 0..256 {
        palette[i*3] = i as u8;
        palette[i*3+1] = i as u8;
        palette[i*3+2] = i as u8;
    }
    palette[3] = rgb[0];
    palette[4] = rgb[1];
    palette[5] = rgb[2];
    
    make_bitmap_bytes(w, h, &compressed, &pixels, Some(&palette))
}

/// Assemble a raw MM6 bitmap file blob.
///
/// Layout:  [48 B header] [compressed pixels] [768 B palette]
fn make_bitmap_bytes(w: u16, h: u16, compressed: &[u8], pixels: &[u8], palette: Option<&[u8; 768]>) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut buf = Vec::new();

    // 48-byte bitmap header
    buf.write_all(&[0u8; 16])?;
    buf.write_u32::<LittleEndian>(pixels.len() as u32)?;
    buf.write_u32::<LittleEndian>(compressed.len() as u32)?;
    buf.write_u16::<LittleEndian>(w)?;
    buf.write_u16::<LittleEndian>(h)?;
    buf.write_all(&[0u8; 12])?;
    buf.write_u32::<LittleEndian>(pixels.len() as u32)?;
    buf.write_all(&[0u8; 4])?;
    assert_eq!(buf.len(), 48);

    // Compressed pixels
    buf.extend_from_slice(compressed);

    // 768-byte embedded palette.
    if let Some(pal) = palette {
        buf.extend_from_slice(pal);
    } else {
        for i in 0u8..=255 {
            buf.push(i); // R
            buf.push(i); // G
            buf.push(i); // B
        }
    }
    Ok(buf)
}

// ─── sprites.lod ─────────────────────────────────────────────────────────────

fn build_sprites_lod(out: &PathBuf) -> Result<(), Box<dyn Error>> {
    LodWriter::new().save(out.join("sprites.lod"))?;
    println!("  sprites.lod — empty");
    Ok(())
}

// ─── icons.lod ───────────────────────────────────────────────────────────────

fn build_icons_lod(out: &PathBuf) -> Result<(), Box<dyn Error>> {
    let mut lod = LodWriter::new();

    lod.add_file("mapstats.txt", make_mapstats());
    lod.add_file("items.txt",    make_items());
    lod.add_file("dtile.bin",    make_dtile_bin());
    lod.add_file("dpft.bin",     make_dpft_bin());
    lod.add_file("dchest.bin",   make_dchest_bin());

    lod.save(out.join("icons.lod"))?;
    println!("  icons.lod — 5 entries (mapstats, items, dtile, dpft, dchest)");
    Ok(())
}

fn make_mapstats() -> Vec<u8> {
    // TSV: 3 header lines then one data row
    let mut s = String::new();
    s.push_str("MapStats.txt\r\n");
    s.push_str("Map name\tFilename\tReset Count\tFirst visit day\tRefill days\tx5 Lock\tTrap d20\t");
    s.push_str("Treasure\tEncounter%\tMon1Enc%\tMon2Enc%\tMon3Enc%\t");
    s.push_str("Mon1Pic\tMon1Name\tMon1Dif\tMon1#\tMon2Pic\tMon2Name\tMon2Dif\tMon2#\t");
    s.push_str("Mon3Pic\tMon3Name\tMon3Dif\tMon3#\tRedbook\tDesigner\r\n");
    s.push_str("---\r\n");
    s.push_str("1\tTest Plains\ttest.odm\t0\t0\t7\t0\t1\t1\t20\t100\t0\t0\t");
    s.push_str("Goblin\tGoblin\t1\t1-3\t");
    s.push_str("\t\t0\t0\t");
    s.push_str("\t\t0\t0\t");
    s.push_str("1\t\r\n");
    s.into_bytes()
}

fn make_items() -> Vec<u8> {
    let mut s = String::new();
    s.push_str("Items.txt\r\n");
    s.push_str("Item#\tPicFile\tName\tValue\tEquipStat\tSkillGroup\tMod1\tMod2\tMaterial\tID/Rep/St\t");
    s.push_str("NotIdentifiedName\tSpriteIndex\tShape\tEquipX\tEquipY\tNotes\r\n");
    s.push_str("1\tisword\tIron Sword\t50\tWeapon\tSword\t2d3\t0\t0\t0\t");
    s.push_str("Sword\t1\t2\t4\t0\tBasic sword\r\n");
    s.into_bytes()
}

/// dtile.bin: u32 count + n × 26 B records.
/// Record layout: name[16] + id i16 + bitmap i16 + tile_set i16 + section i16 + attributes u16.
fn make_dtile_bin() -> Vec<u8> {
    let mut buf = Vec::new();

    // We need at least 512 entries so indices [0..511] are valid.
    // Most will be blank (name="pending", tile_set=0 = grass).
    let count: u32 = 512;
    buf.write_u32::<LittleEndian>(count).unwrap();

    for i in 0u32..count {
        // name: "grastyl\0..." for first 35, "pendant\0..." for rest
        let name = if i < 35 { "grastyl" } else { "pending" };
        let mut name_buf = [0u8; 16];
        let b = name.as_bytes();
        name_buf[..b.len().min(15)].copy_from_slice(&b[..b.len().min(15)]);
        buf.extend_from_slice(&name_buf);
        buf.write_i16::<LittleEndian>(i as i16).unwrap(); // id
        buf.write_i16::<LittleEndian>(0).unwrap();         // bitmap
        buf.write_i16::<LittleEndian>(0).unwrap();         // tile_set = 0 (grass)
        buf.write_i16::<LittleEndian>(0).unwrap();         // section
        buf.write_u16::<LittleEndian>(0).unwrap();         // attributes
    }
    buf
}

/// dpft.bin: u32 count + n × 10 B frames.
fn make_dpft_bin() -> Vec<u8> {
    let mut buf = Vec::new();
    buf.write_u32::<LittleEndian>(1).unwrap();
    buf.write_u16::<LittleEndian>(0).unwrap(); // group_id
    buf.write_u16::<LittleEndian>(0).unwrap(); // frame_index
    buf.write_i16::<LittleEndian>(4).unwrap(); // time
    buf.write_i16::<LittleEndian>(4).unwrap(); // total_time
    buf.write_u16::<LittleEndian>(0).unwrap(); // bits
    buf
}

/// dchest.bin: u32 count + n × 36 B entries.
fn make_dchest_bin() -> Vec<u8> {
    let mut buf = Vec::new();
    buf.write_u32::<LittleEndian>(1).unwrap();
    let mut name_buf = [0u8; 32];
    b"chest01".iter().enumerate().for_each(|(i, &b)| name_buf[i] = b);
    buf.extend_from_slice(&name_buf);
    buf.write_u8(9).unwrap();  // width
    buf.write_u8(9).unwrap();  // height
    buf.write_i16::<LittleEndian>(0).unwrap(); // image_index
    buf
}

// ─── games.lod ───────────────────────────────────────────────────────────────

fn build_games_lod(out: &PathBuf) -> Result<(), Box<dyn Error>> {
    let mut lod = LodWriter::new();
    lod.add_file("test.odm", make_odm()?);
    lod.save(out.join("games.lod"))?;
    println!("  games.lod — 1 entry (test.odm)");
    Ok(())
}

fn make_odm() -> Result<Vec<u8>, Box<dyn Error>> {
    // Generate a 128×128 sine-wave heightmap
    let tgen = TerrainGen::new(128, 128);
    let mut height_map = tgen.generate(42);
    // Flatten a 20×20 patch in the middle for a building platform
    tgen.flatten_rect(&mut height_map, 55, 55, 20, 20, 110);

    // Flat attribute and tile maps (all grass)
    let tile_map = vec![90u8; 128 * 128]; // index 90 = primary tileset slot
    let attr_map = vec![0u8; 128 * 128];

    // 8 tile_data u16s: standard grass layout (grass starts at dtile index 0)
    let tile_data: [u16; 8] = [0, 0, 126, 126, 126, 162, 0, 198];

    // ── assemble binary ODM ──────────────────────────────────────────────────
    let mut buf = Vec::new();

    // 5 × 32-byte string fields
    write_str32(&mut buf, "Test Plains");
    write_str32(&mut buf, "test.odm");
    write_str32(&mut buf, "MMVI");
    write_str32(&mut buf, "sky01");   // sky texture
    write_str32(&mut buf, "grastyl"); // ground texture

    // tile_data: 8 × u16
    for &td in &tile_data {
        buf.write_u16::<LittleEndian>(td)?;
    }

    // Verify we're at offset 176 (HEIGHT_MAP_OFFSET)
    assert_eq!(buf.len(), 176, "ODM header size mismatch: {}", buf.len());

    // Maps — 128×128 each
    buf.extend_from_slice(&height_map);
    buf.extend_from_slice(&tile_map);
    buf.extend_from_slice(&attr_map);

    // BSP models: 0 (no buildings in minimal map — keeps parser safe)
    buf.write_u32::<LittleEndian>(0)?;

    // Billboards: 0
    buf.write_u32::<LittleEndian>(0)?;

    // Spawn points: 1 (player start)
    buf.write_u32::<LittleEndian>(1)?;
    // SpawnPoint: x, y, z, radius, spawn_type, monster_index, attributes
    buf.write_i32::<LittleEndian>(0)?;      // x
    buf.write_i32::<LittleEndian>(0)?;      // y
    buf.write_i32::<LittleEndian>(1024)?;   // z (above ground)
    buf.write_u16::<LittleEndian>(256)?;    // radius
    buf.write_u16::<LittleEndian>(0)?;      // spawn_type  = 0 (player start)
    buf.write_u16::<LittleEndian>(0)?;      // monster_index
    buf.write_u16::<LittleEndian>(0)?;      // attributes

    Ok(buf)
}

// ─── new.lod ─────────────────────────────────────────────────────────────────

fn build_new_lod(out: &PathBuf) -> Result<(), Box<dyn Error>> {
    LodWriter::new().save(out.join("new.lod"))?;
    println!("  new.lod    — empty");
    Ok(())
}

// ─── helpers ─────────────────────────────────────────────────────────────────

fn write_str32(buf: &mut Vec<u8>, s: &str) {
    let mut block = [0u8; 32];
    let b = s.as_bytes();
    block[..b.len().min(31)].copy_from_slice(&b[..b.len().min(31)]);
    buf.extend_from_slice(&block);
}
