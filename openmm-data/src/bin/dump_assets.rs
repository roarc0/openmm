use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use openmm_data::Assets;

fn main() {
    let lod_path = openmm_data::get_data_path();
    let assets = Assets::new(&lod_path).expect("failed to open LOD files");

    let out_dir = Path::new("data/dump");
    fs::create_dir_all(out_dir).expect("failed to create data/dump directory");

    // Build asset index
    let mut asset_map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut archives = assets.archives();
    archives.sort();

    let mut total = 0;
    for archive in &archives {
        if let Some(mut files) = assets.files_in(archive) {
            files.sort();
            total += files.len();
            asset_map.insert(archive.to_string(), files.iter().map(|s| s.to_string()).collect());
        }
    }

    // Write assets.json
    let json = serde_json_minimal(&asset_map);
    let json_path = out_dir.join("assets.json");
    fs::write(&json_path, &json).expect("failed to write assets.json");
    println!(
        "Wrote {} ({} archives, {} files total)",
        json_path.display(),
        archives.len(),
        total
    );

    for (archive, files) in &asset_map {
        println!("  {}: {} files", archive, files.len());
    }

    // Dump all files
    println!("\nDumping files to {}/ ...", out_dir.display());
    let palettes = assets.palettes().ok();
    for (archive, files) in &asset_map {
        let arch_dir = out_dir.join(archive);
        let _ = fs::create_dir_all(&arch_dir);
        for file in files {
            let path = format!("{}/{}", archive, file);
            if let Ok(bytes) = assets.get_decompressed(&path) {
                if let Some(img) = try_decode_image(&bytes, palettes) {
                    let png_name = format!("{}.png", file);
                    let _ = img.save(arch_dir.join(&png_name));
                } else {
                    let _ = fs::write(arch_dir.join(file), bytes);
                }
            }
        }
    }

    // Dump readable versions (JSON). Remove stale .txt files from previous runs.
    println!("\nDumping readable JSON files (*.json) ...");
    // Remove old .txt dump files so stale data is not confusing.
    if let Ok(entries) = std::fs::read_dir(out_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) == Some("txt")
                && p.file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.ends_with(".odm") || s.ends_with(".ddm") || s.ends_with(".blv"))
                    .unwrap_or(false)
            {
                let _ = std::fs::remove_file(&p);
            }
        }
    }
    dump_readable_files(&assets, out_dir);
}

/// Try to decode raw (decompressed) bytes as an image.
/// Tries PCX (0x0A magic), LOD bitmap, and LOD sprite formats in order.
/// Returns None if none match.
fn try_decode_image(
    data: &[u8],
    palettes: Option<&openmm_data::assets::palette::Palettes>,
) -> Option<image::DynamicImage> {
    // PCX: first byte is 0x0A (manufacturer byte)
    if data.len() > 4
        && data[0] == 0x0A
        && let Some(img) = openmm_data::assets::pcx::decode(data)
    {
        return Some(img);
    }
    // LOD bitmap format
    if let Ok(img) = openmm_data::assets::image::Image::try_from(data)
        && let Ok(dyn_img) = img.to_image_buffer()
    {
        return Some(dyn_img);
    }
    // LOD sprite format (needs palettes)
    if let Some(palettes) = palettes
        && let Ok(img) = openmm_data::assets::image::Image::try_from((data, palettes))
        && let Ok(dyn_img) = img.to_image_buffer()
    {
        return Some(dyn_img);
    }
    None
}

fn dump_readable_files(assets: &Assets, out_dir: &Path) {
    let archives = assets.archives();
    for archive in archives {
        if let Some(files) = assets.files_in(&archive) {
            for file in files {
                let lower = file.to_lowercase();
                let mut out_content: Option<String> = None;

                if lower.ends_with(".odm") {
                    if let Ok(data) = openmm_data::odm::Odm::load(assets, &file) {
                        out_content = serde_json::to_string_pretty(&data).ok();
                    }
                } else if lower.ends_with(".ddm") {
                    if let Ok(data) = openmm_data::ddm::Ddm::load(assets, &file) {
                        out_content = serde_json::to_string_pretty(&data).ok();
                    }
                } else if lower.ends_with(".blv")
                    && let Ok(data) = openmm_data::blv::Blv::load(assets, &file)
                {
                    out_content = serde_json::to_string_pretty(&data).ok();
                }

                if let Some(content) = out_content {
                    let out_path = out_dir.join(format!("{}.json", file));
                    let _ = fs::write(&out_path, content);
                }
            }
        }
    }
}

/// Minimal JSON serializer — avoids adding serde as a dependency.
fn serde_json_minimal(data: &BTreeMap<String, Vec<String>>) -> String {
    let mut out = String::from("{\n");
    let entries: Vec<_> = data.iter().collect();
    for (i, (key, files)) in entries.iter().enumerate() {
        out.push_str(&format!("  \"{}\": [\n", key));
        for (j, file) in files.iter().enumerate() {
            out.push_str(&format!("    \"{}\"", file));
            if j < files.len() - 1 {
                out.push(',');
            }
            out.push('\n');
        }
        out.push_str("  ]");
        if i < entries.len() - 1 {
            out.push(',');
        }
        out.push('\n');
    }
    out.push('}');
    out
}
