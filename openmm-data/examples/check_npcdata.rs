fn main() {
    let path = openmm_data::get_data_path();
    let assets = openmm_data::Assets::new(&path).unwrap();
    let raw = assets
        .get_decompressed("NPCdata.txt")
        .or_else(|_| assets.get_decompressed("npcdata.txt"));
    match raw {
        Ok(data) => {
            let text = String::from_utf8_lossy(&data);
            for (i, line) in text.lines().enumerate().take(6) {
                let cols: Vec<_> = line.split('\t').collect();
                println!("{}: {:?}", i, cols);
            }
        }
        Err(_) => println!("not found"),
    }
}
