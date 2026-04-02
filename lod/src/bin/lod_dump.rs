//! Universal LOD data dumper.
//! Usage: lod_dump -n <name> [-f json|raw] [--filter <text>]
//!
//! Examples:
//!   lod_dump -n dsft                     # dump all dsft sprite frames
//!   lod_dump -n dsft --filter shp        # only shp entries
//!   lod_dump -n ddeclist                  # dump decoration list
//!   lod_dump -n ddeclist --filter boat    # filter decorations
//!   lod_dump -n dsounds                   # dump sound descriptors
//!   lod_dump -n billboards oute3.odm      # dump billboards from a map
//!   lod_dump -n billboards oute3.odm --filter shp
//!   lod_dump -n odm oute3.odm             # dump ODM metadata
//!   lod_dump -n raw icons/dsounds.bin -f raw  # dump raw bytes to stdout

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let name = find_arg(&args, "-n").unwrap_or_else(|| {
        usage();
        std::process::exit(1);
    });
    let format = find_arg(&args, "-f").unwrap_or("json".into());
    let filter = find_arg(&args, "--filter").map(|s| s.to_lowercase());
    // Positional arg after -n value (for map name etc.)
    let extra = args
        .iter()
        .position(|a| a == "-n")
        .and_then(|i| args.get(i + 2))
        .filter(|a| !a.starts_with('-'))
        .cloned();

    let lod_manager = lod::LodManager::new(lod::get_lod_path()).expect("failed to open LOD files");

    match name.as_str() {
        "dsft" => dump_dsft(&lod_manager, &filter),
        "ddeclist" => dump_ddeclist(&lod_manager, &filter),
        "dsounds" => dump_dsounds(&lod_manager, &filter),
        "billboards" | "bb" => {
            let map = extra.unwrap_or_else(|| {
                eprintln!("Need map name: lod_dump -n billboards oute3.odm");
                std::process::exit(1)
            });
            dump_billboards(&lod_manager, &map, &filter);
        }
        "odm" => {
            let map = extra.unwrap_or_else(|| {
                eprintln!("Need map name: lod_dump -n odm oute3.odm");
                std::process::exit(1)
            });
            dump_odm(&lod_manager, &map);
        }
        "raw" => {
            let path = extra.unwrap_or_else(|| {
                eprintln!("Need path: lod_dump -n raw icons/dsounds.bin");
                std::process::exit(1)
            });
            dump_raw(&lod_manager, &path, &format);
        }
        "archives" => dump_archives(&lod_manager),
        _ => {
            eprintln!(
                "Unknown data type: '{}'. Available: dsft, ddeclist, dsounds, billboards, odm, raw, archives",
                name
            );
            std::process::exit(1);
        }
    }
}

fn find_arg(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1).cloned())
}

fn usage() {
    eprintln!("Usage: lod_dump -n <type> [-f json|raw] [--filter <text>] [map.odm]");
    eprintln!("Types: dsft, ddeclist, dsounds, billboards <map>, odm <map>, raw <path>, archives");
}

fn matches_filter(filter: &Option<String>, texts: &[&str]) -> bool {
    match filter {
        None => true,
        Some(f) => texts.iter().any(|t| t.contains(f.as_str())),
    }
}

// ─── Dumpers ────────────────────────────────────────────────

fn dump_dsft(lod: &lod::LodManager, filter: &Option<String>) {
    let dsft = lod::dsft::DSFT::new(lod).expect("failed to load dsft.bin");
    println!("[");
    let mut first = true;
    for (i, frame) in dsft.frames.iter().enumerate() {
        let group = frame.group_name().unwrap_or_default();
        let sprite = frame.sprite_name().unwrap_or_default();
        if !matches_filter(filter, &[&group, &sprite]) {
            continue;
        }

        if !first {
            println!(",");
        }
        first = false;
        print!(
            "  {{ \"index\": {}, \"group\": \"{}\", \"sprite\": \"{}\", \"scale\": {}, \"time\": {}, \"palette_id\": {}, \"light_radius\": {}, \"attrs\": \"0x{:04x}\" }}",
            i, group, sprite, frame.scale, frame.time, frame.palette_id, frame.light_radius, frame.attributes
        );
    }
    println!("\n]");
    eprintln!("{} frames", dsft.frames.len());
}

