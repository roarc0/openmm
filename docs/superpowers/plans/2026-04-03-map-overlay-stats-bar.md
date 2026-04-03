# Map Overlay & Stats Bar Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an M-key fullscreen map overlay (blocked gameplay, centered with margin) and consolidate the gold/food display into a single yellow text line over the border1 sidebar.

**Architecture:** `HudView::Map` extends the existing freeze/gate machinery for free. `MapOverviewImage` resource stores the handle from `spawn_hud` and is cleared by `loading_setup` on map change. `map_overlay.rs` owns all map overlay logic; `stats_bar.rs` is simplified to one node.

**Tech Stack:** Bevy 0.18 ECS, `ButtonInput<KeyCode>`, `GameFonts`/`smallnum`, LOD icon loader.

---

### Task 1: Consolidate stats bar to a single text node

**Files:**
- Modify: `openmm/src/game/hud/stats_bar.rs`

- [ ] **Step 1: Replace the two marker components with one**

Replace the entire contents of `stats_bar.rs` with:

```rust
//! Gold and food display on the border1 sidebar.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::config::GameConfig;
use crate::fonts::{GameFonts, YELLOW};
use crate::game::world_state::WorldState;
use crate::ui_assets::UiAssets;

use super::borders::*;

/// Marker for the combined food + gold text image node.
#[derive(Component)]
pub(super) struct HudStatsText;

/// Spawn the combined stats text node as a child of the HUD root.
pub(super) fn spawn_stats_bar(parent: &mut ChildSpawnerCommands) {
    parent.spawn((
        Name::new("hud_stats_text"),
        ImageNode::new(Handle::default()),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Auto,
            height: Val::Auto,
            ..default()
        },
        Visibility::Hidden,
        HudStatsText,
        super::HudUI,
    ));
}

/// Update the combined food + gold text when either value changes.
/// Position is updated every frame because it depends on window size.
pub(super) fn update_stats_bar(
    world_state: Option<Res<WorldState>>,
    mut last_gold: Local<i32>,
    mut last_food: Local<i32>,
    game_fonts: Res<GameFonts>,
    ui_assets: Res<UiAssets>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cfg: Res<GameConfig>,
    mut images: ResMut<Assets<Image>>,
    mut stats_q: Query<(&mut ImageNode, &mut Visibility, &mut Node), With<HudStatsText>>,
) {
    let Some(ws) = world_state else { return };
    let gold = ws.game_vars.gold;
    let food = ws.game_vars.food;

    let Ok(window) = windows.single() else { return };
    let sf = window.scale_factor();
    let (_, _, lpw, lph) = letterbox_rect(window, &cfg);
    let lw = lpw as f32 / sf;
    let lh = lph as f32 / sf;
    let d = hud_dimensions(lw, lh, &ui_assets);

    let text_h = d.scale_h(12.0);
    let right = d.scale_w(8.0);
    let top = d.tap_h + d.scale_h(10.0);

    let needs_update = gold != *last_gold || food != *last_food;

    for (mut img_node, mut vis, mut node) in stats_q.iter_mut() {
        // Always update position — depends on window size
        node.right = Val::Px(right);
        node.top = Val::Px(top);
        node.bottom = Val::Auto;
        node.height = Val::Px(text_h);

        if needs_update {
            let text = format!("{}  {}", food, gold);
            if let Some(handle) = game_fonts.render(&text, "smallnum", YELLOW, &mut images) {
                img_node.image = handle;
                *vis = Visibility::Inherited;
            }
        }
    }

    if needs_update {
        *last_gold = gold;
        *last_food = food;
    }
}
```

- [ ] **Step 2: Build to confirm compilation**

```bash
make build 2>&1 | grep -E "error|warning: unused"
```

Expected: no errors. There may be warnings about unused `HudGoldText`/`HudFoodText` — those will be resolved in Step 3.

- [ ] **Step 3: Remove dead component references from mod.rs**

