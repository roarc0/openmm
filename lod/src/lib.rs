use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fs::{self};
use std::path::{Path, PathBuf};

use lod::Lod;
use palette::Palettes;

// ── Raw format parsers ────────────────────────────────────────────────────
pub mod blv;
pub mod bsp_model;
pub mod ddm;
pub mod dlv;
pub mod dtile;
pub mod evt;
pub mod mapstats;
pub mod monlist;
pub mod monsters;
pub mod odm;
pub mod twodevents;

pub mod awards;
pub mod billboard;
pub mod class;
pub mod dchest;
pub mod ddeclist;
pub mod dift;
pub mod dobjlist;
pub mod doverlay;
pub mod dpft;
pub mod dsft;
pub mod dsounds;
pub mod enums;
pub mod image;
pub mod items;
pub mod npcnews;
pub mod npcprof;
pub mod quest_bits;
pub mod smk;
pub mod snd;
pub mod spells;
pub mod terrain;
pub mod tft;
pub mod vid;

// ── Game-engine API (decoded, game-ready assets) ──────────────────────────
pub mod game;

mod lod;
pub mod lod_data;
pub mod palette;
mod utils;
pub(crate) mod zlib;

pub const ENV_OPENMM_6_PATH: &str = "OPENMM_6_PATH";

pub struct LodManager {
    lods: HashMap<String, Lod>,
    game_dir: PathBuf,
}

impl LodManager {
    pub fn new<P>(path: P) -> Result<Self, Box<dyn Error>>
    where
        P: AsRef<Path>,
    {
        let game_dir = path.as_ref().to_path_buf();
        let lod_files = Self::list_lod_files(&game_dir)?;
        let lod_map = Self::create_lod_file_map(lod_files)?;
        Ok(Self {
            lods: lod_map,
            game_dir,
        })
    }

    pub fn game_dir(&self) -> &Path {
        &self.game_dir
    }

    fn list_lod_files<P>(path: P) -> Result<Vec<PathBuf>, std::io::Error>
    where
        P: AsRef<Path>,
    {
        let mut lod_files = Vec::new();
        let entries = fs::read_dir(&path)?;

        for entry in entries {
            let entry = entry?;
            let file_name = entry.file_name();
            if let Some(name) = file_name.to_str()
                && name.to_lowercase().ends_with(".lod")
            {
                lod_files.push(Path::join(path.as_ref(), name));
            }
        }

        Ok(lod_files)
    }

    fn create_lod_file_map(lod_files: Vec<PathBuf>) -> Result<HashMap<String, Lod>, Box<dyn Error>> {
        let mut lod_file_map: HashMap<String, Lod> = HashMap::new();

        for path in lod_files.iter() {
            let lod = Lod::open(path)?;
            let key = path
                .file_stem()
                .ok_or("file should have a .lod extension")?
                .to_string_lossy()
                .to_lowercase();
            lod_file_map.insert(key, lod);
        }

        Ok(lod_file_map)
    }

    pub fn try_get_bytes<P: AsRef<Path>>(&self, path: P) -> Result<&[u8], Box<dyn Error>> {
        let lod_archive: String = path
            .as_ref()
            .parent()
            .ok_or("invalid path")?
            .to_string_lossy()
            .to_string();
        let lod = self
            .lods
            .get(&lod_archive)
            .ok_or(format!("lod file not found in {lod_archive} "))?;
        let lod_entry: String = path
            .as_ref()
            .file_name()
            .ok_or("invalid lod entry")?
            .to_string_lossy()
            .to_string();
        let lod_data = lod
            .try_get_bytes(&lod_entry)
            .ok_or(format!("unable to open lod entry {:?}", path.as_ref().to_str()))?;
        Ok(lod_data)
    }

    /// Load raw bytes from an archive path, decompressing if needed.
    pub fn get_decompressed<P: AsRef<Path>>(&self, path: P) -> Result<Vec<u8>, Box<dyn Error>> {
        let raw = self.try_get_bytes(path)?;
        Ok(match crate::lod_data::LodData::try_from(raw) {
            Ok(d) => d.data,
            Err(_) => raw.to_vec(),
        })
    }

    pub fn palettes(&self) -> Result<Palettes, Box<dyn Error>> {
        let bitmaps_lod = self.lods.get("bitmaps").ok_or("expected to have bitmaps.lod")?;
        let palettes = palette::Palettes::try_from(bitmaps_lod)?;
        Ok(palettes)
    }

    /// Returns a list of archive names (e.g. "bitmaps", "sprites", "icons").
    pub fn archives(&self) -> Vec<&str> {
        self.lods.keys().map(|s| s.as_str()).collect()
    }

    /// Returns a list of file names within a specific archive.
    pub fn files_in(&self, archive: &str) -> Option<Vec<&str>> {
        self.lods.get(archive).map(|lod| lod.files())
    }

    /// Dumps all files from all archives to the given directory.
    /// Images/sprites are saved as PNG, other data as raw files.
    pub fn dump_all(&self, output_dir: &Path) -> Result<(), Box<dyn Error>> {
        let palettes = self.palettes()?;
        for (name, lod) in &self.lods {
            let archive_dir = output_dir.join(name);
            lod.save_all(&archive_dir, &palettes)?;
        }
        Ok(())
    }

