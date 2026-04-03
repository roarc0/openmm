use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use lod::LodManager;

fn main() {
    let lod_path = lod::get_lod_path();
    let lod_manager = LodManager::new(&lod_path).expect("failed to open LOD files");

    let out_dir = Path::new("assets");
    fs::create_dir_all(out_dir).expect("failed to create assets directory");

    // Build asset index
    let mut assets: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut archives = lod_manager.archives();
    archives.sort();

    let mut total = 0;
    for archive in &archives {
        if let Some(mut files) = lod_manager.files_in(archive) {
            files.sort();
            total += files.len();
            assets.insert(archive.to_string(), files.iter().map(|s| s.to_string()).collect());
        }
    }

    // Write assets.json
    let json = serde_json_minimal(&assets);
    let json_path = out_dir.join("assets.json");
    fs::write(&json_path, &json).expect("failed to write assets.json");
    println!(
        "Wrote {} ({} archives, {} files total)",
        json_path.display(),
        archives.len(),
        total
    );

    for (archive, files) in &assets {
        println!("  {}: {} files", archive, files.len());
    }

    // Dump all files (images as PNG, data as raw)
    println!("\nDumping files to {}/ ...", out_dir.display());
    match lod_manager.dump_all(out_dir) {
        Ok(()) => println!("Done."),
        Err(e) => eprintln!("Error during dump: {}", e),
    }

    // Dump readable versions
    println!("\nDumping readable text files (*.txt) ...");
    dump_readable_files(&lod_manager, out_dir);
}

fn dump_readable_files(lod_manager: &LodManager, out_dir: &Path) {
    let archives = lod_manager.archives();
    for archive in archives {
        if let Some(files) = lod_manager.files_in(archive) {
            for file in files {
                let lower = file.to_lowercase();
                let mut out_content = None;

                if lower.ends_with(".odm") {
                    if let Ok(data) = lod::odm::Odm::new(lod_manager, file) {
                        out_content = Some(format!("{:#?}", data));
                    }
                } else if lower.ends_with(".ddm") {
                    if let Ok(data) = lod::ddm::Ddm::new(lod_manager, file) {
                        out_content = Some(format!("{:#?}", data));
                    }
                } else if lower.ends_with(".blv")
                    && let Ok(data) = lod::blv::Blv::new(lod_manager, file)
                {
                    out_content = Some(format!("{:#?}", data));
                }

                if let Some(content) = out_content {
                    let out_path = out_dir.join(format!("{}.txt", file));
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
