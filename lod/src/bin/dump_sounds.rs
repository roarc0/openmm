use std::fs;
use std::path::Path;

fn main() {
    let data_path = lod::get_data_path();
    let base = Path::new(&data_path);

    let snd_path = [base.join("../Sounds/Audio.snd"), base.join("Sounds/Audio.snd")]
        .into_iter()
        .find(|p| p.exists())
        .expect("Audio.snd not found");

    println!("Opening {:?}", snd_path);
    let archive = lod::snd::SndArchive::open(&snd_path).expect("Failed to open Audio.snd");

    let output_dir = Path::new("assets/sounds");
    fs::create_dir_all(output_dir).expect("Failed to create output directory");

    let mut names: Vec<&str> = archive.list();
    names.sort();

    let mut count = 0;
    for name in &names {
        if let Some(wav_bytes) = archive.get(name) {
            let filename = format!("{}.wav", name);
            let path = output_dir.join(&filename);
            fs::write(&path, &wav_bytes).unwrap_or_else(|_| panic!("Failed to write {}", filename));
            count += 1;
        }
    }

    println!("Extracted {} sound files to {}", count, output_dir.display());
}
