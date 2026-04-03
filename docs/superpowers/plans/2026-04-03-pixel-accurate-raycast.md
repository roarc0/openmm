# Pixel-Accurate Raycast & Hover System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the imprecise angular-cone interaction/hover detection with pixel-accurate ray-geometry intersection for all object types: billboard sprites (alpha-masked), BSP faces, and indoor faces.

**Architecture:** A new `raycast.rs` module holds pure geometry math (ray-plane intersection, polygon containment, billboard hit test, event name resolution). Sprite loading threads `AlphaMask` data alongside materials so each `SpriteSheet` entity always knows which pixels are opaque for the currently displayed frame. Interaction and hover systems query `SpriteSheet` for both dimensions and the current mask, replacing the old cone test everywhere.

**Tech Stack:** Bevy 0.18, `image` crate (`RgbaImage`, `GenericImageView`), `std::sync::Arc`

---

## File Map

| File | Change |
|------|--------|
| `openmm/src/game/mod.rs` | Add `pub(crate) mod raycast;` |
| `openmm/src/game/raycast.rs` | **Create** — `ray_plane_intersect`, `point_in_polygon`, `billboard_hit_test`, `resolve_event_name` |
| `openmm/src/game/entities/sprites.rs` | Add `AlphaMask`; extend `SpriteCache`, `SpriteSheet`, `decode_sprite_frames`, `load_decoration_directions`, `store_in_cache`, `rebuild_from_cache`, `update_sprite_sheets` |
| `openmm/src/game/interaction.rs` | Add `MonsterInteractable`; rewrite `decoration_interact_system`, `npc_interact_system`, `hover_hint_system`; remove `BuildingInfo`, `interact_system`, `raycast_nearest`, `find_nearest_*`, `resolve_*` |
| `openmm/src/game/blv.rs` | Remove `ray_plane_intersect`, `point_in_polygon`; import from `raycast`; add `indoor_hover_hint_system` |
| `openmm/src/game/odm.rs` | Remove `BuildingInfo` inserts; add `MonsterInteractable` to monster spawns; update `SpriteSheet::new` call sites |

---

### Task 1: Create `raycast.rs` — geometry functions moved from `blv.rs`

**Files:**
- Create: `openmm/src/game/raycast.rs`
- Modify: `openmm/src/game/mod.rs`
- Modify: `openmm/src/game/blv.rs`

- [ ] **Step 1: Write failing tests in a new `raycast.rs`**

Create `openmm/src/game/raycast.rs` with just the tests (functions not yet defined):

```rust
use bevy::prelude::*;

/// Ray-plane intersection. Returns distance `t` along ray if hit (positive = in front).
pub fn ray_plane_intersect(origin: Vec3, dir: Vec3, normal: Vec3, plane_dist: f32) -> Option<f32> {
    todo!()
}

/// Test if a 3D point lies inside a convex/concave polygon using winding number.
/// All points assumed coplanar. Projects to the best 2D plane based on normal.
pub fn point_in_polygon(point: Vec3, vertices: &[Vec3], normal: Vec3) -> bool {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ray_plane_hit() {
        // Ray going straight down, plane at y=0 (normal Vec3::Y, dist 0)
        let t = ray_plane_intersect(Vec3::new(0.0, 5.0, 0.0), Vec3::NEG_Y, Vec3::Y, 0.0);
        assert!((t.unwrap() - 5.0).abs() < 1e-4);
    }

    #[test]
    fn ray_plane_parallel_miss() {
        // Ray parallel to plane: no hit
        let t = ray_plane_intersect(Vec3::new(0.0, 1.0, 0.0), Vec3::X, Vec3::Y, 0.0);
        assert!(t.is_none());
    }

    #[test]
    fn ray_plane_behind_miss() {
        // Plane is behind the ray origin: no hit
        let t = ray_plane_intersect(Vec3::new(0.0, -1.0, 0.0), Vec3::NEG_Y, Vec3::Y, 0.0);
        assert!(t.is_none());
    }

    #[test]
    fn point_in_square_polygon() {
        // XZ square from (-1,-1) to (1,1) at y=0, normal up
        let verts = vec![
            Vec3::new(-1.0, 0.0, -1.0),
            Vec3::new( 1.0, 0.0, -1.0),
            Vec3::new( 1.0, 0.0,  1.0),
            Vec3::new(-1.0, 0.0,  1.0),
        ];
        assert!(point_in_polygon(Vec3::new(0.0, 0.0, 0.0), &verts, Vec3::Y));
        assert!(!point_in_polygon(Vec3::new(2.0, 0.0, 0.0), &verts, Vec3::Y));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd /home/roarc/repos/openmm
cargo test -p openmm raycast 2>&1 | head -20
```

Expected: compile error (`todo!()` panics or cannot compile without module registration yet).

- [ ] **Step 3: Register `raycast` module in `game/mod.rs`**

In `openmm/src/game/mod.rs`, add after line 5 (`pub(crate) mod blv;`):

```rust
pub(crate) mod raycast;
```

- [ ] **Step 4: Implement the two geometry functions**

Replace the `todo!()` bodies in `raycast.rs`:

```rust
pub fn ray_plane_intersect(origin: Vec3, dir: Vec3, normal: Vec3, plane_dist: f32) -> Option<f32> {
    let denom = normal.dot(dir);
    if denom.abs() < 1e-6 {
        return None;
    }
    let t = (plane_dist - normal.dot(origin)) / denom;
    if t > 0.0 { Some(t) } else { None }
}

pub fn point_in_polygon(point: Vec3, vertices: &[Vec3], normal: Vec3) -> bool {
    if vertices.len() < 3 {
        return false;
    }
    let abs_n = normal.abs();
    let (ax1, ax2) = if abs_n.x >= abs_n.y && abs_n.x >= abs_n.z {
        (1usize, 2usize)
    } else if abs_n.y >= abs_n.z {
        (0, 2)
    } else {
        (0, 1)
    };
    let get = |v: Vec3, axis: usize| -> f32 {
        match axis { 0 => v.x, 1 => v.y, _ => v.z }
    };
    let px = get(point, ax1);
    let py = get(point, ax2);
    let mut winding = 0i32;
    let n = vertices.len();
    for i in 0..n {
        let v1 = vertices[i];
        let v2 = vertices[(i + 1) % n];
        let y1 = get(v1, ax2);
        let y2 = get(v2, ax2);
        if y1 <= py {
            if y2 > py {
                let x1 = get(v1, ax1);
                let x2 = get(v2, ax1);
                if (x2 - x1) * (py - y1) - (px - x1) * (y2 - y1) > 0.0 {
                    winding += 1;
                }
            }
        } else if y2 <= py {
            let x1 = get(v1, ax1);
            let x2 = get(v2, ax1);
            if (x2 - x1) * (py - y1) - (px - x1) * (y2 - y1) < 0.0 {
                winding -= 1;
            }
        }
    }
    winding != 0
}
```

- [ ] **Step 5: Run tests — they must pass**

```bash
cargo test -p openmm raycast 2>&1
```

Expected: all 4 tests pass.

- [ ] **Step 6: Update `blv.rs` to import from `raycast` instead of defining locally**

In `openmm/src/game/blv.rs`, delete the two private function bodies for `ray_plane_intersect` and `point_in_polygon` (lines ~409–480), and add the import at the top of the file:

```rust
use crate::game::raycast::{ray_plane_intersect, point_in_polygon};
```

- [ ] **Step 7: Verify it still compiles and tests pass**

```bash
cargo test -p openmm 2>&1 | tail -5
```

Expected: no errors, all tests pass.

- [ ] **Step 8: Commit**

```bash
git add openmm/src/game/raycast.rs openmm/src/game/mod.rs openmm/src/game/blv.rs
git commit --no-gpg-sign -m "refactor: extract ray_plane_intersect and point_in_polygon into raycast.rs"
```

---

### Task 2: Add `resolve_event_name` to `raycast.rs`

**Files:**
- Modify: `openmm/src/game/raycast.rs`

This unifies `resolve_building_name` and `resolve_decoration_name` from `interaction.rs` into one function with consistent priority ordering.

- [ ] **Step 1: Write the failing test**

Add to `raycast.rs` at the bottom of the `#[cfg(test)]` block:

```rust
    #[test]
    fn resolve_event_name_hint_wins() {
        use std::collections::HashMap;
        use lod::evt::{EvtFile, EvtStep, GameEvent};
        let mut events: HashMap<u16, Vec<EvtStep>> = HashMap::new();
        events.insert(1, vec![
            EvtStep { step: 0, event: GameEvent::StatusText { text: "status".into(), duration: 0 } },
            EvtStep { step: 1, event: GameEvent::Hint { text: "hint".into(), duration: 0 } },
        ]);
        let evt = EvtFile { events };
        let name = resolve_event_name_from_evt(1, &evt);
        // Hint should NOT beat StatusText in our priority — first non-empty match wins
        assert_eq!(name, Some("status".to_string()));
    }

    #[test]
    fn resolve_event_name_hint_first() {
        use std::collections::HashMap;
        use lod::evt::{EvtFile, EvtStep, GameEvent};
        let mut events: HashMap<u16, Vec<EvtStep>> = HashMap::new();
        events.insert(2, vec![
            EvtStep { step: 0, event: GameEvent::Hint { text: "hint".into(), duration: 0 } },
        ]);
        let evt = EvtFile { events };
        assert_eq!(resolve_event_name_from_evt(2, &evt), Some("hint".to_string()));
    }

    #[test]
    fn resolve_event_name_empty_text_skipped() {
        use std::collections::HashMap;
        use lod::evt::{EvtFile, EvtStep, GameEvent};
        let mut events: HashMap<u16, Vec<EvtStep>> = HashMap::new();
        events.insert(3, vec![
            EvtStep { step: 0, event: GameEvent::Hint { text: "".into(), duration: 0 } },
            EvtStep { step: 1, event: GameEvent::StatusText { text: "real".into(), duration: 0 } },
        ]);
        let evt = EvtFile { events };
        assert_eq!(resolve_event_name_from_evt(3, &evt), Some("real".to_string()));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test -p openmm raycast::tests::resolve 2>&1 | head -10
```

Expected: compile error — `resolve_event_name_from_evt` not defined.

- [ ] **Step 3: Implement `resolve_event_name_from_evt` and the full `resolve_event_name`**

Add these imports at the top of `raycast.rs`:

```rust
use lod::evt::EvtFile;
use crate::game::events::MapEvents;
```

Add the functions after the existing geometry functions:

```rust
/// Resolve a human-readable label for an event ID from its EVT steps.
/// Returns the first non-empty text found, checking in step order.
/// Recognised events: Hint, StatusText, SpeakInHouse, OpenChest, MoveToMap.
pub fn resolve_event_name_from_evt(event_id: u16, evt: &EvtFile) -> Option<String> {
    let steps = evt.events.get(&event_id)?;
    for s in steps {
        let text = match &s.event {
            lod::evt::GameEvent::Hint { text, .. } if !text.is_empty() => text.clone(),
            lod::evt::GameEvent::StatusText { text, .. } if !text.is_empty() => text.clone(),
            lod::evt::GameEvent::LocationName { text, .. } if !text.is_empty() => text.clone(),
            lod::evt::GameEvent::SpeakInHouse { house_id } => {
                format!("Building #{}", house_id)
            }
            lod::evt::GameEvent::OpenChest { id } => format!("Chest #{}", id),
            lod::evt::GameEvent::MoveToMap { map_name, .. } => {
                format!("Enter {}", map_name)
            }
            _ => continue,
        };
        return Some(text);
    }
    None
}

/// Resolve a label for an event from the map's loaded event table.
/// Returns `None` when no map events are loaded or no matching event exists.
pub fn resolve_event_name(event_id: u16, map_events: &Option<bevy::prelude::Res<MapEvents>>) -> Option<String> {
    let me = map_events.as_ref()?;
    let evt = me.evt.as_ref()?;

    // For SpeakInHouse, look up the house name from the loaded house table
    if let Some(steps) = evt.events.get(&event_id) {
        for s in steps {
            if let lod::evt::GameEvent::SpeakInHouse { house_id } = &s.event {
                if let Some(houses) = me.houses.as_ref()
                    && let Some(entry) = houses.houses.get(house_id)
                {
                    return Some(entry.name.clone());
                }
                return Some(format!("Building #{}", house_id));
            }
        }
    }

    resolve_event_name_from_evt(event_id, evt.as_ref())
}
```

- [ ] **Step 4: Run tests — they must pass**

```bash
cargo test -p openmm raycast 2>&1
```

Expected: all 7 tests pass.

- [ ] **Step 5: Commit**

```bash
git add openmm/src/game/raycast.rs
git commit --no-gpg-sign -m "feat: add resolve_event_name to raycast.rs"
```

---

### Task 3: Add `AlphaMask` to `sprites.rs`

**Files:**
- Modify: `openmm/src/game/entities/sprites.rs`

- [ ] **Step 1: Write the failing unit tests**

Add to the bottom of `sprites.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn make_mask(width: u32, height: u32, opaque: &[(u32, u32)]) -> AlphaMask {
        let mut data = vec![false; (width * height) as usize];
        for &(x, y) in opaque {
            data[(y * width + x) as usize] = true;
        }
        AlphaMask { width, height, data }
    }

    #[test]
    fn alpha_mask_opaque_pixel() {
        let mask = make_mask(4, 4, &[(1, 1), (2, 2)]);
        // u=1/4+0.5*1/4=... let's just use center of pixel (1,1): u=(1+0.5)/4, v=(1+0.5)/4
        assert!(mask.test(1.5 / 4.0, 1.5 / 4.0));
    }

    #[test]
    fn alpha_mask_transparent_pixel() {
        let mask = make_mask(4, 4, &[(1, 1)]);
        assert!(!mask.test(0.5 / 4.0, 0.5 / 4.0)); // pixel (0,0) is transparent
    }

    #[test]
    fn alpha_mask_clamped_edges() {
        // Out-of-range UV should clamp, not panic
        let mask = make_mask(2, 2, &[(0, 0), (1, 0), (0, 1), (1, 1)]);
        assert!(mask.test(-0.5, -0.5)); // clamps to (0,0)
        assert!(mask.test(1.5, 1.5));   // clamps to (1,1)
    }

    #[test]
    fn alpha_mask_from_image() {
        let mut img = image::RgbaImage::new(2, 2);
        img.put_pixel(0, 0, image::Rgba([255, 0, 0, 255])); // opaque
        img.put_pixel(1, 0, image::Rgba([0, 0, 0, 0]));     // transparent
        img.put_pixel(0, 1, image::Rgba([0, 0, 0, 0]));     // transparent
        img.put_pixel(1, 1, image::Rgba([0, 255, 0, 128])); // semi → opaque (>127)
        let mask = AlphaMask::from_image(&img);
        assert!(mask.test(0.25, 0.25));  // pixel (0,0) opaque
        assert!(!mask.test(0.75, 0.25)); // pixel (1,0) transparent
        assert!(mask.test(0.75, 0.75));  // pixel (1,1) semi-opaque → opaque
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test -p openmm sprites::tests 2>&1 | head -10
```