In `openmm/src/game/hud/mod.rs`, the `update_hud_layout` function has no reference to `HudGoldText`/`HudFoodText` — they were only used inside `stats_bar.rs`. Verify with:

```bash
grep -n "HudGoldText\|HudFoodText" openmm/src/game/hud/mod.rs
```

Expected: no output. If any lines appear, remove them.

- [ ] **Step 4: Build and lint**

```bash
make lint 2>&1 | grep "error"
```

Expected: no errors.

- [ ] **Step 5: Commit**

```bash
git add openmm/src/game/hud/stats_bar.rs
git commit --no-gpg-sign -m "feat: consolidate gold/food into single stats text line over border1"
```

---

### Task 2: Add HudView::Map and MapOverviewImage resource

**Files:**
- Modify: `openmm/src/game/hud/mod.rs`
- Modify: `openmm/src/states/loading.rs`

- [ ] **Step 1: Add Map variant to HudView**

In `openmm/src/game/hud/mod.rs`, find the `HudView` enum and add the `Map` variant:

```rust
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HudView {
    #[default]
    World,
    Building,
    NpcDialogue,
    Chest,
    Inventory,
    Stats,
    Rest,
    /// Fullscreen map overlay (M key). Freezes time, blocks input.
    Map,
}
```

- [ ] **Step 2: Define MapOverviewImage resource in mod.rs**

Add this struct after the `HudView` definition (before `pub struct HudPlugin`):

```rust
/// Handle to the current map's overview image for the M-key fullscreen overlay.
/// `None` for indoor maps (no overview icon exists).
#[derive(Resource)]
pub struct MapOverviewImage(pub Option<Handle<Image>>);
```

- [ ] **Step 3: Insert MapOverviewImage in spawn_hud**

In `spawn_hud`, after the line:

```rust
let map_overview = load_map_overview(&map_overview_name, &game_assets, &mut images, &cfg);
```

Add:

```rust
commands.insert_resource(MapOverviewImage(map_overview.clone()));
```

- [ ] **Step 4: Remove MapOverviewImage in loading_setup**

In `openmm/src/states/loading.rs`, find the cleanup block starting at line 270 and add:

```rust
commands.remove_resource::<crate::game::hud::MapOverviewImage>();
```

after the existing `remove_resource` calls.

- [ ] **Step 5: Build to confirm**

```bash
make build 2>&1 | grep "error"
```

Expected: no errors.

- [ ] **Step 6: Commit**

```bash
git add openmm/src/game/hud/mod.rs openmm/src/states/loading.rs
git commit --no-gpg-sign -m "feat: add HudView::Map and MapOverviewImage resource"
```

---

### Task 3: Create map_overlay.rs with layout logic and tests

**Files:**
- Create: `openmm/src/game/hud/map_overlay.rs`

- [ ] **Step 1: Write the failing tests first**

Create `openmm/src/game/hud/map_overlay.rs` with only the pure layout function and its tests:

