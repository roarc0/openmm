use lod::LodManager;

fn main() {
    let lod_manager = LodManager::new(lod::get_lod_path()).unwrap();
    let raw = lod_manager.try_get_bytes("icons/mapstats.txt").unwrap();
    let data = match lod::lod_data::LodData::try_from(raw) {
        Ok(d) => d.data,
        Err(_) => raw.to_vec(),
    };
    let text = String::from_utf8_lossy(&data);
    for (i, line) in text.lines().enumerate().take(8) {
        println!("LINE {}: {}", i, line);
        if i >= 2 {
            let cols: Vec<&str> = line.split('\t').collect();
            for (j, col) in cols.iter().enumerate() {
                println!("  col[{}] = {:?}", j, col.trim());
            }
        }
    }
}