    /// Access the game-engine API: decoded, game-ready assets (sprites, bitmaps, fonts, NPC tables).
    pub fn game(&self) -> game::GameLod<'_> {
        game::GameLod::new(self)
    }
}

/// Returns the base MM6 game data directory (e.g. for Sounds/).
/// Uses OPENMM_6_PATH env var if set, otherwise falls back to workspace target dir.
pub fn get_data_path() -> String {
    env::var(ENV_OPENMM_6_PATH)
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(default_data_path)
}

/// Returns the LOD archive directory (where .lod files live).
/// Uses OPENMM_6_PATH env var if set, otherwise falls back to workspace target dir.
pub fn get_lod_path() -> String {
    env::var(ENV_OPENMM_6_PATH)
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(default_lod_path)
}

fn default_data_path() -> String {
    // Try workspace root (two levels up from lod crate manifest)
    let manifest = env!("CARGO_MANIFEST_DIR");
    let workspace = Path::new(manifest).parent().unwrap_or(Path::new("."));
    let candidate = workspace.join("data/mm6");
    if candidate.exists() {
        return candidate.to_string_lossy().into_owned();
    }
    "./data/mm6".into()
}

fn default_lod_path() -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let workspace = Path::new(manifest).parent().unwrap_or(Path::new("."));
    let candidate = workspace.join("data/mm6/data");
    if candidate.exists() {
        return candidate.to_string_lossy().into_owned();
    }
    "./data/mm6/data".into()
}

