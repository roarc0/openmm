fn main() {
    let assets = openmm_data::Assets::new(openmm_data::get_data_path()).unwrap();
    let out = std::path::Path::new("/tmp/hud_assets");
    std::fs::create_dir_all(out).unwrap();
    let names = &[
        "buttcast", "buttcamp", "buttmenu", "buttref", "compass", "mapbordr", "facemask", "bardata", "mhp_bg",
        "mhp_grn", "mhp_red", "mhp_yel", "npc001", "npc002", "npc003", "npc004", "ibground", "tap1", "tap2", "tap3",
        "tap4",
    ];
    for name in names {
        if let Some(img) = assets.lod().icon(name) {
            img.save(out.join(format!("{}.png", name))).unwrap();
            println!("{}: {}x{}", name, img.width(), img.height());
        } else {
            println!("{}: FAILED", name);
        }
    }
}
