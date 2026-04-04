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
    /// Number of animation frames in the DSFT group (1 = static, >1 = animated loop).
    pub num_frames: usize,
    /// Seconds per animation frame (0.15 normal, 0.30 for is_slow_loop decorations).
    pub frame_duration: f32,
    /// Flicker rate in Hz (0.0 = no flicker). Driven by is_flicker_{slow,medium,fast} flags.
    pub flicker_rate: f32,
    /// Point-light emission radius in MM6 world units (0 = no light).
    pub light_radius: u16,
    /// Decoration type from ddeclist.bin.
    pub dec_type: u16,
    /// Event variable / parameter passed to the event script on activation.
    pub event_variable: i16,
    /// Proximity radius in MM6 units for trigger activations (0 = no trigger).
    pub trigger_radius: i16,
    /// If true, this decoration does not block player movement.
    pub no_block_movement: bool,
    /// If true, decoration emits fire particles.
    pub emit_fire: bool,
    /// If true, decoration plays its sound at dawn.
    pub sound_on_dawn: bool,
    /// If true, decoration plays its sound at dusk.
    pub sound_on_dusk: bool,
    /// If true, decoration emits smoke particles.
    pub emit_smoke: bool,
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

            // Animation and flicker attributes from the declist item
            let flicker_rate = if declist_item.is_flicker_fast() {
                4.0_f32
            } else if declist_item.is_flicker_medium() {
                2.0_f32
            } else if declist_item.is_flicker_slow() {
                1.0_f32
            } else {
                0.0_f32
            };
            let frame_duration = if declist_item.is_slow_loop() {
                0.30_f32
            } else {
                0.15_f32
            };

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

            let num_frames = if is_directional {
                1 // Directional animation handled separately
            } else {
                mgr.animation_frame_count(bb.data.declist_id)
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
                num_frames,
                frame_duration,
                flicker_rate,
                light_radius: declist_item.light_radius,
                dec_type: declist_item.dec_type,
                event_variable: bb.data.event_variable,
                trigger_radius: bb.data.trigger_radius,
                no_block_movement: declist_item.is_no_block_movement(),
                emit_fire: declist_item.is_emit_fire(),
                sound_on_dawn: declist_item.is_sound_on_dawn(),
                sound_on_dusk: declist_item.is_sound_on_dusk(),
                emit_smoke: declist_item.is_emit_smoke(),
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