```rust
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::config::GameConfig;
use crate::ui_assets::UiAssets;

use super::{HudUI, HudView, MapOverviewImage};
use super::overlay::viewport_inner_rect;

/// Marker for the fullscreen map overlay UI node.
#[derive(Component)]
pub(super) struct MapOverlayUI;

/// Compute the overlay rect (left, top, size, size) centered in the inner viewport.
///
/// Applies 10% margin on each side — the display occupies 80% of the available area.
/// The map image is 1:1, so size = min(available_w, available_h).
pub(super) fn map_overlay_rect(
    inner_left: f32,
    inner_top: f32,
    inner_w: f32,
    inner_h: f32,
) -> (f32, f32, f32, f32) {
    let available_w = inner_w * 0.8;
    let available_h = inner_h * 0.8;
    let size = available_w.min(available_h);
    let left = inner_left + (inner_w - size) / 2.0;
    let top = inner_top + (inner_h - size) / 2.0;
    (left, top, size, size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_viewport_limited_by_height() {
        // 800×600: available = 640×480, size = 480 (height wins)
        let (left, top, w, h) = map_overlay_rect(0.0, 0.0, 800.0, 600.0);
        let size = 600.0 * 0.8;
        assert_eq!(w, size);
        assert_eq!(h, size);
        assert!((left - (800.0 - size) / 2.0).abs() < 0.001);
        assert!((top - (600.0 - size) / 2.0).abs() < 0.001);
    }

    #[test]
    fn tall_viewport_limited_by_width() {
        // 400×700 with offset: available = 320×560, size = 320 (width wins)
        let (left, top, w, h) = map_overlay_rect(10.0, 20.0, 400.0, 700.0);
        let size = 400.0 * 0.8;
        assert_eq!(w, size);
        assert_eq!(h, size);
        assert!((left - (10.0 + (400.0 - size) / 2.0)).abs() < 0.001);
        assert!((top - (20.0 + (700.0 - size) / 2.0)).abs() < 0.001);
    }

    #[test]
    fn square_viewport_both_equal() {
        let (_, _, w, h) = map_overlay_rect(0.0, 0.0, 500.0, 500.0);
        assert_eq!(w, 400.0);
        assert_eq!(h, 400.0);
    }
}
```

- [ ] **Step 2: Register the module in mod.rs so the test compiles**

In `openmm/src/game/hud/mod.rs`, add after the existing `mod` declarations:

```rust
mod map_overlay;
```

- [ ] **Step 3: Run the tests — expect them to pass**

```bash
make test 2>&1 | grep -E "map_overlay|FAILED|ok"
```

Expected:
```
test game::hud::map_overlay::tests::wide_viewport_limited_by_height ... ok
test game::hud::map_overlay::tests::tall_viewport_limited_by_width ... ok
test game::hud::map_overlay::tests::square_viewport_both_equal ... ok
```

- [ ] **Step 4: Commit**

```bash
git add openmm/src/game/hud/map_overlay.rs openmm/src/game/hud/mod.rs
git commit --no-gpg-sign -m "feat: add map_overlay module with layout function and tests"
```

---

### Task 4: Add input and spawn/despawn/layout systems to map_overlay.rs

**Files:**
- Modify: `openmm/src/game/hud/map_overlay.rs`

- [ ] **Step 1: Add the four systems after the layout function**

Append these systems to `map_overlay.rs` (after `map_overlay_rect` and before `#[cfg(test)]`):

