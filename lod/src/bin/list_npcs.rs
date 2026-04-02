fn main() {
    let lod_manager = lod::LodManager::new(lod::get_lod_path()).unwrap();
    let data = lod_manager.get_decompressed("icons/npcdata.txt").unwrap();
    let text = String::from_utf8_lossy(&data);
    // Show first 5 data rows (skip 2 header rows)
    for line in text.lines().skip(2).take(5) {
        let cols: Vec<&str> = line.split('\t').collect();
        println!(
            "id={} name={:?} portrait={} profession={} map={}",
            cols.first().unwrap_or(&""),
            cols.get(1).unwrap_or(&""),
            cols.get(2).unwrap_or(&""),
            cols.get(7).unwrap_or(&""),
            cols.get(6).unwrap_or(&"")
        );
    }
}
