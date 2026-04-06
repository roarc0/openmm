use openmm_data::Assets;

fn main() {
    let assets = Assets::new(openmm_data::get_data_path()).expect("failed to open LOD files");

    // List all peasant sprite files and check their palette IDs
    if let Some(files) = assets.files_in("sprites") {
        let mut peasant_files: Vec<_> = files
            .iter()
            .filter(|f| {
                let l = f.to_lowercase();
                l.starts_with("pfem")
                    || l.starts_with("pman")
                    || l.starts_with("pmn2")
                    || l.starts_with("fmpst")
                    || l.starts_with("fmpwa")
            })
            .collect();
        peasant_files.sort();

        println!("Peasant sprite files and their embedded palette IDs:\n");
        for file in &peasant_files {
            let path = format!("sprites/{}", file);
            if let Ok(data) = assets.get_bytes(&path)
                && data.len() >= 22
            {
                let pal = u16::from_le_bytes([data[20], data[21]]);
                println!("{:20} pal={}", file, pal);
            }
        }

        println!("\nTotal peasant sprite files: {}", peasant_files.len());
    }
}
