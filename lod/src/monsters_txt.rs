//! Parser for monsters.txt — per-variant monster stats.
//!
//! Tab-separated, 3 header lines (group labels / column names / blank), then one row per variant.
//! Uses the `csv` crate so quoted fields (e.g. `"1,131"`, `"Fireball,N,5"`) are handled correctly.
//!
//! Column layout (0-indexed after the `#` row-number column):
//!   0:#  1:Picture  2:Name  3:LVL  4:HP  5:AC  6:EXP  7:Treasure  8:Quest
//!   9:Fly  10:Move  11:AIType  12:Hst  13:Spd  14:Rec  15:Pref  16:Bonus
//!   17:Atk1Type  18:Atk1Dmg  19:Atk1Miss  20:Atk1%
//!   21:Atk2Type  22:Atk2Dmg  23:Atk2Miss  24:Atk2%
//!   25:Spell  26:Fire  27:Elec  28:Cold  29:Pois  30:Phys  31:Mag  32:Special

use std::collections::HashMap;
use std::error::Error;
use std::io::Cursor;

use csv::ReaderBuilder;

use crate::LodManager;

/// All per-variant stats for one monster from monsters.txt.
#[derive(Debug, Clone)]
pub struct MonsterStats {
    /// Display name shown in game (e.g. "Archer", "Hobgoblin"). Column 2.
    pub display_name: String,
    /// Monster level. Column 3.
    pub level: u8,
    /// Max hit points. Column 4.
    pub hp: i16,
    /// Armor class. Column 5.
    pub armor_class: i16,
    /// Experience awarded on kill. Column 6.
    pub experience: i32,
    /// Treasure formula string (e.g. "5%3D20+L1Bow"). Column 7.
    pub treasure: String,
    /// Quest item ID carried (0 = none). Column 8.
    pub quest_item: u8,
    /// Whether this monster can fly. Column 9 ("Y"/"N").
    pub can_fly: bool,
    /// Movement type: "Short", "Long", "Global", "StaticMelee", "StaticRanged". Column 10.
    pub move_type: String,
    /// AI behaviour: "Normal", "Aggress", "Wimp", "Berserk", "Suicidal". Column 11.
    pub ai_type: String,
    /// Hostile type (how quickly monster aggros). Column 12.
    pub hostile_type: u8,
    /// Attack speed in MM6 ticks. Column 13.
    pub speed: u16,
    /// Attack recovery in MM6 ticks. Column 14.
    pub recovery: u16,
    /// Preferred target class (0 = any). Column 15.
    pub preferred_target: u8,
    /// Bonus attack effect code (disease, paralyse, etc.). Column 16.
    pub bonus_type: u8,
    /// Attack 1 damage type ("Phys", "Fire", "Cold", …). Column 17.
    pub atk1_type: String,
    /// Attack 1 damage formula ("1D6+1"). Column 18.
    pub atk1_damage: String,
    /// Attack 1 missile name ("Arrow", "0" = melee). Column 19.
    pub atk1_missile: String,
    /// Attack 1 use chance (always 0 for the primary attack). Column 20.
    pub atk1_chance: u8,
    /// Attack 2 damage type. Column 21.
    pub atk2_type: String,
    /// Attack 2 damage formula. Column 22.
    pub atk2_damage: String,
    /// Attack 2 missile name. Column 23.
    pub atk2_missile: String,
    /// Percent chance to use attack 2 instead of attack 1. Column 24.
    pub atk2_chance: u8,
    /// Spell cast info: "SpellName,Mastery,SkillLevel" or "0". Column 25.
    pub spell: String,
    /// Fire resistance (0-200, 200 = immune). Column 26.
    pub resist_fire: u8,
    /// Electricity resistance. Column 27.
    pub resist_elec: u8,
    /// Cold resistance. Column 28.
    pub resist_cold: u8,
    /// Poison resistance. Column 29.
    pub resist_poison: u8,
    /// Physical resistance. Column 30.
    pub resist_phys: u8,
    /// Magic resistance. Column 31.
    pub resist_magic: u8,
    /// Special ability string (e.g. "Explode", "Summon"). Column 32.
    pub special: String,
}

/// All monster stats keyed by full internal name (e.g. "GoblinA").
pub struct MonstersTxt {
    entries: HashMap<String, MonsterStats>,
}

impl MonstersTxt {
    pub fn new(lod: &LodManager) -> Result<Self, Box<dyn Error>> {
        let raw = lod.try_get_bytes("icons/monsters.txt")?;
        let data = match crate::lod_data::LodData::try_from(raw) {
            Ok(d) => d.data,
            Err(_) => raw.to_vec(),
        };
        let text = String::from_utf8_lossy(&data);
        Self::parse(&text)
    }