Expected: compile error — `AlphaMask` not defined.

- [ ] **Step 3: Implement `AlphaMask`**

Add this block near the top of `sprites.rs`, before `SpriteCache`:

```rust
use std::sync::Arc;

/// CPU-side 1-bit alpha mask for a sprite image. Used for pixel-accurate ray hit testing.
/// Built from the padded RGBA image at load time and kept in memory alongside the material.
pub struct AlphaMask {
    pub width: u32,
    pub height: u32,
    /// Row-major, true = opaque (alpha > 127).
    data: Vec<bool>,
}

impl AlphaMask {
    /// Build a mask from a padded RGBA sprite image. Pixels with alpha > 127 are opaque.
    pub fn from_image(img: &image::RgbaImage) -> Self {
        let data = img.pixels().map(|p| p[3] > 127).collect();
        Self { width: img.width(), height: img.height(), data }
    }

    /// Test a UV coordinate (both in \[0,1\]) — returns true if the pixel is opaque.
    /// UV is clamped to the image bounds.
    pub fn test(&self, u: f32, v: f32) -> bool {
        let x = (u * self.width as f32).clamp(0.0, (self.width - 1) as f32) as u32;
        let y = (v * self.height as f32).clamp(0.0, (self.height - 1) as f32) as u32;
        self.data[(y * self.width + x) as usize]
    }
}
```

- [ ] **Step 4: Run tests — they must pass**

```bash
cargo test -p openmm sprites::tests 2>&1
```

Expected: all 4 tests pass.

- [ ] **Step 5: Commit**

```bash
git add openmm/src/game/entities/sprites.rs
git commit --no-gpg-sign -m "feat: add AlphaMask for pixel-accurate sprite hit testing"
```

---

### Task 4: Thread `AlphaMask` through the sprite loading pipeline

**Files:**
- Modify: `openmm/src/game/entities/sprites.rs`
- Modify: `openmm/src/game/odm.rs`
- Modify: `openmm/src/game/blv.rs`

This task extends `SpriteCache`, `decode_sprite_frames`, `load_decoration_directions`, `load_entity_sprites`, `store_in_cache`, `rebuild_from_cache`, and `SpriteSheet` to carry alpha masks alongside materials. It also updates `update_sprite_sheets` to keep `current_mask` in sync.

- [ ] **Step 1: Extend `SpriteCache` with a masks map**

In `sprites.rs`, update `SpriteCache`:

```rust
#[derive(Resource, Default, Clone)]
pub struct SpriteCache {
    materials: HashMap<String, Handle<StandardMaterial>>,
    dimensions: HashMap<String, (f32, f32)>,
    /// Alpha masks keyed identically to `materials`.
    masks: HashMap<String, Arc<AlphaMask>>,
}
```

- [ ] **Step 2: Extend `store_in_cache` to also store masks**

Replace `store_in_cache`:

```rust
fn store_in_cache(
    key: &str,
    frames: &[[Handle<StandardMaterial>; 5]],
    frame_masks: &[[Arc<AlphaMask>; 5]],
    w: f32,
    h: f32,
    cache: &mut Option<&mut SpriteCache>,
) {
    if let Some(cache) = cache.as_mut() {
        cache.dimensions.insert(key.to_string(), (w, h));
        for (fi, (dirs, masks)) in frames.iter().zip(frame_masks.iter()).enumerate() {
            let frame_letter = (b'a' + fi as u8) as char;
            for di in 0..5 {
                let mat_key = format!("{}{}{}", key, frame_letter, di);
                cache.materials.insert(mat_key.clone(), dirs[di].clone());
                cache.masks.insert(mat_key, masks[di].clone());
            }
        }
    }
}
```

- [ ] **Step 3: Extend `rebuild_from_cache` to return masks alongside frames**

Replace `rebuild_from_cache`:

```rust
fn rebuild_from_cache(
    key: &str,
    cache: &SpriteCache,
) -> (Vec<[Handle<StandardMaterial>; 5]>, Vec<[Arc<AlphaMask>; 5]>) {
    let mut frames = Vec::new();
    let mut mask_frames = Vec::new();
    let fallback_mask = Arc::new(AlphaMask { width: 1, height: 1, data: vec![true] });
    for fi in 0..6 {
        let frame_letter = (b'a' + fi) as char;
        let key0 = format!("{}{}0", key, frame_letter);
        if let Some(mat0) = cache.materials.get(&key0) {
            let mask0 = cache.masks.get(&key0).cloned().unwrap_or_else(|| fallback_mask.clone());
            let mut dirs: [Handle<StandardMaterial>; 5] = Default::default();
            let mut masks: [Arc<AlphaMask>; 5] = std::array::from_fn(|_| fallback_mask.clone());
            for di in 0..5 {
                let mat_key = format!("{}{}{}", key, frame_letter, di);
                dirs[di] = cache.materials.get(&mat_key).cloned().unwrap_or_else(|| mat0.clone());
                masks[di] = cache.masks.get(&mat_key).cloned().unwrap_or_else(|| mask0.clone());
            }
            frames.push(dirs);
            mask_frames.push(masks);
        } else {
            break;
        }
    }
    (frames, mask_frames)
}
```

- [ ] **Step 4: Extend `decode_sprite_frames` to return masks**

Change the return type signature from `(Vec<[Handle<StandardMaterial>; 5]>, f32, f32)` to `(Vec<[Handle<StandardMaterial>; 5]>, Vec<[Arc<AlphaMask>; 5]>, f32, f32)`.

In the second pass (around line 331), build the mask alongside the material. Find the block that creates `dir_materials` and add `dir_masks`:

```rust
    // Second pass: tint, pad to uniform size, and create materials.
    let mut frames = Vec::new();
    let mut frame_masks: Vec<[Arc<AlphaMask>; 5]> = Vec::new();
    let fallback_mask = Arc::new(AlphaMask { width: 1, height: 1, data: vec![true] });
    for dir_imgs in raw_sprites {
        let mut dir_materials: [Handle<StandardMaterial>; 5] = Default::default();
        let mut dir_masks: [Arc<AlphaMask>; 5] = std::array::from_fn(|_| fallback_mask.clone());
        for (dir, img_opt) in dir_imgs.into_iter().enumerate() {
            if let Some(img) = img_opt {
                // Pad to uniform size: center horizontally, align bottom vertically
                let rgba = img.into_rgba8();
                let rgba = if rgba.width() != max_w || rgba.height() != max_h {
                    let mut padded = image::RgbaImage::new(max_w, max_h);
                    let x_off = (max_w - rgba.width()) / 2;
                    let y_off = max_h - rgba.height();
                    for py in 0..rgba.height() {
                        for px in 0..rgba.width() {
                            padded.put_pixel(px + x_off, py + y_off, *rgba.get_pixel(px, py));
                        }
                    }
                    padded
                } else {
                    rgba
                };
                dir_masks[dir] = Arc::new(AlphaMask::from_image(&rgba));
                let bevy_img = crate::assets::dynamic_to_bevy_image(image::DynamicImage::ImageRgba8(rgba));
                let tex = images.add(bevy_img);
                dir_materials[dir] = materials.add(StandardMaterial {
                    unlit: true,
                    base_color_texture: Some(tex),
                    alpha_mode: AlphaMode::Mask(0.5),
                    double_sided: true,
                    cull_mode: None,
                    perceptual_roughness: 1.0,
                    reflectance: 0.0,
                    ..default()
                });
            } else if dir > 0 {
                dir_materials[dir] = dir_materials[0].clone();
                dir_masks[dir] = dir_masks[0].clone();
            }
        }
        frames.push(dir_materials);
        frame_masks.push(dir_masks);
    }

    (frames, frame_masks, max_w as f32, max_h as f32)
```

Note: the original used `DynamicImage` throughout; this converts to `RgbaImage` earlier so we can build the mask from it. Remove the original `let img = if img.width() != max_w ...` block and replace with the above.

- [ ] **Step 5: Update `load_sprite_frames` to thread masks through**

`load_sprite_frames` calls `decode_sprite_frames` and `rebuild_from_cache`. Update its return type and threading:

```rust
pub fn load_sprite_frames(
    root: &str,
    lod_manager: &LodManager,
    images: &mut Assets<Image>,
    materials: &mut Assets<StandardMaterial>,
    cache: &mut Option<&mut SpriteCache>,
    variant: u8,
    min_w: u32,
    min_h: u32,
    palette_id: u16,
) -> (Vec<[Handle<StandardMaterial>; 5]>, Vec<[Arc<AlphaMask>; 5]>, f32, f32) {
    let root = root.trim_end_matches(|c: char| c.is_ascii_digit());
    let key = cache_key(root, variant, min_w, min_h, palette_id);

    if let Some(c) = cache.as_ref()
        && let Some(&(w, h)) = c.dimensions.get(&key)
    {
        let (frames, masks) = rebuild_from_cache(&key, c);
        if !frames.is_empty() {
            return (frames, masks, w, h);
        }
    }

    let mut try_root = root;
    while try_root.len() >= 3 {
        let (frames, frame_masks, w, h) = decode_sprite_frames(
            try_root, lod_manager, images, materials, variant, min_w, min_h, palette_id,
        );
        if !frames.is_empty() {
            store_in_cache(&key, &frames, &frame_masks, w, h, cache);
            return (frames, frame_masks, w, h);
        }
        try_root = &try_root[..try_root.len() - 1];
    }
    (Vec::new(), Vec::new(), 0.0, 0.0)
}
```

- [ ] **Step 6: Update `load_entity_sprites` to return masks**

Change the return type to `(Vec<Vec<[Handle<StandardMaterial>; 5]>>, Vec<Vec<[Arc<AlphaMask>; 5]>>, f32, f32)`.

Thread the mask data from each `load_sprite_frames` call:

```rust
pub fn load_entity_sprites(
    standing_root: &str,
    walking_root: &str,
    lod_manager: &LodManager,
    images: &mut Assets<Image>,
    materials: &mut Assets<StandardMaterial>,
    cache: &mut Option<&mut SpriteCache>,
    variant: u8,
    palette_id: u16,
) -> (Vec<Vec<[Handle<StandardMaterial>; 5]>>, Vec<Vec<[Arc<AlphaMask>; 5]>>, f32, f32) {
    let (walking, walking_masks, ww, wh) = load_sprite_frames(
        walking_root, lod_manager, images, materials, cache, variant, 0, 0, palette_id,
    );
    let (standing, standing_masks, sw, sh) = load_sprite_frames(
        standing_root, lod_manager, images, materials, cache, variant, ww as u32, wh as u32, palette_id,
    );
    if standing.is_empty() {
        return (Vec::new(), Vec::new(), 0.0, 0.0);
    }
    let qw = sw.max(ww);
    let qh = sh.max(wh);
    let (walking, walking_masks) = if !walking.is_empty() && (sw > ww || sh > wh) {
        let (padded, padded_masks, _, _) = load_sprite_frames(
            walking_root, lod_manager, images, materials, cache, variant, qw as u32, qh as u32, palette_id,
        );
        (padded, padded_masks)
    } else {
        (walking, walking_masks)
    };
    let mut states = vec![standing];
    let mut state_masks = vec![standing_masks];
    if !walking.is_empty() {
        states.push(walking);
        state_masks.push(walking_masks);
    }
    (states, state_masks, qw, qh)
}
```

- [ ] **Step 7: Extend `load_decoration_directions` to return masks**

Change return type to `([Handle<StandardMaterial>; 5], [Arc<AlphaMask>; 5], f32, f32)`.

In the cache-hit path, also rebuild masks:

```rust
    if let Some(c) = cache.as_ref()
        && let Some(&(w, h)) = c.dimensions.get(&key)
    {
        let mut dirs: [Handle<StandardMaterial>; 5] = Default::default();
        let fallback = Arc::new(AlphaMask { width: 1, height: 1, data: vec![true] });
        let mut masks: [Arc<AlphaMask>; 5] = std::array::from_fn(|_| fallback.clone());
        let mut found = true;
        for di in 0..5 {
            let mat_key = format!("{}a{}", key, di);
            if let Some(mat) = c.materials.get(&mat_key) {
                dirs[di] = mat.clone();
                masks[di] = c.masks.get(&mat_key).cloned().unwrap_or_else(|| fallback.clone());
            } else {
                found = false;
                break;
            }
        }
        if found {
            return (dirs, masks, w, h);
        }
    }
```

In the decode path, after padding each image, build the mask before creating the material:

```rust
    let fallback_mask = Arc::new(AlphaMask { width: 1, height: 1, data: vec![true] });
    let mut dirs: [Handle<StandardMaterial>; 5] = Default::default();
    let mut dir_masks: [Arc<AlphaMask>; 5] = std::array::from_fn(|_| fallback_mask.clone());
    for (dir, img_opt) in raw.into_iter().enumerate() {
        if let Some(img) = img_opt {
            let rgba = img.into_rgba8();
            let rgba = if rgba.width() != max_w || rgba.height() != max_h {
                let mut padded = image::RgbaImage::new(max_w, max_h);
                let x_off = (max_w - rgba.width()) / 2;
                let y_off = max_h - rgba.height();
                for py in 0..rgba.height() {
                    for px in 0..rgba.width() {
                        padded.put_pixel(px + x_off, py + y_off, *rgba.get_pixel(px, py));
                    }
                }
                padded
            } else {
                rgba
            };
            dir_masks[dir] = Arc::new(AlphaMask::from_image(&rgba));
            let bevy_img = crate::assets::dynamic_to_bevy_image(image::DynamicImage::ImageRgba8(rgba));
            let tex = images.add(bevy_img);
            dirs[dir] = materials.add(StandardMaterial {
                unlit: true,
                base_color_texture: Some(tex),
                alpha_mode: AlphaMode::Mask(0.5),
                double_sided: true,
                cull_mode: None,
                perceptual_roughness: 1.0,
                reflectance: 0.0,
                ..default()
            });
        } else if dir > 0 {
            dirs[dir] = dirs[0].clone();
            dir_masks[dir] = dir_masks[0].clone();
        }
    }

    // Store in cache
    if let Some(c) = cache.as_mut() {
        c.dimensions.insert(key.clone(), (max_w as f32, max_h as f32));
        for di in 0..5 {
            let mat_key = format!("{}a{}", key, di);
            c.materials.insert(mat_key.clone(), dirs[di].clone());
            c.masks.insert(mat_key, dir_masks[di].clone());
        }
    }

    (dirs, dir_masks, max_w as f32, max_h as f32)
```

