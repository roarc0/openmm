//! Dumps all oute3.odm spawn points to assets/oute3_spawns.txt for investigation.
//!
//! Output format (one line per spawn):
//!   spawn[N] pos=(x,y,z) index=I attrs=A raw=[...20 bytes...] => "Mon slot=S fv=V seed=K range=L-H gsz=G cv=C"
//!
//! Fields:
//!   fv   = forced_variant (0 = use difficulty; 1-3 = forced A/B/C)
//!   seed = |x|+|y| used for group size calculation
//!   range= MonNLow-MonNHi from mapstats
//!   gsz  = computed group size = Low + (seed % (Hi-Low+1))
//!   cv   = champion variant (difficulty[slot] when fv=0)

use lod::{LodManager, game::global::GameData, lod_data::LodData, odm::Odm};
use std::{fs, io::Write};

fn main() {
    let lod = LodManager::new(lod::get_lod_path()).unwrap();
    let gd = GameData::new(&lod).unwrap();
    let map_name = "oute3.odm";
    let odm = Odm::new(&lod, map_name).unwrap();
    let cfg = gd.mapstats.get(map_name).expect("oute3.odm not in mapstats");

    // Read raw bytes to extract per-spawn raw bytes
    let raw = lod.try_get_bytes(format!("games/{}", map_name)).unwrap();
    let data = match LodData::try_from(raw) {
        Ok(d) => d.data,
        Err(_) => raw.to_vec(),
    };
    let n = odm.spawn_points.len();
    let section_start = data.len() - (4 + n * 20);

    let out_path = "assets/oute3_spawns.txt";
    let mut f = fs::File::create(out_path).expect("failed to create output file");

    writeln!(f, "oute3.odm — {} spawn points", n).unwrap();
    writeln!(
        f,
        "mapstats: Mon1={} dif={} range={}-{} | Mon2={} dif={} range={}-{} | Mon3={} dif={} range={}-{}",
        cfg.monster_names[0],
        cfg.difficulty[0],
        cfg.encounter_min[0],
        cfg.encounter_max[0],
        cfg.monster_names[1],
        cfg.difficulty[1],
        cfg.encounter_min[1],
        cfg.encounter_max[1],
        cfg.monster_names[2],
        cfg.difficulty[2],
        cfg.encounter_min[2],
        cfg.encounter_max[2],
    )
    .unwrap();
    writeln!(f).unwrap();

    for (i, sp) in odm.spawn_points.iter().enumerate() {
        let off = section_start + 4 + i * 20;
        let raw_bytes: Vec<u8> = data[off..off + 20].to_vec();

        let info = cfg.monster_for_index(sp.monster_index).map(|(name, _, slot, fv)| {
            let seed = sp.position[0]
                .unsigned_abs()
                .wrapping_mul(sp.position[1].unsigned_abs());
            let (lo, hi) = cfg.count_range_for_slot(slot);
            let range = (hi - lo) as u32 + 1;
            let gsz = lo as usize + (seed % range) as usize;
            // Sample variant for member 0 (representative roll).
            let roll0 = (seed % 100) as u8;
            let cv0 = if fv == 0 {
                cfg.variant_from_roll(slot, roll0)
            } else {
                fv
            };
            format!(
                "{} slot={} fv={} seed={} range={}-{} gsz={} cv0={}",
                name, slot, fv, seed, lo, hi, gsz, cv0
            )
        });

        writeln!(
            f,
            "spawn[{:2}] pos=({},{},{}) index={:2} attrs={} raw={:?} => {:?}",
            i, sp.position[0], sp.position[1], sp.position[2], sp.monster_index, sp.attributes, raw_bytes, info
        )
        .unwrap();
    }

    println!("Wrote {}", out_path);
}
