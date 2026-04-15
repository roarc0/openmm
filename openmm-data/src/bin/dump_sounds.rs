use std::fs;
use std::path::Path;

use log::{debug, error, info};
use openmm_data::snd::SndExt;

/// Byte offset where embedded signatures are searched: after RIFF id, size field, and form FourCC.
fn riff_payload_scan_start(data: &[u8]) -> usize {
    if data.len() >= 12 && data.starts_with(b"RIFF") {
        12
    } else {
        0
    }
}

fn log_riff_and_music_diagnostics(name: &str, wav_bytes: &[u8]) {
    if wav_bytes.len() >= 12 && wav_bytes.starts_with(b"RIFF") {
        let riff_size = u32::from_le_bytes([wav_bytes[4], wav_bytes[5], wav_bytes[6], wav_bytes[7]]) as usize;
        let form = String::from_utf8_lossy(&wav_bytes[8..12]);
        info!(
            "Sound '{}' RIFF: size_field={} (0x{:x}) form='{}'",
            name, riff_size, riff_size, form
        );
        if &wav_bytes[8..12] != b"WAVE" {
            log::warn!("Sound '{}' RIFF form is not WAVE: '{}'", name, form);
        }
        let expected_len = riff_size.saturating_add(8);
        debug!(
            "Sound '{}' RIFF implied_total={} actual_len={}",
            name,
            expected_len,
            wav_bytes.len()
        );
        if expected_len != wav_bytes.len() {
            log::warn!(
                "Sound '{}' RIFF length mismatch: size_field implies {} bytes total, actual {}",
                name,
                expected_len,
                wav_bytes.len()
            );
        }
    }

    let scan_start = riff_payload_scan_start(wav_bytes);
    let payload = &wav_bytes[scan_start..];
    let magics = [b"MThd", b"XMID", b"XDIR", b"RMID"];
    for magic in magics {
        if let Some(pos) = payload.windows(magic.len()).position(|window| window == magic) {
            log::warn!(
                "Sound '{}' has music signature '{:?}' at offset 0x{:x}",
                name,
                String::from_utf8_lossy(magic),
                scan_start + pos
            );
        }
    }
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let data_path = openmm_data::get_data_path();
    let assets = openmm_data::Assets::new(&data_path).expect("Failed to load game assets");
    let archive = assets.get_snd("audio").expect("Audio.snd not found in loaded archives");

    let output_dir = Path::new("data/dump/sounds");
    fs::create_dir_all(output_dir).expect("Failed to create output directory");

    let mut names: Vec<String> = archive.list();
    names.sort();

    let mut count = 0;
    for name in &names {
        if let Some(wav_bytes) = archive.get(name) {
            log_riff_and_music_diagnostics(name, &wav_bytes);

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
