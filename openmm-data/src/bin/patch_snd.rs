use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new("data/mm6/Sounds/Audio.snd");
    if !path.exists() {
        eprintln!("Audio.snd not found at {}", path.display());
        return Ok(());
    }

    // Step 1: Backup
    let backup_path = path.with_extension("snd.bak");
    if !backup_path.exists() {
        println!("Creating backup at {}", backup_path.display());
        std::fs::copy(path, &backup_path)?;
    }

    // Step 2: Open for R/W
    let mut file = OpenOptions::new().read(true).write(true).open(path)?;

    // Step 3: Verify current state at 0x1274
    // 0x1274 is the offset field for Index 90 (29_02)
    // 4 + 90 * 52 + 40 = 4684 + 40 = 4724 = 0x1274
    let patch_offset = 0x1274;
    file.seek(SeekFrom::Start(patch_offset))?;

    let mut current = [0u8; 4];
    file.read_exact(&mut current)?;

    let current_val = u32::from_le_bytes(current);
    let target_val = 0x0f9583;

    if current_val == target_val {
        println!("Audio.snd is already patched (offset is 0x{:06x})", current_val);
        return Ok(());
    }

    if current_val != 0x0f6683 {
        eprintln!(
            "Warning: Unexpected offset at 0x{:x}: 0x{:06x} (expected 0x0f6683)",
            patch_offset, current_val
        );
        println!("Proceeding with patch anyway as requested...");
    }

    // Step 4: Write the new offset
    println!(
        "Patching Audio.snd at 0x{:x}: 0x{:06x} -> 0x{:06x}",
        patch_offset, current_val, target_val
    );
    file.seek(SeekFrom::Start(patch_offset))?;
    file.write_all(&target_val.to_le_bytes())?;

    println!("Patch successful!");

    Ok(())
}
