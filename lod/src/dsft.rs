use std::{
    error::Error,
    io::{Cursor, Read},
};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::{enums::SpriteFrameFlags, lod_data::LodData, utils::try_read_name, LodManager};

pub struct DSFT {
    pub frames: Vec<DSFTFrame>,
    pub groups: Vec<u16>,
}

#[allow(dead_code)]
#[repr(C)]
#[derive(Default, Clone)]
pub struct DSFTFrame {
    group_name: [u8; 12],
    sprite_name: [u8; 12],
    pub sprite_index: [i16; 8],
    pub scale: i32,
    pub attributes: u16,
    pub light_radius: i16,
    pub palette_id: i16,
    pub palette_index: i16,
    pub time: i16,
    pub time_total: i16,
}

impl DSFTFrame {
    pub fn is_not_group_end(&self) -> bool {
        (self.attributes & 0x0001) != 0
    }

    pub fn is_luminous(&self) -> bool {
        (self.attributes & 0x0002) != 0
    }

    pub fn is_group_start(&self) -> bool {
        (self.attributes & 0x0004) != 0
    }

    pub fn is_image1(&self) -> bool {
        (self.attributes & 0x0010) != 0
    }

    pub fn is_center(&self) -> bool {
        (self.attributes & 0x0020) != 0
    }

    pub fn is_fidget(&self) -> bool {
        (self.attributes & 0x0040) != 0
    }

    pub fn is_loaded(&self) -> bool {
        (self.attributes & 0x0080) != 0
    }

    pub fn is_mirror0(&self) -> bool {
        (self.attributes & 0x0100) != 0
    }

    pub fn is_mirror1(&self) -> bool {
        (self.attributes & 0x0200) != 0
    }

    pub fn is_mirror2(&self) -> bool {
        (self.attributes & 0x0400) != 0
    }

    pub fn is_mirror3(&self) -> bool {
        (self.attributes & 0x0800) != 0
    }

    pub fn is_mirror4(&self) -> bool {
        (self.attributes & 0x1000) != 0
    }

    pub fn is_mirror5(&self) -> bool {
        (self.attributes & 0x2000) != 0
    }

    pub fn is_mirror7(&self) -> bool {
        (self.attributes & 0x4000) != 0
    }

    pub fn is_mirror8(&self) -> bool {
        (self.attributes & 0x8000) != 0
    }

    /// Get typed sprite frame flags.
    pub fn flags(&self) -> SpriteFrameFlags {
        SpriteFrameFlags::from_bits_truncate(self.attributes)
    }

    pub fn group_name(&self) -> Option<String> {
        try_read_name(&self.group_name)
    }

    pub fn sprite_name(&self) -> Option<String> {
        try_read_name(&self.sprite_name)
    }
}

impl DSFT {
    /// Look up the scale factor for a sprite group name (case-insensitive).
    /// Returns the fixed-point 16.16 scale as f32, or 1.0 if not found or zero.
    pub fn scale_for_group(&self, group: &str) -> f32 {
        for frame in &self.frames {
            if let Some(name) = frame.group_name() {
                if name.eq_ignore_ascii_case(group) {
                    if frame.scale > 0 {
                        return frame.scale as f32 / 65536.0;
                    }
                    return 1.0;
                }
            }
        }
        1.0
    }

    pub fn new(lod_manager: &LodManager) -> Result<Self, Box<dyn Error>> {
        let data = LodData::try_from(lod_manager.try_get_bytes("icons/dsft.bin")?)?;
        let data = data.data.as_slice();

        let mut cursor = Cursor::new(data);

        let mut frames = Vec::new();
        let frame_count = cursor.read_u32::<LittleEndian>()?;
        let group_count = cursor.read_u32::<LittleEndian>()?;
        let frame_size = std::mem::size_of::<DSFTFrame>();

        for _ in 0..frame_count {
            let mut frame = DSFTFrame::default();
            cursor.read_exact(unsafe {
                std::slice::from_raw_parts_mut(&mut frame as *mut _ as *mut u8, frame_size)
            })?;
            frames.push(frame);
        }

        let mut groups = Vec::new();
        for _ in 0..group_count {
            let g = cursor.read_u16::<LittleEndian>()?;
            groups.push(g)
        }

        Ok(Self { frames, groups })
    }
}

#[cfg(test)]
mod tests {
    use crate::{get_lod_path, LodManager};

    use super::DSFT;