    fn parse(text: &str) -> Result<Self, Box<dyn Error>> {
        // Skip the 3 header lines (group labels, column names, blank separator).
        let body: String = text.lines().skip(3).collect::<Vec<_>>().join("\n");

        let mut rdr = ReaderBuilder::new()
            .delimiter(b'\t')
            .has_headers(false)
            .flexible(true)
            .from_reader(Cursor::new(body.as_bytes()));

        let mut entries = HashMap::new();

        for result in rdr.records() {
            let rec = result?;

            // Col 1: internal name (e.g. "GoblinA"). Skip blank/category marker rows.
            let internal = rec.get(1).unwrap_or("").trim();
            if internal.len() < 2 {
                continue;
            }

            // Col 2: display name. Skip rows with no display name.
            let display = rec.get(2).unwrap_or("").trim();
            if display.is_empty() {
                continue;
            }

            let stats = MonsterStats {
                display_name: display.to_string(),
                level: col_u8(&rec, 3),
                hp: col_i16(&rec, 4),
                armor_class: col_i16(&rec, 5),
                experience: col_i32(&rec, 6),
                treasure: col_str(&rec, 7),
                quest_item: col_u8(&rec, 8),
                can_fly: rec.get(9).unwrap_or("").trim().eq_ignore_ascii_case("y"),
                move_type: col_str(&rec, 10),
                ai_type: col_str(&rec, 11),
                hostile_type: col_u8(&rec, 12),
                speed: col_u16(&rec, 13),
                recovery: col_u16(&rec, 14),
                preferred_target: col_u8(&rec, 15),
                bonus_type: col_u8(&rec, 16),
                atk1_type: col_str(&rec, 17),
                atk1_damage: col_str(&rec, 18),
                atk1_missile: col_str(&rec, 19),
                atk1_chance: col_u8(&rec, 20),
                atk2_type: col_str(&rec, 21),
                atk2_damage: col_str(&rec, 22),
                atk2_missile: col_str(&rec, 23),
                atk2_chance: col_u8(&rec, 24),
                spell: col_str(&rec, 25),
                resist_fire: col_u8(&rec, 26),
                resist_elec: col_u8(&rec, 27),
                resist_cold: col_u8(&rec, 28),
                resist_poison: col_u8(&rec, 29),
                resist_phys: col_u8(&rec, 30),
                resist_magic: col_u8(&rec, 31),
                special: col_str(&rec, 32),
            };

            entries.insert(internal.to_string(), stats);
        }

        Ok(MonstersTxt { entries })
    }

    /// Look up all stats for a specific monster variant.
    ///
    /// `prefix` is the dmonlist prefix (e.g. "Goblin"), `variant` 1=A/2=B/3=C.
    pub fn get(&self, prefix: &str, variant: u8) -> Option<&MonsterStats> {
        self.entries.get(&self.key(prefix, variant))
    }

    /// Look up the display name for a specific monster variant.
    pub fn display_name(&self, prefix: &str, variant: u8) -> Option<&str> {
        self.get(prefix, variant).map(|e| e.display_name.as_str())
    }

    /// Look up the max HP for a specific monster variant.
    pub fn max_hp(&self, prefix: &str, variant: u8) -> Option<i16> {
        self.get(prefix, variant).map(|e| e.hp)
    }

    fn key(&self, prefix: &str, variant: u8) -> String {
        let suffix = match variant {
            1 => "A",
            2 => "B",
            _ => "C",
        };
        format!("{}{}", prefix, suffix)
    }
}

// ── column helpers ────────────────────────────────────────────────────────────

fn col_str(rec: &csv::StringRecord, i: usize) -> String {
    rec.get(i).unwrap_or("").trim().to_string()
}

fn col_u8(rec: &csv::StringRecord, i: usize) -> u8 {
    // Strip commas from numbers like "1,131" before parsing.
    rec.get(i).unwrap_or("").replace(',', "").trim().parse().unwrap_or(0)
}

fn col_u16(rec: &csv::StringRecord, i: usize) -> u16 {
    rec.get(i).unwrap_or("").replace(',', "").trim().parse().unwrap_or(0)
}

fn col_i16(rec: &csv::StringRecord, i: usize) -> i16 {
    rec.get(i).unwrap_or("").replace(',', "").trim().parse().unwrap_or(0)
}

fn col_i32(rec: &csv::StringRecord, i: usize) -> i32 {
    rec.get(i).unwrap_or("").replace(',', "").trim().parse().unwrap_or(0)
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_lod;

    #[test]
    fn goblin_a_all_stats() {
        let Some(lod) = test_lod() else { return };
        let txt = MonstersTxt::new(&lod).unwrap();
        let g = txt.get("Goblin", 1).expect("GoblinA must exist");
        assert_eq!(g.display_name, "Goblin");
        assert!(g.hp > 0);
        assert!(g.level > 0);
        assert!(g.experience > 0);
        assert!(!g.atk1_damage.is_empty());
    }

    #[test]
    fn archer_c_has_quoted_exp_and_spell() {
        // ArcherC EXP is "1,131" (quoted with comma) and spell is "Fireball,N,5".
        let Some(lod) = test_lod() else { return };
        let txt = MonstersTxt::new(&lod).unwrap();
        let a = txt.get("Archer", 3).expect("ArcherC must exist");
        assert_eq!(a.experience, 1131, "quoted comma-number must parse correctly");
        assert_eq!(a.spell, "Fireball,N,5");
    }

    #[test]
    fn peasant_m2_variants_have_distinct_names() {
        let Some(lod) = test_lod() else { return };
        let txt = MonstersTxt::new(&lod).unwrap();
        assert_eq!(txt.display_name("PeasantM2", 1), Some("Apprentice Mage"));
        assert_eq!(txt.display_name("PeasantM2", 2), Some("Journeyman Mage"));
        assert_eq!(txt.display_name("PeasantM2", 3), Some("Mage"));
    }

    #[test]
    fn resistances_are_loaded() {
        let Some(lod) = test_lod() else { return };
        let txt = MonstersTxt::new(&lod).unwrap();
        // ArcherA has 10 resistance across Fire/Elec/Cold/Pois (row 1 of data).
        let a = txt.get("Archer", 1).expect("ArcherA must exist");
        assert_eq!(a.resist_fire, 10);
        assert_eq!(a.resist_elec, 10);
        assert_eq!(a.resist_cold, 10);
        assert_eq!(a.resist_poison, 10);
        assert_eq!(a.resist_phys, 0);
        assert_eq!(a.resist_magic, 0);
    }

    #[test]
    fn unknown_monster_returns_none() {
        let Some(lod) = test_lod() else { return };
        let txt = MonstersTxt::new(&lod).unwrap();
        assert!(txt.display_name("NonExistentXyz", 1).is_none());
    }
}
