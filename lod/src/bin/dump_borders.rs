fn main() {
    let lod = lod::LodManager::new(lod::get_lod_path()).unwrap();
    let out = std::path::Path::new("/tmp/borders");
    std::fs::create_dir_all(out).unwrap();
    for name in &["border1.pcx", "border2.pcx", "border3", "border4", "border5", "border6"] {
        // Dump raw PCX header
        let path = format!("icons/{}", name.to_lowercase());
        if let Ok(data) = lod.get_decompressed(&path) {
            if data.len() >= 128 && data[0] == 0x0A {
                let x_min = u16::from_le_bytes([data[4], data[5]]);
                let y_min = u16::from_le_bytes([data[6], data[7]]);
                let x_max = u16::from_le_bytes([data[8], data[9]]);
                let y_max = u16::from_le_bytes([data[10], data[11]]);
                println!("{}: x_min={} y_min={} x_max={} y_max={} => w={} h={} (with +1: {}x{})",
                    name, x_min, y_min, x_max, y_max,
                    x_max - x_min, y_max - y_min,
                    x_max - x_min + 1, y_max - y_min + 1);
            }
        }
        if let Some(img) = lod.icon(name) {
            let fname = name.replace(".pcx", "");
            img.save(out.join(format!("{}.png", fname))).unwrap();
            println!("Saved {}: {}x{}", name, img.width(), img.height());
        }
    }
}
