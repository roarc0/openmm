/// Dump raw time-related bytes from party.bin in a save file.
/// Usage: cargo run --example dump_party_time [path_to_lod]
use openmm_data::save::file::SaveFile;

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "data/mm6/Saves/save001.mm6".to_string());

    let save = SaveFile::open(&path).expect("open save LOD");
    let data = save.get_file("party.bin").expect("party.bin missing");

    println!("party.bin size: {} bytes", data.len());

    // OpenEnroth Party_MM7 struct: timePlayed is i64 at offset 0x2C
    let time_i64 = i64::from_le_bytes([
        data[0x2C], data[0x2D], data[0x2E], data[0x2F],
        data[0x30], data[0x31], data[0x32], data[0x33],
    ]);
    println!("\n=== 0x2C timePlayed (i64, OpenEnroth) ===");
    println!(
        "raw: {:02X} {:02X} {:02X} {:02X}  {:02X} {:02X} {:02X} {:02X}",
        data[0x2C], data[0x2D], data[0x2E], data[0x2F],
        data[0x30], data[0x31], data[0x32], data[0x33],
    );
    println!("i64 value: {}", time_i64);

    // Decode via OpenEnroth formula: ticks -> game seconds -> calendar
    // TICKS_PER_REALTIME_SECOND=128, GAME_SECONDS_IN_REALTIME_SECOND=30
    let game_seconds = time_i64 * 30 / 128;
    let minutes = game_seconds / 60;
    let hours = minutes / 60;
    let days = hours / 24;
    let weeks = days / 7;
    let months = weeks / 4;
    let years = months / 12;

    let start_year: i64 = 1168;
    println!("\n=== Decoded (startYear={}) ===", start_year);
    println!("year:   {}", start_year + years);
    println!("month:  {}", 1 + months % 12);
    println!("week:   {}", 1 + weeks % 4);
    println!("day:    {}", 1 + days % 28);
    println!("hour:   {}", hours % 24);
    println!("minute: {}", minutes % 60);
    println!("second: {}", game_seconds % 60);

    // What the current parser reads at 0xA0-0xB8
    println!("\n=== Current parser offsets (0xA0-0xB8, each i32) ===");
    let labels = ["year", "month", "week", "day", "hour", "minute", "second"];
    for (i, label) in labels.iter().enumerate() {
        let off = 0x00A0 + i * 4;
        let val = i32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
        println!("0x{:04X} {:8}: {} (0x{:08X})", off, label, val, val as u32);
    }

    // Hex dump 0x00..0x100
    println!("\n=== Raw hex 0x00-0xFF ===");
    for row in 0..16 {
        let off = row * 16;
        print!("0x{:04X}: ", off);
        for col in 0..16 {
            print!("{:02X} ", data[off + col]);
        }
        println!();
    }
}