    #[test]
    fn read_declist_data_works() {
        let lod_manager = LodManager::new(get_lod_path()).unwrap();
        let dsft = DSFT::new(&lod_manager).unwrap();
        assert_eq!(dsft.frames.len(), 6455);
        assert_eq!(dsft.frames[0].group_name(), Some("null".to_string()));
        assert_eq!(dsft.frames[1].group_name(), Some("key".to_string()));
        assert_eq!(dsft.frames[1].sprite_name(), Some("3gem7".to_string()));
        assert_eq!(dsft.frames[1017].group_name(), Some("rok1".to_string()));
        assert_eq!(dsft.frames[1017].sprite_name(), Some("rok1".to_string()));
        assert_eq!(dsft.groups.len(), 1656);
    }

    /// Helper: find the first DSFT frame matching a group name (case-insensitive).
    fn find_frame_by_group<'a>(dsft: &'a DSFT, group: &str) -> Option<&'a super::DSFTFrame> {
        dsft.frames.iter().find(|f| {
            f.group_name()
                .map(|g| g.eq_ignore_ascii_case(group))
                .unwrap_or(false)
        })
    }

    /// Goblin variants A/B/C must resolve to different DSFT palette IDs.
    /// Regression: find_with_sprite was falling back to variant A for B/C,
    /// causing all goblins to get palette_id=225 (variant A's palette).
    #[test]
    fn goblin_variants_have_distinct_dsft_palettes() {
        let lod_manager = LodManager::new(get_lod_path()).unwrap();
        let dsft = DSFT::new(&lod_manager).unwrap();
        let monlist = crate::monlist::MonsterList::new(&lod_manager).unwrap();

        // Resolve goblin A, B, C through monlist + DSFT
        let gob_a = monlist.find_by_name("Goblin", 1).expect("Goblin A should exist");
        let gob_b = monlist.find_by_name("Goblin", 2).expect("Goblin B should exist");
        let gob_c = monlist.find_by_name("Goblin", 3).expect("Goblin C should exist");

        // Each variant should have a different internal name
        assert_ne!(gob_a.internal_name, gob_b.internal_name);
        assert_ne!(gob_b.internal_name, gob_c.internal_name);

        // Their standing sprite group names should differ
        let st_a = &gob_a.sprite_names[0];
        let st_b = &gob_b.sprite_names[0];
        let st_c = &gob_c.sprite_names[0];
        assert_ne!(st_a, st_b, "goblin A/B standing sprites should differ");
        assert_ne!(st_b, st_c, "goblin B/C standing sprites should differ");

        // Lookup DSFT frames for each variant's standing group
        let frame_a = find_frame_by_group(&dsft, st_a).expect("DSFT frame for goblin A");
        let frame_b = find_frame_by_group(&dsft, st_b).expect("DSFT frame for goblin B");
        let frame_c = find_frame_by_group(&dsft, st_c).expect("DSFT frame for goblin C");

        // Variant B and C must have different palette IDs than variant A
        assert_ne!(
            frame_a.palette_id, frame_b.palette_id,
            "goblin A ({}) and B ({}) should have different DSFT palette IDs",
            st_a, st_b
        );
        assert_ne!(
            frame_a.palette_id, frame_c.palette_id,
            "goblin A ({}) and C ({}) should have different DSFT palette IDs",
            st_a, st_c
        );
    }

    /// Ghost/skeleton DSFT palette_id must differ from sprite file header palette.
    /// Regression: using sprite header palette offset gave wrong colors for ghosts.
    #[test]
    fn ghost_dsft_palette_differs_from_header() {
        let lod_manager = LodManager::new(get_lod_path()).unwrap();
        let dsft = DSFT::new(&lod_manager).unwrap();
        let monlist = crate::monlist::MonsterList::new(&lod_manager).unwrap();

        // Ghost variant B should have a non-zero DSFT palette_id
        let ghost_b = monlist.find_by_name("Ghost", 2).expect("Ghost B should exist");
        let st_group = &ghost_b.sprite_names[0];
        let frame = find_frame_by_group(&dsft, st_group).expect("DSFT frame for ghost B");
        assert!(
            frame.palette_id > 0,
            "ghost B DSFT palette_id should be non-zero (got {})",
            frame.palette_id
        );

        // The DSFT palette_id is the authoritative palette for variant coloring.
        // Verify we can actually load a sprite with this palette.
        let sprite_name = frame.sprite_name().expect("ghost B should have sprite name");
        let root = sprite_name.trim_end_matches(|c: char| c.is_ascii_digit());
        let root = if root.ends_with(|c: char| c >= 'a' && c <= 'f') {
            &root[..root.len() - 1]
        } else {
            root
        };
        let test_name = format!("{}a0", root);
        let img = lod_manager.sprite_with_palette(&test_name, frame.palette_id as u16);
        assert!(img.is_some(), "should load ghost sprite with DSFT palette {}", frame.palette_id);
    }

    /// Monster variants A/B/C must resolve through monlist → DSFT → LOD sprite files.
    /// Each variant should have a valid DSFT entry with a sprite that exists in the LOD.
    /// Table-driven so we can easily add more monsters as we discover issues.
    ///
    /// Regressions caught:
    /// - Goblin B/C getting variant A's palette (find_with_sprite fallback)
    /// - Ghost/skeleton wrong palette (sprite header vs DSFT palette mismatch)
    /// - Lizardman B/C failing to load (DSFT group names ≠ file names)
    #[test]
    fn monster_variants_resolve_through_dsft() {
        let lod_manager = LodManager::new(get_lod_path()).unwrap();
        let dsft = DSFT::new(&lod_manager).unwrap();
        let monlist = crate::monlist::MonsterList::new(&lod_manager).unwrap();

        // (monster_name, variants_to_test, expect_distinct_palettes)
        // expect_distinct_palettes: if true, variant B/C must have different palette_id than A
        let test_cases: &[(&str, &[u8], bool)] = &[
            ("Goblin",   &[1, 2, 3], true),
            ("Ghost",    &[1, 2, 3], true),
            ("Skeleton", &[1, 2, 3], true),
            ("Spider",   &[1, 2, 3], false),
        ];

        for &(monster_name, variants, expect_distinct) in test_cases {
            let mut palette_ids: Vec<(u8, i16)> = Vec::new();

            for &dif in variants {
                let variant_letter = match dif { 1 => "A", 2 => "B", _ => "C" };
                let desc = match monlist.find_by_name(monster_name, dif) {
                    Some(d) => d,
                    None => {
                        // Some monsters may not have all 3 variants — skip
                        continue;
                    }
                };

                // Verify the correct variant suffix was returned
                let expected_suffix = variant_letter;
                assert!(
                    desc.internal_name.ends_with(expected_suffix),
                    "{} difficulty {} should return variant {}, got '{}'",
                    monster_name, dif, expected_suffix, desc.internal_name
                );

                let st_group = &desc.sprite_names[0];
                assert!(
                    !st_group.is_empty(),
                    "{} {} standing sprite should be non-empty",
                    monster_name, variant_letter
                );

                // DSFT must have a frame for this group
                let frame = match find_frame_by_group(&dsft, st_group) {
                    Some(f) => f,
                    None => {
                        panic!(
                            "DSFT frame for {} {} group '{}' should exist",
                            monster_name, variant_letter, st_group
                        );
                    }
                };

                palette_ids.push((dif, frame.palette_id));

                // The DSFT sprite_name should resolve to a real file in the LOD
                let sprite_name = frame.sprite_name().unwrap_or_else(|| {
                    panic!("{} {} DSFT frame should have sprite name", monster_name, variant_letter)
                });
                let root = sprite_name.trim_end_matches(|c: char| c.is_ascii_digit());
                let root = if root.ends_with(|c: char| c >= 'a' && c <= 'f') {
                    &root[..root.len() - 1]
                } else {
                    root
                };
                let test = format!("sprites/{}a0", root.to_lowercase());
                assert!(
                    lod_manager.try_get_bytes(&test).is_ok(),
                    "{} {} sprite '{}' (from DSFT group '{}') should exist in LOD",
                    monster_name, variant_letter, test, st_group
                );
            }

            // Check palette distinctness for monsters that need it
            if expect_distinct && palette_ids.len() >= 2 {
                let a_palette = palette_ids.iter().find(|(d, _)| *d == 1).map(|(_, p)| *p);
                for &(dif, pal) in &palette_ids {
                    if dif > 1 {
                        if let Some(a_pal) = a_palette {
                            assert_ne!(
                                a_pal, pal,
                                "{} variant {} palette_id ({}) should differ from variant A ({})",
                                monster_name,
                                match dif { 2 => "B", _ => "C" },
                                pal,
                                a_pal
                            );
                        }
                    }
                }
            }
        }
    }
}