/// Returns `Some(LodManager)` when MM6 game data is present, or `None` otherwise.
/// Tests that call this and get `None` should return early — the data is simply not
/// available in this environment (e.g. CI). This is the standard way to write
/// game-data-dependent tests in this crate.
///
/// ```ignore
/// #[test]
/// fn my_test() {
///     let Some(lod) = test_lod() else { return; };
///     // ... use lod
/// }
/// ```
#[cfg(test)]
pub(crate) fn test_lod() -> Option<LodManager> {
    let path = get_lod_path();
    if !Path::new(&path).exists() {
        eprintln!("test: MM6 game data not found at '{path}' — skipping");
        return None;
    }
    LodManager::new(path).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lod_manager_works() {
        let lod_path = get_lod_path();
        let lod_manager = LodManager::new(lod_path).unwrap();
        let grastyl = lod_manager.try_get_bytes("bitmaps/grastyl");
        assert_eq!(17676, grastyl.unwrap().len());
    }

    #[test]
    fn font_loading_works() {
        let lod_path = get_lod_path();
        let lod_manager = LodManager::new(lod_path).unwrap();

        let names = lod_manager.game().font_names();
        assert!(!names.is_empty(), "should find .fnt files");

        let font = lod_manager.game().font("arrus.fnt").expect("arrus.fnt should load");
        assert_eq!(font.height, 19);
        assert!(font.has_glyph(b'A'));
        assert!(font.glyph_pixels(b'A').is_some());

        // Measure and render
        let width = font.measure("Hello");
        assert!(width > 0);
        let (w, h, buf) = font.render_text("Hi", [255, 255, 255, 255]);
        assert_eq!(h, 19);
        assert!(w > 0);
        assert_eq!(buf.len(), (w * h * 4) as usize);
    }

    #[test]
    fn sprite_works() {
        let lod_path = get_lod_path();
        let lod_manager = LodManager::new(lod_path).unwrap();
        let rock = lod_manager.game().sprite("rok1");
        assert!(rock.is_some());
    }

    /// Verify that sprite_with_palette produces different pixel data than the default palette.
    /// This is the mechanism used for monster variant B/C coloring (ghosts, skeletons, etc.).
    #[test]
    fn sprite_with_palette_produces_different_pixels() {
        let lod_path = get_lod_path();
        let lod_manager = LodManager::new(lod_path).unwrap();
        let dsft = crate::dsft::DSFT::new(&lod_manager).unwrap();
        let monlist = crate::monlist::MonsterList::new(&lod_manager).unwrap();

        // Ghost B has a different DSFT palette than the sprite file header palette.
        // Loading with the DSFT palette should produce visually different pixels.
        let ghost_b = monlist.find_by_name("Ghost", 2).expect("Ghost B should exist");
        let st_group = &ghost_b.sprite_names[0];

        // Find DSFT palette_id for this group
        let frame = dsft
            .frames
            .iter()
            .find(|f| {
                f.group_name()
                    .map(|g| g.eq_ignore_ascii_case(st_group))
                    .unwrap_or(false)
            })
            .expect("DSFT frame for ghost B");
        assert!(frame.palette_id > 0, "ghost B should have non-zero DSFT palette");

        // Derive the sprite root from DSFT sprite name
        let sprite_name = frame.sprite_name().unwrap();
        let root = sprite_name.trim_end_matches(|c: char| c.is_ascii_digit());
        let root =
            if root.len() > 1 && root.as_bytes()[root.len() - 1] >= b'a' && root.as_bytes()[root.len() - 1] <= b'f' {
                &root[..root.len() - 1]
            } else {
                root
            };
        let test_sprite = format!("{}a0", root);

        // Load with default palette and DSFT palette
        let default_img = lod_manager
            .game()
            .sprite(&test_sprite)
            .expect("ghost sprite with default palette");
        let dsft_img = lod_manager
            .game()
            .sprite_with_palette(&test_sprite, frame.palette_id as u16)
            .expect("ghost sprite with DSFT palette");

        // Both should be the same dimensions
        assert_eq!(default_img.width(), dsft_img.width());
        assert_eq!(default_img.height(), dsft_img.height());

        // But the pixel data should differ (different palette = different colors)
        let default_bytes = default_img.to_rgba8().into_raw();
        let dsft_bytes = dsft_img.to_rgba8().into_raw();
        assert_ne!(
            default_bytes, dsft_bytes,
            "sprite with DSFT palette {} should produce different pixels than default",
            frame.palette_id
        );
    }

    /// Check which attack/die sprite files actually exist to debug naming.
    #[test]
    fn print_archer_sprite_names() {
        let lod_path = get_lod_path();
        let mgr = LodManager::new(lod_path).unwrap();
        let monlist = crate::monlist::MonsterList::new(&mgr).unwrap();
        let dsft = crate::dsft::DSFT::new(&mgr).unwrap();

        // Print DSFT groups matching arc1* to understand what roots exist
        println!("DSFT groups matching arc1*:");
        for frame in &dsft.frames {
            if let Some(g) = frame.group_name()
                && g.to_lowercase().starts_with("arc1")
            {
                println!("  group='{}' sprite='{:?}'", g, frame.sprite_name());
            }
        }

        // Check which arc1di* sprite files exist
        println!("arc1di* sprites in LOD:");
        for suffix in &["a0","a1","b0","qa0","qa","q"] {
            let path = format!("sprites/arc1di{}", suffix);
            let exists = mgr.try_get_bytes(&path).is_ok();
            if exists { println!("  EXISTS: {}", path); }
        }
        // ArcherA dying sprite_names[5] = "arc1diQ"
        let archer = monlist.find_by_name("Archer", 1).expect("Archer A");
        println!("ArcherA sprite_names: {:?}", &archer.sprite_names[..6]);
    }
    /// actually exist as sprite files in the LOD. Guards against the previous bug where
    /// sprite_names[2]/[5] (DSFT group names) were used directly as file roots, causing
    /// the fallback to load standing sprites for attack/die animations.
    #[test]
    fn attacking_and_dying_sprite_roots_exist() {
        let lod_path = get_lod_path();
        let mgr = LodManager::new(lod_path).unwrap();
        let monlist = crate::monlist::MonsterList::new(&mgr).unwrap();
        let dsft = crate::dsft::DSFT::new(&mgr).unwrap();

        let sample = &monlist.monsters[..monlist.monsters.len().min(30)];
        let mut missing_att = vec![];
        let mut missing_die = vec![];
        for desc in sample {
            if desc.sprite_names[2].is_empty() || desc.sprite_names[5].is_empty() {
                continue;
            }
            // Resolve via DSFT like the fixed pipeline does
            let resolve = |group: &str| -> String {
                // Look up DSFT group → get sprite_name → derive root
                for frame in &dsft.frames {
                    if let Some(gname) = frame.group_name()
                        && gname.eq_ignore_ascii_case(group)
                    {
                        if let Some(sprite_name) = frame.sprite_name() {
                            let without_digits = sprite_name.trim_end_matches(|c: char| c.is_ascii_digit());
                            if without_digits.len() > 1 {
                                let last = without_digits.as_bytes()[without_digits.len() - 1];
                                let root = if (b'a'..=b'f').contains(&last) {
                                    &without_digits[..without_digits.len() - 1]
                                } else {
                                    without_digits
                                };
                                return root.to_lowercase();
                            }
                        }
                        break;
                    }
                }
                group.to_lowercase()
            };

            let at_root = resolve(&desc.sprite_names[2]);
            let die_root = resolve(&desc.sprite_names[5]);
            let exists = |root: &str| {
                mgr.try_get_bytes(&format!("sprites/{}a0", root)).is_ok()
                    || mgr.try_get_bytes(&format!("sprites/{}", root)).is_ok()
            };
            if !exists(&at_root) {
                missing_att.push(format!("{}: group='{}' resolved='{}'", desc.internal_name, &desc.sprite_names[2], at_root));
            }
            if !exists(&die_root) {
                missing_die.push(format!("{}: group='{}' resolved='{}'", desc.internal_name, &desc.sprite_names[5], die_root));
            }
        }
        assert!(
            missing_att.is_empty(),
            "Monsters with unresolvable attack sprite after DSFT lookup: {:?}",
            missing_att
        );
        assert!(
            missing_die.is_empty(),
            "Monsters with unresolvable dying sprite after DSFT lookup: {:?}",
            missing_die
        );
    }
}
