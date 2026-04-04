//! Parser for items.txt — item definitions from the icons LOD.
//!
//! Tab-separated text file with 2 header lines, then one row per item.
//! Column layout (0-indexed):
//!   0: Item#, 1: PicFile, 2: Name, 3: Value, 4: EquipStat, 5: SkillGroup,
//!   6: Mod1, 7: Mod2, 8: Material, 9: ID/Rep/St, 10: NotIdentifiedName,
//!   11: SpriteIndex, 12: Shape, 13: EquipX, 14: EquipY, 15: Notes/Description

use std::error::Error;
use std::io::Cursor;

use csv::ReaderBuilder;

use crate::LodManager;

/// A single item definition from `items.txt`.
pub struct ItemInfo {
    /// 1-based item ID.
    pub id: u16,
    /// Picture/icon file name (without extension).
    pub pic_file: String,
    /// Display name.
    pub name: String,
    /// Base gold value.
    pub value: u32,
    /// Equip stat (e.g. "Weapon", "Armor", "Misc").
    pub equip_stat: String,
    /// Skill group (e.g. "Sword", "Axe", "Fire").
    pub skill_group: String,
    /// Primary damage/bonus modifier (e.g. "3d3").
    pub mod1: String,
    /// Secondary modifier (extra bonus).
    pub mod2: i32,
    /// Material number (equip stat flags).
    pub material: u8,
    /// Identification / reputation / status flags.
    pub id_rep_st: u8,
    /// Unidentified display name.
    pub not_identified_name: String,
    /// Sprite index into dsft.bin.
    pub sprite_index: u16,
    /// Shape index (inventory grid shape).
    pub shape: u8,
    /// Inventory/doll equip X position.
    pub equip_x: u8,
    /// Inventory/doll equip Y position.
    pub equip_y: u8,
    /// Flavour text / description.
    pub notes: String,
}

/// All item definitions.
pub struct ItemsTable {
    pub items: Vec<ItemInfo>,
}

impl ItemsTable {
    pub fn new(lod_manager: &LodManager) -> Result<Self, Box<dyn Error>> {
        let raw = lod_manager.try_get_bytes("icons/items.txt")?;
        let data = match crate::lod_data::LodData::try_from(raw) {
            Ok(d) => d.data,
            Err(_) => raw.to_vec(),
        };
        let text = String::from_utf8_lossy(&data);
        Self::parse(&text)
    }

    fn parse(text: &str) -> Result<Self, Box<dyn Error>> {
        let body: String = text.lines().skip(2).collect::<Vec<_>>().join("\n");
        let mut rdr = ReaderBuilder::new()
            .delimiter(b'\t')
            .has_headers(false)
            .flexible(true)
            .from_reader(Cursor::new(body.as_bytes()));

        let mut items = Vec::new();
        for result in rdr.records() {
            let rec = result?;

            let id: u16 = match rec.get(0).unwrap_or("").trim().parse() {
                Ok(v) => v,
                Err(_) => continue,
            };
            if id == 0 {
                continue;
            }

            let name = rec.get(2).unwrap_or("").trim().to_string();
            if name.is_empty() {
                continue;
            }

            items.push(ItemInfo {
                id,
                pic_file: rec.get(1).unwrap_or("").trim().to_string(),
                name,
                value: rec.get(3).unwrap_or("0").trim().parse().unwrap_or(0),
                equip_stat: rec.get(4).unwrap_or("").trim().to_string(),
                skill_group: rec.get(5).unwrap_or("").trim().to_string(),
                mod1: rec.get(6).unwrap_or("0").trim().to_string(),
                mod2: rec.get(7).unwrap_or("0").trim().parse().unwrap_or(0),
                material: rec.get(8).unwrap_or("0").trim().parse().unwrap_or(0),
                id_rep_st: rec.get(9).unwrap_or("0").trim().parse().unwrap_or(0),
                not_identified_name: rec.get(10).unwrap_or("").trim().to_string(),
                sprite_index: rec.get(11).unwrap_or("0").trim().parse().unwrap_or(0),
                shape: rec.get(12).unwrap_or("0").trim().parse().unwrap_or(0),
                equip_x: rec.get(13).unwrap_or("0").trim().parse().unwrap_or(0),
                equip_y: rec.get(14).unwrap_or("0").trim().parse().unwrap_or(0),
                notes: rec.get(15).unwrap_or("").trim().to_string(),
            });
        }
        Ok(ItemsTable { items })
    }

    /// Look up an item by its 1-based ID.
    pub fn get(&self, id: u16) -> Option<&ItemInfo> {
        self.items.iter().find(|i| i.id == id)
    }
}
