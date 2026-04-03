//! Parser for items.txt — item definitions from the icons LOD.
//!
//! Tab-separated text file with 2 header lines, then one row per item.
//! Column layout (0-indexed):
//!   0: Item#, 1: PicFile, 2: Name, 3: Value, 4: EquipStat, 5: SkillGroup,
//!   6: Mod1, 7: Mod2, 8: Material, 9: ID/Rep/St, 10: NotIdentifiedName,
//!   11: SpriteIndex, 12: Shape, 13: EquipX, 14: EquipY, 15: Notes/Description

use std::error::Error;

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
        let mut items = Vec::new();
        // Skip 2 header lines (blank + column headers)
        for line in text.lines().skip(2) {
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() < 12 {
                continue;
            }
            let id: u16 = match cols[0].trim().parse() {
                Ok(v) => v,
                Err(_) => continue,
            };
            if id == 0 {
                continue;
            }

            let pic_file = cols.get(1).unwrap_or(&"").trim().to_string();
            let name = cols.get(2).unwrap_or(&"").trim().to_string();
            if name.is_empty() {
                continue;
            }

            let value: u32 = cols.get(3).unwrap_or(&"0").trim().parse().unwrap_or(0);
            let equip_stat = cols.get(4).unwrap_or(&"").trim().to_string();
            let skill_group = cols.get(5).unwrap_or(&"").trim().to_string();
            let mod1 = cols.get(6).unwrap_or(&"0").trim().to_string();
            let mod2: i32 = cols.get(7).unwrap_or(&"0").trim().parse().unwrap_or(0);
            let material: u8 = cols.get(8).unwrap_or(&"0").trim().parse().unwrap_or(0);
            let id_rep_st: u8 = cols.get(9).unwrap_or(&"0").trim().parse().unwrap_or(0);
            let not_identified_name = cols.get(10).unwrap_or(&"").trim().to_string();
            let sprite_index: u16 = cols.get(11).unwrap_or(&"0").trim().parse().unwrap_or(0);
            let shape: u8 = cols.get(12).unwrap_or(&"0").trim().parse().unwrap_or(0);
            let equip_x: u8 = cols.get(13).unwrap_or(&"0").trim().parse().unwrap_or(0);
            let equip_y: u8 = cols.get(14).unwrap_or(&"0").trim().parse().unwrap_or(0);
            let notes = cols
                .get(15)
                .unwrap_or(&"")
                .trim()
                .trim_matches('"')
                .to_string();

            items.push(ItemInfo {
                id,
                pic_file,
                name,
                value,
                equip_stat,
                skill_group,
                mod1,
                mod2,
                material,
                id_rep_st,
                not_identified_name,
                sprite_index,
                shape,
                equip_x,
                equip_y,
                notes,
            });
        }
        Ok(ItemsTable { items })
    }

    /// Look up an item by its 1-based ID.
    pub fn get(&self, id: u16) -> Option<&ItemInfo> {
        self.items.iter().find(|i| i.id == id)
    }
}
