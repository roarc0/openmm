/// Quick diagnostic: compare position vs initial_position in DDM actors from a save file.
/// Usage: cargo run --example check_ddm_positions [save.mm6] [MapName]
use openmm_data::assets::ddm::Ddm;
use openmm_data::save::file::SaveFile;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let save_path = args.get(1).map(|s| s.as_str()).unwrap_or("data/Saves/save006.mm6");
    let map_name = args.get(2).map(|s| s.as_str()).unwrap_or("Oute3");

    let save = SaveFile::open(save_path).expect("open save");

    // Try both DDM and DLV (case-insensitive)
    let ddm_candidates = [
        format!("{}.ddm", map_name),
        format!("{}.DDM", map_name),
        format!("{}.dlv", map_name),
    ];

    let mut data = None;
    let mut found_name = String::new();
    for name in &ddm_candidates {
        if let Some(d) = save.get_file(name) {
            found_name = name.clone();
            data = Some(d);
            break;
        }
    }
    // Also try case-insensitive
    if data.is_none() {
        if let Some(d) = save.get_file_ci(&format!("{}.ddm", map_name)) {
            found_name = format!("{}.ddm (ci)", map_name);
            data = Some(d);
        } else if let Some(d) = save.get_file_ci(&format!("{}.dlv", map_name)) {
            found_name = format!("{}.dlv (ci)", map_name);
            data = Some(d);
        }
    }

    // SaveFile::get_file handles decompression transparently
    let data = data.unwrap_or_else(|| panic!("no DDM/DLV for '{map_name}' in save"));
    println!("=== {} from {} ({} bytes) ===\n", found_name, save_path, data.len());

    match Ddm::parse_from_data(&data) {
        Ok(actors) => dump_actors(&actors),
        Err(e) => {
            eprintln!("parse_from_data failed: {e}");
            eprintln!("First 64 bytes: {:02x?}", &data[..64.min(data.len())]);
        }
    }
}

fn dump_actors(actors: &[openmm_data::assets::ddm::DdmActor]) {
    println!("Total actors: {}\n", actors.len());

    let mut moved = 0;
    let mut dead = 0;
    for (i, a) in actors.iter().enumerate() {
        let pos_diff = a.position != a.initial_position;
        let is_dead = a.ai_state == 5;
        if pos_diff || is_dead || a.hp == 0 {
            println!(
                "  [{:3}] '{}' pos=[{},{},{}] init=[{},{},{}] ai={} hp={} {}{}{}",
                i,
                a.name,
                a.position[0],
                a.position[1],
                a.position[2],
                a.initial_position[0],
                a.initial_position[1],
                a.initial_position[2],
                a.ai_state,
                a.hp,
                if pos_diff { "MOVED " } else { "" },
                if is_dead { "DEAD " } else { "" },
                if a.hp == 0 && !is_dead { "HP=0 " } else { "" },
            );
            if pos_diff {
                moved += 1;
            }
            if is_dead {
                dead += 1;
            }
        }
    }
    println!("\nSummary: {moved} moved, {dead} dead out of {}", actors.len());
}