Remove the old `c.materials.insert(...)` block that was at the end of this function.

- [ ] **Step 8: Extend `SpriteSheet` with `state_masks` and `current_mask`**

Update the struct and `new`:

```rust
#[derive(Component)]
pub struct SpriteSheet {
    pub states: Vec<Vec<[Handle<StandardMaterial>; 5]>>,
    pub state_dimensions: Vec<(f32, f32)>,
    /// Per-state, per-frame, per-direction alpha masks (parallel to `states`).
    pub state_masks: Vec<Vec<[Arc<AlphaMask>; 5]>>,
    /// The mask for the currently displayed (state, frame, direction).
    pub current_mask: Option<Arc<AlphaMask>>,
    pub current_frame: usize,
    pub current_state: usize,
    pub frame_timer: f32,
    pub frame_duration: f32,
    last_applied: (usize, usize, usize),
}

impl SpriteSheet {
    pub fn new(
        states: Vec<Vec<[Handle<StandardMaterial>; 5]>>,
        state_dimensions: Vec<(f32, f32)>,
        state_masks: Vec<Vec<[Arc<AlphaMask>; 5]>>,
    ) -> Self {
        Self {
            states,
            state_dimensions,
            state_masks,
            current_mask: None,
            current_frame: 0,
            current_state: 0,
            frame_timer: 0.0,
            frame_duration: 0.15,
            last_applied: (usize::MAX, usize::MAX, usize::MAX),
        }
    }
}
```

- [ ] **Step 9: Update `update_sprite_sheets` to swap `current_mask`**

In the frame-change branch (where `current_key != last_applied`), add mask update:

```rust
        if current_key != sprites.last_applied {
            sprites.last_applied = current_key;
            let new_mat = sprites.states[state_idx][sprites.current_frame][direction].clone();
            *mat_handle = MeshMaterial3d(new_mat);
            // Update current mask to match displayed frame
            if state_idx < sprites.state_masks.len()
                && sprites.current_frame < sprites.state_masks[state_idx].len()
            {
                sprites.current_mask =
                    Some(sprites.state_masks[state_idx][sprites.current_frame][direction].clone());
            }
        }
```

- [ ] **Step 10: Fix all call sites in `odm.rs` and `blv.rs`**

In `odm.rs`, all `load_entity_sprites` calls now return a 4-tuple. Update each:

```rust
// Before:
let (s2, w2, h2) = sprites::load_entity_sprites(...);
// ...
sprites::SpriteSheet::new(states, vec![(sw, sh)])

// After:
let (s2, m2, w2, h2) = sprites::load_entity_sprites(...);
// ...
sprites::SpriteSheet::new(states, vec![(sw, sh)], state_masks)
```

There are 3 spawn blocks in `odm.rs`: billboards with `SpriteSheet`, NPC actors, and monsters. Each needs the `state_masks` variable threaded through. For billboards using `load_decoration_directions`:

```rust
// Before:
let (dirs, sw, sh) = sprites::load_decoration_directions(...);
let states = vec![vec![dirs]];
sprites::SpriteSheet::new(states, vec![(sw, sh)])

// After:
let (dirs, dir_masks, sw, sh) = sprites::load_decoration_directions(...);
let states = vec![vec![dirs]];
let state_masks = vec![vec![dir_masks]];
sprites::SpriteSheet::new(states, vec![(sw, sh)], state_masks)
```

Apply same pattern in `blv.rs` for actor spawns.

- [ ] **Step 11: Verify it compiles and tests pass**

```bash
cargo test -p openmm 2>&1 | tail -10
```

Expected: compiles, all tests pass.

- [ ] **Step 12: Commit**

```bash
git add openmm/src/game/entities/sprites.rs openmm/src/game/odm.rs openmm/src/game/blv.rs
git commit --no-gpg-sign -m "feat: thread AlphaMask through sprite loading pipeline into SpriteSheet"
```

---

### Task 5: Add `billboard_hit_test` to `raycast.rs`

