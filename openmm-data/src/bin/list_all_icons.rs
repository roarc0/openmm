fn main() {
    let assets = openmm_data::Assets::new(openmm_data::get_data_path()).unwrap();
    if let Some(files) = assets.files_in("icons") {
        let mut items: Vec<(String, u32, u32)> = Vec::new();
        for f in &files {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| assets.lod().icon(f)));
            if let Ok(Some(img)) = result {
                items.push((f.to_string(), img.width(), img.height()));
            }
        }
        items.sort_by(|a, b| a.0.cmp(&b.0));
        for (name, w, h) in &items {
            println!("{:20} {}x{}", name, w, h);
        }
        eprintln!("Total: {} icons", items.len());
    }
}
