fn main() {
    let lod = lod::LodManager::new(lod::get_data_path()).unwrap();
    if let Some(files) = lod.files_in("icons") {
        let mut bgs: Vec<(String, u32, u32)> = Vec::new();
        for f in &files {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| lod.game().icon(f)));
            if let Ok(Some(img)) = result
                && img.width() >= 200
            {
                bgs.push((f.to_string(), img.width(), img.height()));
            }
        }
        bgs.sort_by(|a, b| a.0.cmp(&b.0));
        println!("Large icons (>= 200px wide):");
        for (name, w, h) in &bgs {
            println!("  {:20} {}x{}", name, w, h);
        }
    }
}