```rust
/// Toggle the fullscreen map view on M key press.
/// Opens only when in World view and an outdoor map is loaded.
/// Closes from Map view back to World.
pub(super) fn map_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut view: ResMut<HudView>,
    map_image: Option<Res<MapOverviewImage>>,
) {
    if !keys.just_pressed(KeyCode::KeyM) {
        return;
    }
    match *view {
        HudView::World => {
            // Only open if there is an overview image (outdoor map)
            if map_image.as_ref().and_then(|m| m.0.as_ref()).is_some() {
                *view = HudView::Map;
            }
        }
        HudView::Map => {
            *view = HudView::World;
        }
        _ => {}
    }
}

/// Spawn the map overlay image node when HudView is Map and none exists yet.
pub(super) fn spawn_map_overlay(
    mut commands: Commands,
    view: Res<HudView>,
    map_image: Option<Res<MapOverviewImage>>,
    existing: Query<Entity, With<MapOverlayUI>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cfg: Res<GameConfig>,
    ui_assets: Res<UiAssets>,
) {
    if !matches!(*view, HudView::Map) || !existing.is_empty() {
        return;
    }
    let Some(map_image) = map_image else { return };
    let Some(ref handle) = map_image.0 else { return };
    let Ok(window) = windows.single() else { return };

    let (il, it, iw, ih) = viewport_inner_rect(window, &cfg, &ui_assets);
    let (left, top, size, _) = map_overlay_rect(il, it, iw, ih);

    commands.spawn((
        Name::new("map_overlay"),
        ImageNode::new(handle.clone()),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(left),
            top: Val::Px(top),
            width: Val::Px(size),
            height: Val::Px(size),
            ..default()
        },
        MapOverlayUI,
        HudUI,
        crate::game::InGame,
    ));
}

/// Despawn the map overlay node when leaving Map view.
pub(super) fn despawn_map_overlay(
    mut commands: Commands,
    view: Res<HudView>,
    existing: Query<Entity, With<MapOverlayUI>>,
) {
    if matches!(*view, HudView::Map) {
        return;
    }
    for entity in existing.iter() {
        commands.entity(entity).despawn();
    }
}

/// Update overlay position and size on window resize.
pub(super) fn update_map_overlay_layout(
    view: Res<HudView>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cfg: Res<GameConfig>,
    ui_assets: Res<UiAssets>,
    mut query: Query<&mut Node, With<MapOverlayUI>>,
) {
    if !matches!(*view, HudView::Map) {
        return;
    }
    let Ok(window) = windows.single() else { return };
    let (il, it, iw, ih) = viewport_inner_rect(window, &cfg, &ui_assets);
    let (left, top, size, _) = map_overlay_rect(il, it, iw, ih);

    for mut node in query.iter_mut() {
        node.left = Val::Px(left);
        node.top = Val::Px(top);
        node.width = Val::Px(size);
        node.height = Val::Px(size);
    }
}
```

- [ ] **Step 2: Build to confirm**

```bash
make build 2>&1 | grep "error"
```

Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add openmm/src/game/hud/map_overlay.rs
git commit --no-gpg-sign -m "feat: map overlay input, spawn, despawn, and layout systems"
```

---

### Task 5: Wire map_overlay systems into HudPlugin

**Files:**
- Modify: `openmm/src/game/hud/mod.rs`

- [ ] **Step 1: Add the systems to HudPlugin::build**

In `openmm/src/game/hud/mod.rs`, find the `HudPlugin::build` method. The `Update` system chain currently ends with `freeze_system`. Add the four map overlay systems to the chain:

```rust
impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FooterText>()
            .init_resource::<HudView>()
            .add_systems(OnEnter(GameState::Game), spawn_hud)
            .add_systems(
                Update,
                (
                    update_hud_layout,
                    update_minimap,
                    update_footer_text,
                    stats_bar::update_stats_bar,
                    update_viewport,
                    crosshair::update_crosshair,
                    overlay::spawn_overlay,
                    overlay::despawn_overlay,
                    overlay::update_overlay_layout,
                    overlay::spawn_npc_portrait,
                    overlay::despawn_npc_portrait,
                    map_overlay::map_input_system,
                    map_overlay::spawn_map_overlay,
                    map_overlay::despawn_map_overlay,
                    map_overlay::update_map_overlay_layout,
                    freeze_system,
                )
                    .chain()
                    .run_if(in_state(GameState::Game)),
            );
    }
}
```

- [ ] **Step 2: Build**

```bash
make build 2>&1 | grep "error"
```

Expected: no errors.

- [ ] **Step 3: Run lint**

```bash
make lint 2>&1 | grep "error"
```

Expected: no errors.

- [ ] **Step 4: Run the game and verify both features**

```bash
make run map=oute3
```

Verify:
- Stats bar: top of border1 sidebar shows `"7  200"` in yellow on one line, slightly inset from right edge
- M key: pressing M opens the map overlay (centered, with margin, 1:1 square); pressing M again closes it
- Map overlay blocks player movement and freezes game time while open
- Indoor map (e.g. `load out_start.blv` in console): M key does nothing

- [ ] **Step 5: Final commit**

```bash
git add openmm/src/game/hud/mod.rs
git commit --no-gpg-sign -m "feat: wire map overlay systems into HudPlugin"
```