**Files:**
- Modify: `openmm/src/game/raycast.rs`

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)]` block in `raycast.rs`:

```rust
    #[test]
    fn billboard_hit_center() {
        // Billboard at origin, facing -Z (towards camera at +Z)
        // Camera ray from (0,0,10) going -Z
        let origin = Vec3::new(0.0, 0.0, 10.0);
        let dir = Vec3::NEG_Z;
        let center = Vec3::ZERO;
        let rotation = bevy::math::Quat::IDENTITY; // faces -Z
        let t = billboard_hit_test(origin, dir, center, rotation, 50.0, 50.0, None);
        assert!(t.is_some());
        assert!((t.unwrap() - 10.0).abs() < 0.1);
    }

    #[test]
    fn billboard_hit_miss_too_far_right() {
        let origin = Vec3::new(0.0, 0.0, 10.0);
        let dir = Vec3::NEG_Z;
        let center = Vec3::new(200.0, 0.0, 0.0); // billboard 200 units to the right
        let t = billboard_hit_test(origin, dir, center, bevy::math::Quat::IDENTITY, 50.0, 50.0, None);
        assert!(t.is_none());
    }

    #[test]
    fn billboard_hit_transparent_pixel_misses() {
        use crate::game::entities::sprites::AlphaMask;
        // 2x2 mask — only bottom-left pixel is opaque
        let mask = AlphaMask { width: 2, height: 2, data: vec![true, false, false, false] };
        // Ray hits top-right corner: u~0.75, v~0.25 → transparent
        let origin = Vec3::new(25.0, 25.0, 10.0); // offset right and up from center
        let dir = Vec3::NEG_Z;
        let center = Vec3::ZERO;
        let t = billboard_hit_test(origin, dir, center, bevy::math::Quat::IDENTITY, 50.0, 50.0, Some(&mask));
        assert!(t.is_none());
    }

    #[test]
    fn billboard_hit_opaque_pixel_hits() {
        use crate::game::entities::sprites::AlphaMask;
        // 2x2 mask — only bottom-left pixel is opaque (u<0.5, v>0.5)
        let mask = AlphaMask { width: 2, height: 2, data: vec![true, false, false, false] };
        // Ray hits bottom-left: local_x = -25, local_y = -25 → u=0.25, v=0.75
        let origin = Vec3::new(-25.0, -25.0, 10.0);
        let dir = Vec3::NEG_Z;
        let center = Vec3::ZERO;
        let t = billboard_hit_test(origin, dir, center, bevy::math::Quat::IDENTITY, 50.0, 50.0, Some(&mask));
        assert!(t.is_some());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test -p openmm raycast::tests::billboard 2>&1 | head -10
```

Expected: compile error — `billboard_hit_test` not defined.

- [ ] **Step 3: Add the import and implement `billboard_hit_test`**

Add the import at the top of `raycast.rs`:

```rust
use crate::game::entities::sprites::AlphaMask;
```

Add after the geometry functions:

```rust
/// Test whether the camera forward ray hits a billboard sprite at pixel level.
///
/// - `center`: world-space center of the billboard (from `GlobalTransform::translation`)
/// - `rotation`: the billboard's current Y-axis rotation (from `Transform::rotation`)
/// - `half_w`, `half_h`: half the sprite's world-space width and height
/// - `mask`: optional alpha mask; if `None`, the full quad counts as opaque
///
/// Returns the ray distance `t` if the ray hits an opaque pixel, or `None` on miss.
pub fn billboard_hit_test(
    ray_origin: Vec3,
    ray_dir: Vec3,
    center: Vec3,
    rotation: bevy::math::Quat,
    half_w: f32,
    half_h: f32,
    mask: Option<&AlphaMask>,
) -> Option<f32> {
    // Billboard plane normal = direction the sprite faces (rotation * +Z)
    let normal = rotation * Vec3::Z;
    let plane_dist = normal.dot(center);

    let t = ray_plane_intersect(ray_origin, ray_dir, normal, plane_dist)?;

    let hit = ray_origin + ray_dir * t;
    let delta = hit - center;

    // Project onto billboard local axes
    let right = rotation * Vec3::X;
    let local_x = delta.dot(right);  // horizontal, ±half_w
    let local_y = delta.y;           // vertical, ±half_h (billboards stay upright)

    if local_x.abs() > half_w || local_y.abs() > half_h {
        return None; // Outside quad bounds
    }

    // UV: u in [0,1] left-to-right, v in [0,1] top-to-bottom
    let u = local_x / (half_w * 2.0) + 0.5;
    let v = 0.5 - local_y / (half_h * 2.0);

    if let Some(mask) = mask {
        if !mask.test(u, v) {
            return None; // Transparent pixel
        }
    }

    Some(t)
}
```

- [ ] **Step 4: Run tests — they must pass**

```bash
cargo test -p openmm raycast 2>&1
```

Expected: all 11 tests pass.

- [ ] **Step 5: Commit**

```bash
git add openmm/src/game/raycast.rs
git commit --no-gpg-sign -m "feat: add billboard_hit_test with alpha mask support to raycast.rs"
```

---

### Task 6: Add `MonsterInteractable` and wire into spawn sites

**Files:**
- Modify: `openmm/src/game/interaction.rs`
- Modify: `openmm/src/game/odm.rs`
- Modify: `openmm/src/game/blv.rs`

- [ ] **Step 1: Add `MonsterInteractable` to `interaction.rs`**

Add after the `NpcInteractable` struct (around line 36):

```rust
/// Component on monster entities for hover name display.
/// No click action yet — combat system not implemented.
#[derive(Component)]
pub struct MonsterInteractable {
    pub name: String,
}
```

- [ ] **Step 2: Add `MonsterInteractable` to monster spawns in `odm.rs`**

In `odm.rs`, find the monster spawn block (around line 740, where `EntityKind::Monster` is set). After the `SpriteSheet::new(...)` insert, add:

```rust
.insert(crate::game::interaction::MonsterInteractable {
    name: monster.name.clone(),
})
```

The `monster.name` field comes from `Actor.name` — check the existing actor spawn to confirm the field name is `name`.

- [ ] **Step 3: Add `MonsterInteractable` to monster spawns in `blv.rs`**

Find the BLV monster spawn block (where `EntityKind::Monster` is set in `blv.rs`). Add the same insert:

```rust
.insert(crate::game::interaction::MonsterInteractable {
    name: actor.name.clone(),
})
```

- [ ] **Step 4: Verify it compiles**

```bash
cargo build -p openmm 2>&1 | grep -E "^error" | head -10
```

Expected: no errors.

- [ ] **Step 5: Commit**

```bash
git add openmm/src/game/interaction.rs openmm/src/game/odm.rs openmm/src/game/blv.rs
git commit --no-gpg-sign -m "feat: add MonsterInteractable component for hover name display"
```

---

### Task 7: Rewrite interact and hover systems to use `billboard_hit_test`

**Files:**
- Modify: `openmm/src/game/interaction.rs`

- [ ] **Step 1: Update imports at the top of `interaction.rs`**

Replace existing `use` statements with:

```rust
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use crate::GameState;
use crate::game::event_dispatch::EventQueue;
use crate::game::events::MapEvents;
use crate::game::hud::{FooterText, HudView, OverlayImage};
use crate::game::player::PlayerCamera;
use crate::game::raycast::{billboard_hit_test, resolve_event_name};
use crate::game::blv::ClickableFaces;
use crate::game::raycast::{ray_plane_intersect, point_in_polygon};
use crate::game::entities::sprites::SpriteSheet;
```

- [ ] **Step 2: Replace `decoration_interact_system`**

Delete the old `decoration_interact_system` and `find_nearest_decoration` functions. Add:

```rust
/// Detect click on a decoration and push its events. Uses billboard hit test with alpha mask.
fn decoration_interact_system(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    gamepads: Query<&Gamepad>,
    camera_query: Query<(&GlobalTransform, &Camera), With<PlayerCamera>>,
    decorations: Query<(&DecorationInfo, &GlobalTransform, &Transform, &SpriteSheet)>,
    map_events: Option<Res<MapEvents>>,
    mut event_queue: ResMut<EventQueue>,
    cursor_query: Query<&CursorOptions, With<PrimaryWindow>>,
) {
    let Ok((cam_global, _)) = camera_query.single() else { return };
    let (key, click, gamepad) = check_interact_input(&keys, &mouse, &gamepads);
    if !key && !click && !gamepad { return }
    let cursor_grabbed = cursor_query.single()
        .map(|c| !matches!(c.grab_mode, CursorGrabMode::None)).unwrap_or(true);
    if click && !cursor_grabbed { return }

    let origin = cam_global.translation();
    let dir = cam_global.forward().as_vec3();

    let mut nearest: Option<(f32, u16)> = None;
    for (info, g_tf, tf, sheet) in decorations.iter() {
        let (sw, sh) = sheet.state_dimensions[sheet.current_state];
        if let Some(t) = billboard_hit_test(
            origin, dir, g_tf.translation(), tf.rotation,
            sw / 2.0, sh / 2.0, sheet.current_mask.as_deref(),
        ) {
            if nearest.is_none() || t < nearest.unwrap().0 {
                nearest = Some((t, info.event_id));
            }
        }
    }

    let Some((_, event_id)) = nearest else { return };
    let Some(me) = map_events else { return };
    let Some(evt) = me.evt.as_ref() else { return };
    event_queue.push_all(event_id, evt);
}
```

- [ ] **Step 3: Replace `npc_interact_system`**

Delete the old `npc_interact_system` and `find_nearest_npc` functions. Add:

```rust
/// Detect click on an NPC and push a SpeakNPC event. Uses billboard hit test with alpha mask.
fn npc_interact_system(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    gamepads: Query<&Gamepad>,
    camera_query: Query<(&GlobalTransform, &Camera), With<PlayerCamera>>,
    npcs: Query<(&NpcInteractable, &GlobalTransform, &Transform, &SpriteSheet)>,
    mut event_queue: ResMut<EventQueue>,
    cursor_query: Query<&CursorOptions, With<PrimaryWindow>>,
) {
    let Ok((cam_global, _)) = camera_query.single() else { return };
    let (key, click, gamepad) = check_interact_input(&keys, &mouse, &gamepads);
    if !key && !click && !gamepad { return }
    let cursor_grabbed = cursor_query.single()
        .map(|c| !matches!(c.grab_mode, CursorGrabMode::None)).unwrap_or(true);
    if click && !cursor_grabbed { return }

    let origin = cam_global.translation();
    let dir = cam_global.forward().as_vec3();

    let mut nearest: Option<(f32, i16)> = None;
    for (info, g_tf, tf, sheet) in npcs.iter() {
        let (sw, sh) = sheet.state_dimensions[sheet.current_state];
        if let Some(t) = billboard_hit_test(
            origin, dir, g_tf.translation(), tf.rotation,
            sw / 2.0, sh / 2.0, sheet.current_mask.as_deref(),
        ) {
            if nearest.is_none() || t < nearest.unwrap().0 {
                nearest = Some((t, info.npc_id));
            }
        }
    }

    let Some((_, npc_id)) = nearest else { return };
    event_queue.push_single(lod::evt::GameEvent::SpeakNPC { npc_id: npc_id as i32 });
}
```

- [ ] **Step 4: Rewrite `hover_hint_system`**

Delete the old `hover_hint_system` and all `resolve_*` functions. Add:

```rust
/// Show the nearest interactive object's name in the footer — pixel-accurate for all types.
fn hover_hint_system(
    camera_query: Query<(&GlobalTransform, &Camera), With<PlayerCamera>>,
    clickable_faces: Option<Res<ClickableFaces>>,
    decorations: Query<(&DecorationInfo, &GlobalTransform, &Transform, &SpriteSheet)>,
    npcs: Query<(&NpcInteractable, &GlobalTransform, &Transform, &SpriteSheet)>,
    monsters: Query<(&MonsterInteractable, &GlobalTransform, &Transform, &SpriteSheet)>,
    map_events: Option<Res<MapEvents>>,
    mut footer: ResMut<FooterText>,
) {
    let Ok((cam_global, _)) = camera_query.single() else { return };
    let origin = cam_global.translation();
    let dir = cam_global.forward().as_vec3();

    let mut nearest: Option<(f32, String)> = None;

    // BSP faces (outdoor and indoor) via ClickableFaces
    if let Some(faces) = clickable_faces.as_ref() {
        for face in &faces.faces {
            if let Some(t) = ray_plane_intersect(origin, dir, face.normal, face.plane_dist) {
                if t > crate::game::blv::OUTDOOR_INTERACT_RANGE {
                    continue;
                }
                let hit = origin + dir * t;
                if point_in_polygon(hit, &face.vertices, face.normal) {
                    if let Some(name) = resolve_event_name(face.event_id, &map_events) {
                        if nearest.is_none() || t < nearest.as_ref().unwrap().0 {
                            nearest = Some((t, name));
                        }
                    }
                }
            }
        }
    }

    // Decorations
    for (info, g_tf, tf, sheet) in decorations.iter() {
        let (sw, sh) = sheet.state_dimensions[sheet.current_state];
        if let Some(t) = billboard_hit_test(
            origin, dir, g_tf.translation(), tf.rotation,
            sw / 2.0, sh / 2.0, sheet.current_mask.as_deref(),
        ) {
            if nearest.is_none() || t < nearest.as_ref().unwrap().0 {
                if let Some(name) = resolve_event_name(info.event_id, &map_events) {
                    nearest = Some((t, name));
                }
            }
        }
    }

    // NPCs
    for (info, g_tf, tf, sheet) in npcs.iter() {
        let (sw, sh) = sheet.state_dimensions[sheet.current_state];
        if let Some(t) = billboard_hit_test(
            origin, dir, g_tf.translation(), tf.rotation,
            sw / 2.0, sh / 2.0, sheet.current_mask.as_deref(),
        ) {
            if nearest.is_none() || t < nearest.as_ref().unwrap().0 {
                nearest = Some((t, info.name.clone()));
            }
        }
    }

    // Monsters
    for (info, g_tf, tf, sheet) in monsters.iter() {
        let (sw, sh) = sheet.state_dimensions[sheet.current_state];
        if let Some(t) = billboard_hit_test(
            origin, dir, g_tf.translation(), tf.rotation,
            sw / 2.0, sh / 2.0, sheet.current_mask.as_deref(),
        ) {
            if nearest.is_none() || t < nearest.as_ref().unwrap().0 {
                nearest = Some((t, info.name.clone()));
            }
        }
    }

    match nearest {
        Some((_, name)) => footer.set(&name),
        None => footer.clear(),
    }
}
```

Note: `crate::game::blv::OUTDOOR_INTERACT_RANGE` needs to be made `pub(crate)` — see Task 8 where we expose the constant.

- [ ] **Step 5: Update `InteractionPlugin` system registration**

The `interact_system` is removed (BSP faces handled by `indoor_interact_system` in `blv.rs`). Update `build`:

```rust
impl Plugin for InteractionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                hover_hint_system,
                decoration_interact_system,
                npc_interact_system,
            )
                .chain()
                .run_if(in_state(GameState::Game))
                .run_if(resource_equals(HudView::World)),
        )
        .add_systems(
            Update,
            interaction_input
                .run_if(in_state(GameState::Game))
                .run_if(|view: Res<HudView>| matches!(*view, HudView::Building | HudView::NpcDialogue | HudView::Chest))
                .after(crate::game::player::PlayerInputSet),
        );
    }
}
```

- [ ] **Step 6: Verify it compiles**

```bash
cargo build -p openmm 2>&1 | grep "^error" | head -20
```

Fix any type errors. The most likely issues: `ClickableFaces` import, `OUTDOOR_INTERACT_RANGE` visibility — fix those first.

- [ ] **Step 7: Run all tests**

```bash
cargo test -p openmm 2>&1 | tail -10
```

Expected: all tests pass.

- [ ] **Step 8: Commit**

```bash
git add openmm/src/game/interaction.rs
git commit --no-gpg-sign -m "feat: rewrite hover/interact systems with pixel-accurate billboard hit testing"
```

---

### Task 8: Add indoor hover hints and expose range constant

**Files:**
- Modify: `openmm/src/game/blv.rs`

The `indoor_interact_system` already handles clicks on indoor faces. This task adds the hover equivalent: show the face's event name in the footer while the crosshair is over it.

- [ ] **Step 1: Make `INDOOR_INTERACT_RANGE` pub(crate) and add `OUTDOOR_INTERACT_RANGE`**

In `blv.rs`, change:

```rust
const INDOOR_INTERACT_RANGE: f32 = 5120.0;
```

to:

```rust
pub(crate) const INDOOR_INTERACT_RANGE: f32 = 5120.0;
/// Outdoor BSP face click range — matches original MM6 outdoor interaction distance.
pub(crate) const OUTDOOR_INTERACT_RANGE: f32 = 2000.0;
```

Update the reference in `hover_hint_system` in `interaction.rs` to use `OUTDOOR_INTERACT_RANGE` for the face range check (you may want to use `INDOOR_INTERACT_RANGE` when indoors — for simplicity use `INDOOR_INTERACT_RANGE` for all `ClickableFaces` since it's the larger of the two and both map types use the same resource).

Actually for simplicity: use `INDOOR_INTERACT_RANGE` for all ClickableFaces regardless of map type — it's 5120 which is fine for outdoors too. Remove `OUTDOOR_INTERACT_RANGE` if unused.

- [ ] **Step 2: Add `indoor_hover_hint_system`**

Add after `indoor_interact_system` in `blv.rs`:

```rust
/// Show event name in footer when the crosshair is over a clickable indoor face.
fn indoor_hover_hint_system(
    camera_query: Query<(&GlobalTransform, &Camera), With<crate::game::player::PlayerCamera>>,
    clickable: Option<Res<ClickableFaces>>,
    map_events: Option<Res<crate::game::events::MapEvents>>,
    mut footer: ResMut<crate::game::hud::FooterText>,
) {
    let Some(clickable) = clickable else { return };
    let Ok((cam_global, _)) = camera_query.single() else { return };
    let origin = cam_global.translation();
    let dir = cam_global.forward().as_vec3();

    let mut nearest: Option<(f32, String)> = None;
    for face in &clickable.faces {
        if let Some(t) = crate::game::raycast::ray_plane_intersect(origin, dir, face.normal, face.plane_dist) {
            if t > INDOOR_INTERACT_RANGE {
                continue;
            }
            let hit = origin + dir * t;
            if crate::game::raycast::point_in_polygon(hit, &face.vertices, face.normal) {
                if let Some(name) = crate::game::raycast::resolve_event_name(face.event_id, &map_events) {
                    if nearest.is_none() || t < nearest.as_ref().unwrap().0 {
                        nearest = Some((t, name));
                    }
                }
            }
        }
    }

    if let Some((_, name)) = nearest {
        footer.set(&name);
    }
}
```

- [ ] **Step 3: Register `indoor_hover_hint_system` in `BlvPlugin`**

In `BlvPlugin::build`, add the system to the existing chain:

```rust
app.add_systems(OnEnter(GameState::Game), spawn_indoor_world)
    .add_systems(
        Update,
        (
            indoor_interact_system,
            indoor_hover_hint_system,
            indoor_touch_trigger_system,
            door_animation_system,
        )
            .run_if(in_state(GameState::Game))
            .run_if(resource_equals(HudView::World)),
    );
