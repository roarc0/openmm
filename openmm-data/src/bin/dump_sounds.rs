use std::fs;
use std::path::Path;

use log::{error, info};
use openmm_data::snd::SndExt;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let data_path = openmm_data::get_data_path();
    let base = Path::new(&data_path);

    let snd_path = [base.join("../Sounds/Audio.snd"), base.join("Sounds/Audio.snd")]
        .into_iter()
        .find(|p| p.exists())
        .expect("Audio.snd not found");

    info!("Opening {:?}", snd_path);
    let archive = openmm_data::assets::snd::SndArchive::open(&snd_path).expect("Failed to open Audio.snd");

    let output_dir = Path::new("data/dump/sounds");
    fs::create_dir_all(output_dir).expect("Failed to create output directory");

    let mut names: Vec<String> = archive.list();
    names.sort();

    let mut count = 0;
    for name in &names {
        if let Some(wav_bytes) = archive.get(name) {
            let filename = format!("{}.wav", name);
            let path = output_dir.join(&filename);
            if let Err(e) = fs::write(&path, &wav_bytes) {
                error!("Failed to write {}: {}", filename, e);
            } else {
                count += 1;
            }
        } else {
            error!("Failed to get sound data for {}", name);
        }
    }

    info!("Extracted {} sound files to {}", count, output_dir.display());
}
