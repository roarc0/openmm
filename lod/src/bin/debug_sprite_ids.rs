use lod::{LodManager, ddm::Ddm, dsft::DSFT, monlist::MonsterList};

fn main() {
    let lod_manager = LodManager::new(lod::get_lod_path()).unwrap();
    let ddm = Ddm::new(&lod_manager, "oute3.odm").unwrap();
    let dsft = DSFT::new(&lod_manager).unwrap();
    let monlist = MonsterList::new(&lod_manager).unwrap();

    println!("=== DDM actors after 1-indexed fix ===\n");
    let mut seen = std::collections::HashSet::new();
    for (i, actor) in ddm.actors.iter().enumerate() {
        if !seen.insert(actor.monlist_id) && i > 5 {
            continue;
        }
        let mid = actor.monlist_id as usize;
        let (name, sprite) = if mid < monlist.monsters.len() {
            let d = &monlist.monsters[mid];
            // Resolve through DSFT
            let mut root = String::new();
            let mut pal: i16 = 0;
            for frame in &dsft.frames {
                if let Some(gname) = frame.group_name()
                    && gname.eq_ignore_ascii_case(&d.sprite_names[0])
                {
                    if let Some(sname) = frame.sprite_name() {
                        let without_digits = sname.trim_end_matches(|c: char| c.is_ascii_digit());
                        root = if without_digits.len() > 1 {
                            let last = without_digits.as_bytes()[without_digits.len() - 1];
                            if (b'a'..=b'f').contains(&last) {
                                without_digits[..without_digits.len() - 1].to_lowercase()
                            } else {
                                without_digits.to_lowercase()
                            }
                        } else {
                            without_digits.to_lowercase()
                        };
                        pal = frame.palette_id;
                    }
                    break;
                }
            }
            (d.internal_name.as_str(), format!("{} (pal {})", root, pal))
        } else {
            ("OUT OF RANGE", String::new())
        };
        println!("Actor {:2}: monlist_id={:3} -> {:15} -> {}", i, mid, name, sprite);
    }
}