fn dump_ddeclist(lod: &lod::LodManager, filter: &Option<String>) {
    let ddeclist = lod::ddeclist::DDecList::new(lod).expect("failed to load ddeclist.bin");
    println!("[");
    let mut first = true;
    for (i, item) in ddeclist.items.iter().enumerate() {
        let name = item.name().unwrap_or_default();
        let game_name = item.game_name().unwrap_or_default();
        if !matches_filter(filter, &[&name, &game_name]) {
            continue;
        }

        if !first {
            println!(",");
        }
        first = false;
        print!(
            "  {{ \"id\": {}, \"name\": \"{}\", \"game_name\": \"{}\", \"type\": {}, \"height\": {}, \"radius\": {}, \"light_radius\": {}, \"sft_index\": {}, \"sound_id\": {}, \"attrs\": \"0x{:04x}\" }}",
            i,
            name,
            game_name,
            item.dec_type,
            item.height,
            item.radius,
            item.light_radius,
            item.sft_index(),
            item.sound_id,
            item.attributes
        );
    }
    println!("\n]");
    eprintln!("{} decorations", ddeclist.items.len());
}

fn dump_dsounds(lod: &lod::LodManager, filter: &Option<String>) {
    let dsounds = lod::dsounds::DSounds::new(lod).expect("failed to load dsounds.bin");
    println!("[");
    let mut first = true;
    for (i, item) in dsounds.items.iter().enumerate() {
        let name = item.name().unwrap_or_default();
        if !matches_filter(filter, &[&name]) {
            continue;
        }

        if !first {
            println!(",");
        }
        first = false;
        print!(
            "  {{ \"index\": {}, \"name\": \"{}\", \"sound_id\": {}, \"type\": {}, \"attrs\": \"0x{:04x}\", \"is_3d\": {} }}",
            i,
            name,
            item.sound_id,
            item.sound_type,
            item.attributes,
            item.is_3d()
        );
    }
    println!("\n]");
    eprintln!("{} sounds", dsounds.items.len());
}

fn dump_billboards(lod: &lod::LodManager, map: &str, filter: &Option<String>) {
    let odm = lod::odm::Odm::new(lod, map).expect("failed to load ODM");
    println!("[");
    let mut first = true;
    for (i, bb) in odm.billboards.iter().enumerate() {
        let name = &bb.declist_name;
        if !matches_filter(filter, &[&name.to_lowercase()]) {
            continue;
        }

        if !first {
            println!(",");
        }
        first = false;
        print!(
            "  {{ \"index\": {}, \"name\": \"{}\", \"declist_id\": {}, \"pos\": [{}, {}, {}], \"direction\": {} }}",
            i,
            name,
            bb.data.declist_id,
            bb.data.position[0],
            bb.data.position[1],
            bb.data.position[2],
            bb.data.direction_degrees
        );
    }
    println!("\n]");
    eprintln!("{} billboards", odm.billboards.len());
}

fn dump_odm(lod: &lod::LodManager, map: &str) {
    let odm = lod::odm::Odm::new(lod, map).expect("failed to load ODM");
    println!("{{");
    println!("  \"name\": \"{}\",", odm.name);
    println!("  \"sky_texture\": \"{}\",", odm.sky_texture);
    println!("  \"ground_texture\": \"{}\",", odm.ground_texture);
    println!("  \"tile_data\": {:?},", odm.tile_data);
    println!("  \"bsp_models\": {},", odm.bsp_models.len());
    println!("  \"billboards\": {},", odm.billboards.len());
    println!("  \"spawn_points\": {}", odm.spawn_points.len());
    println!("}}");
}

fn dump_raw(lod: &lod::LodManager, path: &str, format: &str) {
    let data = lod.get_decompressed(path).expect("file not found in LOD");
    match format {
        "raw" => {
            use std::io::Write;
            std::io::stdout().write_all(&data).unwrap();
        }
        _ => {
            // Hex dump
            for (i, chunk) in data.chunks(16).enumerate() {
                print!("{:08x}  ", i * 16);
                for b in chunk {
                    print!("{:02x} ", b);
                }
                for _ in 0..16 - chunk.len() {
                    print!("   ");
                }
                print!(" |");
                for &b in chunk {
                    print!(
                        "{}",
                        if b.is_ascii_graphic() || b == b' ' {
                            b as char
                        } else {
                            '.'
                        }
                    );
                }
                println!("|");
            }
            eprintln!("{} bytes", data.len());
        }
    }
}

fn dump_archives(lod: &lod::LodManager) {
    let mut archives = lod.archives();
    archives.sort();
    for archive in &archives {
        let files = lod.files_in(archive).unwrap_or_default();
        println!("{}: {} files", archive, files.len());
    }
}