```

Note: `indoor_hover_hint_system` runs every frame and calls `footer.set()` — this is correct since `FooterText::set` is a soft set that won't overwrite locked status messages. However, it will fight with `hover_hint_system` in `InteractionPlugin`. Since both now test `ClickableFaces`, the `hover_hint_system` already handles BSP faces for both indoor and outdoor. Remove `indoor_hover_hint_system` if `hover_hint_system` covers it — or keep it as a backup. Since `hover_hint_system` in `interaction.rs` now queries `ClickableFaces` for all map types, `indoor_hover_hint_system` is redundant. **Do not add it.** The `BlvPlugin` system list stays as is (no `indoor_hover_hint_system`).

- [ ] **Step 4: Verify it compiles and tests pass**

```bash
cargo test -p openmm 2>&1 | tail -10
```

Expected: all tests pass, no compile errors.

- [ ] **Step 5: Commit**

```bash
git add openmm/src/game/blv.rs
git commit --no-gpg-sign -m "refactor: expose interact range constants from blv.rs"
```

---

### Task 9: Remove `BuildingInfo` and dead cone code

**Files:**
- Modify: `openmm/src/game/interaction.rs`
- Modify: `openmm/src/game/odm.rs`

At this point `BuildingInfo`, `interact_system`, `raycast_nearest`, `find_nearest_building`, `make_building_info`, `resolve_building_name`, and `resolve_decoration_name` are all unused.

- [ ] **Step 1: Delete dead code from `interaction.rs`**

Remove:
- `BuildingInfo` struct (lines 14–18)
- `make_building_info` function (lines 76–82)
- `INTERACT_RANGE`, `RAYCAST_RANGE`, `RAY_ANGLE_TAN`, `RAY_MIN_PERP` constants
- `raycast_nearest` function
- `find_nearest_building` function
- `interact_system` function
- `resolve_building_name` function
- `resolve_decoration_name` function (was already unused since Task 2)

Keep: `DecorationInfo`, `NpcInteractable`, `MonsterInteractable`, `check_interact_input`, `check_exit_input`, `decoration_interact_system`, `npc_interact_system`, `hover_hint_system`, `interaction_input`, `InteractionPlugin`.

- [ ] **Step 2: Remove `BuildingInfo` inserts from `odm.rs`**

In `odm.rs`, find all `.insert(crate::game::interaction::make_building_info(...))` or `.insert(BuildingInfo { ... })` calls and delete them along with any imports of `make_building_info`.

- [ ] **Step 3: Run `make lint`**

```bash
cd /home/roarc/repos/openmm && make lint 2>&1 | head -30
```

Expected: no warnings about unused imports or dead code. Fix any clippy lint that appears.

- [ ] **Step 4: Run all tests**

```bash
cargo test -p openmm 2>&1 | tail -10
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add openmm/src/game/interaction.rs openmm/src/game/odm.rs
git commit --no-gpg-sign -m "refactor: remove BuildingInfo cone system, all interaction now pixel-accurate"
```

---

### Task 10: Final verification and `make fix`

- [ ] **Step 1: Run `make fix`**

```bash
cd /home/roarc/repos/openmm && make fix 2>&1
```

- [ ] **Step 2: Run `make lint`**

```bash
make lint 2>&1
```

Expected: no warnings, no errors.

- [ ] **Step 3: Run full test suite**

```bash
make test 2>&1 | tail -20
```

Expected: all tests pass.

- [ ] **Step 4: Commit any formatting fixes**

```bash
git add -p
git commit --no-gpg-sign -m "style: apply clippy and fmt fixes"
```

---

## Self-Review

**Spec coverage check:**
- ✅ `ray_plane_intersect`, `point_in_polygon` → Task 1
- ✅ `resolve_event_name` (unified) → Task 2
- ✅ `AlphaMask` struct → Task 3
- ✅ `SpriteCache.masks`, `SpriteSheet.state_masks`, `SpriteSheet.current_mask`, `update_sprite_sheets` → Task 4
- ✅ `billboard_hit_test` → Task 5
- ✅ `MonsterInteractable` → Task 6
- ✅ Rewritten interact/hover systems → Task 7
- ✅ Indoor hover hints → covered by `hover_hint_system` querying `ClickableFaces`; Task 8 exposes the constant
- ✅ Remove `BuildingInfo` / cone code → Task 9
- ✅ `make lint` → Task 10

**Type consistency:**
- `AlphaMask` defined Task 3, used in Task 4 and Task 5 — consistent
- `billboard_hit_test` signature defined Task 5, called Task 7 — `(origin, dir, g_tf.translation(), tf.rotation, sw/2, sh/2, sheet.current_mask.as_deref())` matches
- `state_masks: Vec<Vec<[Arc<AlphaMask>; 5]>>` defined Task 4 Step 8, populated Task 4 Step 10 — consistent
- `resolve_event_name` defined Task 2, called Task 7 — signature matches
- `ClickableFaces` imported in `interaction.rs` Task 7, already `pub struct` in `blv.rs` — confirmed accessible

**Placeholders:** None found.
