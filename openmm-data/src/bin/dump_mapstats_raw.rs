use openmm_data::Assets;

fn main() {
    let assets = Assets::new(openmm_data::get_data_path()).unwrap();
    let raw = assets.get_bytes("icons/mapstats.txt").unwrap();
    let data = match openmm_data::assets::lod_data::LodData::try_from(raw.as_slice()) {
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
