use std::error::Error;

use crate::{
    LodManager,
    billboard::{Billboard, BillboardManager},
};

/// A single resolved decoration (billboard/spawn point) from an outdoor map.
#[derive(Clone)]
pub struct DecorationEntry {
    /// MM6 world coordinates [x, y, z].
    pub position: [i32; 3],
    /// Resolved sprite root name (e.g. "shp" for a directional tree, "fount1" for a fountain).
    pub sprite_name: String,
    /// True if the sprite has directional variants ({root}0..{root}4).
    pub is_directional: bool,
    /// World-space width (DSFT scale already applied; 0.0 for directional sprites).
    pub width: f32,
    /// World-space height (DSFT scale already applied; 0.0 for directional sprites).
    pub height: f32,
    /// Sound ID for ambient sound playback (0 = none).
    pub sound_id: u16,
    /// Event ID for interaction triggers.
    pub event_id: i16,
    /// Original index in the ODM billboard list (for event correlation during spawn).
    pub billboard_index: usize,
    /// Facing yaw in radians, converted from MM6 direction_degrees.
    pub facing_yaw: f32,
    /// Declist ID used to resolve sprite name, SFT frame, and scale via BillboardManager::get().
    pub declist_id: u16,
}

/// Per-map decoration roster built from ODM billboard data.
#[derive(Clone)]
pub struct Decorations {
    entries: Vec<DecorationEntry>,
}

impl Decorations {
    /// Return an empty decoration roster (e.g. for maps with no billboards).
    pub fn empty() -> Self {
        Decorations { entries: Vec::new() }
    }

    /// Build a decoration roster from the ODM billboard list.
    ///
    /// Filters out invisible, marker, and no-draw decorations.
    /// Resolves sprite names, detects directional sprites, pre-extracts DSFT scale,
    /// and pre-computes world dimensions for non-directional sprites.
    pub fn new(lod: &LodManager, odm_billboards: &[Billboard]) -> Result<Self, Box<dyn Error>> {
        let mgr = BillboardManager::new(lod)?;
        let mut entries = Vec::new();

        for (billboard_index, bb) in odm_billboards.iter().enumerate() {
            // Skip invisible billboards
            if bb.data.is_invisible() {
                continue;
            }

            // Resolve declist item; skip if not found
            let Some(declist_item) = mgr.get_declist_item(bb.data.declist_id) else {
                log::warn!(
                    "decorations: declist_id {} not found for billboard {}",
                    bb.data.declist_id,
                    billboard_index
                );
                continue;
            };

            // Skip markers and no-draw decorations
            if declist_item.is_marker() || declist_item.is_no_draw() {
                continue;
            }

            let sound_id = declist_item.sound_id;
            let event_id = bb.data.event;
            let position = bb.data.position;
            // direction_degrees is in a 2048-unit full-circle (1 unit = PI/1024 radians)
            let facing_yaw = bb.data.direction_degrees as f32 * std::f32::consts::PI / 1024.0;

            // Check for directional sprites
            let name = &bb.declist_name;
            let (sprite_name, is_directional, width, height) = if let Some(root) = find_directional_root(name, lod) {
                (root, true, 0.0, 0.0)
            } else {
                // Non-directional: pre-compute world dimensions
                let (w, h) = mgr
                    .get(lod, name, bb.data.declist_id)
                    .map(|sprite| sprite.dimensions())
                    .unwrap_or((1.0, 1.0));
                (name.clone(), false, w, h)
            };

            entries.push(DecorationEntry {
                position,
                sprite_name,
                is_directional,
                width,
                height,
                sound_id,
                event_id,
                billboard_index,
                facing_yaw,
                declist_id: bb.data.declist_id,
            });
        }

        Ok(Decorations { entries })
    }

    pub fn entries(&self) -> &[DecorationEntry] {
        &self.entries
    }

    pub fn iter(&self) -> impl Iterator<Item = &DecorationEntry> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Detect whether a sprite name has directional variants by probing the LOD.
///
/// Strips trailing digits from `name` and progressively shortens the root,
/// testing whether `{root}0` and `{root}1` both exist in the sprites archive.
/// Returns the matching root if found, otherwise `None`.
fn find_directional_root(name: &str, lod: &LodManager) -> Option<String> {
    let root = name.trim_end_matches(|c: char| c.is_ascii_digit());
    let mut try_root = root;
    while try_root.len() >= 3 {
        let lower = try_root.to_lowercase();
        let test0 = format!("sprites/{}0", lower);
        let test1 = format!("sprites/{}1", lower);
        if lod.try_get_bytes(&test0).is_ok() && lod.try_get_bytes(&test1).is_ok() {
            return Some(lower);
        }
        try_root = &try_root[..try_root.len() - 1];
    }
    None
}

#[cfg(test)]
#[path = "decorations_tests.rs"]
mod tests;
