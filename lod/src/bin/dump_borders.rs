fn main() {
    let lod = lod::LodManager::new(lod::get_lod_path()).unwrap();
    let out = std::path::Path::new("/tmp/borders");
    std::fs::create_dir_all(out).unwrap();
    for name in &["border1.pcx", "border2.pcx", "border3", "border4", "border5", "border6"] {
        if let Some(img) = lod.icon(name) {
            let fname = name.replace(".pcx", "");
            img.save(out.join(format!("{}.png", fname))).unwrap();
            println!("Saved {}: {}x{}", name, img.width(), img.height());
        }
    }
}
