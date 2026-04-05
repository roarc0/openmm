use std::{
    fs,
    path::{Path, PathBuf},
};

use lod::{
    dchest::ChestList,
    ddeclist::DDecList,
    dpft::PFT,
    dsft::DSFT,
    dsounds::DSounds,
    get_data_path,
    items::ItemsTable,
    lod::Lod,
    mapstats::MapStats,
    monlist::MonsterList,
    odm::Odm,
    ddm::Ddm,
    snd::{SndArchive, SndWriter},
    vid::{Vid, VidWriter},
    LodSerialise, LodWriter,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let src_root = PathBuf::from(get_data_path());
    let dst_root = PathBuf::from("./data/mm6_serialized");

    if !src_root.exists() {
        eprintln!(
            "Source root {} not found. Set OPENMM_6_PATH?",
            src_root.display()
        );
        return Ok(());
    }

    println!("Mirroring {} to {}", src_root.display(), dst_root.display());
    fs::create_dir_all(&dst_root)?;

    mirror_dir(&src_root, &dst_root)?;

    println!("\nRound-trip complete.");
    Ok(())
}

fn mirror_dir(src: &Path, dst: &Path) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let rel = path.strip_prefix(src)?;
        let target = dst.join(rel);

        if path.is_dir() {
            let dir_name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase();
            if dir_name.ends_with("_lod") || dir_name.ends_with("_vid") || dir_name.ends_with("_snd") {
                continue;
            }
            // We don't create the directory here; process_* will create it if needed.
            mirror_dir(&path, &target)?;
        } else {
            let ext = path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_lowercase();
            match ext.as_str() {
                "lod" => {
                    fs::create_dir_all(target.parent().unwrap())?;
                    process_lod(&path, &target)?;
                }
                // "snd" => {
                //     fs::create_dir_all(target.parent().unwrap())?;
                //     process_snd(&path, &target)?;
                // }
                "vid" => {
                    fs::create_dir_all(target.parent().unwrap())?;
                    process_vid(&path, &target)?;
                }
                _ => {
                    // Skip everything else (no copying)
                }
            }
        }
    }
    Ok(())
}

fn process_lod(src: &Path, dst: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let name = src.file_name().unwrap().to_string_lossy();
    print!("  LOD: {:<15} ", name);

    let lod = Lod::open(src)?;
    let mut writer = LodWriter::new();
    let mut count = 0;
    let mut re_serialized = 0;
    let mut mismatches = 0;

    for (entry_name, data) in lod.entries() {
        count += 1;
        let lower = entry_name.to_lowercase();

        // Decompress metadata if needed for potential modification.
        // We now store the original compression kind in LodData.
        let mut lod_data = lod::raw::lod_data::LodData::try_from(data.as_slice()).unwrap();

        let res = match lower.as_str() {
            "mapstats.txt" => MapStats::parse(&String::from_utf8_lossy(&lod_data.data))
                .ok()
                .map(|p| p.to_bytes()),
            "items.txt" => ItemsTable::parse(&String::from_utf8_lossy(&lod_data.data))
                .ok()
                .map(|p| p.to_bytes()),
            "dsft.bin" => DSFT::try_from(lod_data.data.as_slice()).ok().map(|p| p.to_bytes()),
            "ddeclist.bin" => DDecList::try_from(lod_data.data.as_slice()).ok().map(|p| p.to_bytes()),
            "dsounds.bin" => DSounds::try_from(lod_data.data.as_slice()).ok().map(|p| p.to_bytes()),
            "dpft.bin" => PFT::try_from(lod_data.data.as_slice()).ok().map(|p| p.to_bytes()),
            "dchest.bin" => ChestList::try_from(lod_data.data.as_slice()).ok().map(|p| p.to_bytes()),
            "dmonlist.bin" => MonsterList::try_from(lod_data.data.as_slice()).ok().map(|p| p.to_bytes()),
            _ if lower.ends_with(".odm") => Odm::try_from(lod_data.data.as_slice()).ok().map(|p| p.to_bytes()),
            _ if lower.ends_with(".ddm") => Ddm::try_from(lod_data.data.as_slice()).ok().map(|p| p.to_bytes()),
            _ => None,
        };

        if let Some(serialized) = res {
            re_serialized += 1;
            if serialized != lod_data.data {
                mismatches += 1;
            }
            lod_data.data = serialized;
            // pack() restores original zlib compression (if any)
            writer.add_file(entry_name, lod_data.pack());
        } else {
            // NO CHEATING: skip files we haven't successfully parsed and re-serialized.
            // This ensures only "OpenMM-Verified" files end up in the output LOD.
        }
    }

    writer.save(dst)?;
    if mismatches > 0 {
        println!(
            "DONE ({} entries, re-serialized {}, {} bit-mismatches)",
            count, re_serialized, mismatches
        );
    } else {
        println!(
            "DONE ({} entries, re-serialized {}, all bit-perfect)",
            count, re_serialized
        );
    }
    Ok(())
}

// fn process_snd(src: &Path, dst: &Path) -> Result<(), Box<dyn std::error::Error>> {
//     let name = src.file_name().unwrap().to_string_lossy();
//     print!("  SND: {:<15} ", name);
//     let archive = SndArchive::open(src)?;
//     let mut writer = SndWriter::new();
//     let entries = archive.list();
//     for name in &entries {
//         if let Some((data, decomp_size)) = archive.get_raw(name) {
//             writer.add(name, data, decomp_size);
//         }
//     }
//     writer.save(dst)?;
//     println!("DONE ({} sounds)", entries.len());
//     Ok(())
// }

fn process_vid(src: &Path, dst: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let name = src.file_name().unwrap().to_string_lossy();
    print!("  VID: {:<15} ", name);
    let vid = Vid::open(src)?;
    let mut writer = VidWriter::new();
    for i in 0..vid.entries.len() {
        let entry = &vid.entries[i];
        writer.add(&entry.name, vid.smk_bytes(i).to_vec());
    }
    writer.save(&dst)?;
    println!("DONE ({} videos)", vid.entries.len());
    Ok(())
}
