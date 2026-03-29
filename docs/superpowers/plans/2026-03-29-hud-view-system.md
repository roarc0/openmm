# HUD View System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract the monolithic `hud.rs` into a modular `game/hud/` folder with a `HudView` resource for switching between game views (world, building, chest, inventory, stats, rest), freezing time/rendering when not in the 3D world.

**Architecture:** A `HudView` enum resource controls which view is active. Systems are gated on `HudView::World` instead of `InGameState::Playing`. When a non-World view is active, `Time<Virtual>` is paused. An overlay system renders images in the viewport inner rect (correctly inset within all four borders). The current `interaction.rs` is refactored to use this API.

**Tech Stack:** Rust, Bevy 0.18 ECS

---

## File Structure

```
game/hud/
  mod.rs          — HudPlugin, HudView resource, viewport_inner_rect(), freeze system, re-exports
  borders.rs      — HudDimensions, letterbox_rect, hud_dimensions, border spawning, update_hud_layout, update_viewport
  minimap.rs      — Minimap image, compass strip, tap frames, direction arrows, update_minimap
  footer.rs       — FooterText resource + update_footer_text system
  overlay.rs      — OverlayImage resource, overlay spawn/despawn/resize
```

Files modified:
- `game/mod.rs` — update `hud` module path, remove `InGameState` references, gate terrain_material on `HudView::World`
- `game/interaction.rs` — remove UI overlay code, use `HudView` + `OverlayImage` instead of `InGameState`
- `game/player.rs` — gate on `HudView::World` instead of `InGameState::Playing`
- `game/entities/mod.rs` — gate on `HudView::World` instead of `InGameState::Playing`
- `game/odm.rs` — gate on `HudView::World` instead of `InGameState::Playing`
- `game/physics.rs` — gate on `HudView::World` instead of `InGameState::Playing`
- `game/debug.rs` — update `viewport_rect` import path

---

### Task 1: Create `game/hud/` module skeleton with `HudView` and `viewport_inner_rect`

**Files:**
- Create: `openmm/src/game/hud/mod.rs`
- Create: `openmm/src/game/hud/borders.rs`
- Create: `openmm/src/game/hud/minimap.rs`
- Create: `openmm/src/game/hud/footer.rs`
- Create: `openmm/src/game/hud/overlay.rs`
- Delete: `openmm/src/game/hud.rs`

This task splits the monolithic `hud.rs` into submodules. Every piece of code moves — nothing is rewritten. The only new code is the `HudView` enum, `viewport_inner_rect()`, and the freeze system.

- [ ] **Step 1: Create `game/hud/borders.rs`**

Move these items from `hud.rs` into `borders.rs`:
- Constants: `REF_H`, `REF_W`, `FOOTER_H`, `FOOTER_OVERLAP`, `FOOTER_LIFT`, `FOOTER_EXPOSED_H`
- Marker components: `HudBorder1` through `HudBorder6`, `HudCorner`, `HudMapback`, `HudRoot`, `HudBorder4Left`
- `HudDimensions` struct + `impl` block (`scale_h`, `scale_w`)
- `hud_dimensions()` function
- `parse_aspect_ratio()` function
- `letterbox_rect()` function (make `pub(crate)`)
- `logical_size()` function (make `pub(crate)`)
- `update_hud_layout()` function (make `pub(super)`)
- `update_viewport()` function (make `pub(super)`)
- `viewport_rect()` function (keep `pub`)
- `HudCamera` component

Make `FOOTER_EXPOSED_H` `pub(super)` so `mod.rs` can use it for `viewport_inner_rect`. Make `HudDimensions` and `hud_dimensions` `pub(super)` so `overlay.rs` can compute the inner rect. Make all border marker components `pub(super)` so they remain usable by `update_hud_layout`.

Add these imports at the top of `borders.rs`:

```rust
use bevy::camera::Viewport;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::config::GameConfig;
use crate::game::player::PlayerCamera;
use crate::ui_assets::UiAssets;
```

- [ ] **Step 2: Create `game/hud/minimap.rs`**

Move these items from `hud.rs` into `minimap.rs`:
- `HudMinimapClip`, `HudMinimapImage`, `HudMinimapArrow` components
- `HudCompassClip`, `HudCompassStrip` components
- `MinimapArrows` resource
- `TapFrames` resource
- `update_minimap()` function (make `pub(super)`)
- `load_map_overview()` function (make `pub(super)`)
- `make_tap_key_transparent()` function (make `pub(super)`)

Add import for `borders::hud_dimensions`, `borders::logical_size`, and the border marker components needed for `Without` filters. The `update_minimap` system queries `HudCompassStrip` and `HudMinimapImage` which are local, so no cross-module marker issues.

```rust
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::assets::{self, GameAssets};
use crate::config::GameConfig;
use crate::game::player::Player;
use crate::ui_assets::UiAssets;

use super::borders::{hud_dimensions, logical_size};
```

- [ ] **Step 3: Create `game/hud/footer.rs`**

Move these items from `hud.rs` into `footer.rs`:
- `FooterText` resource + `impl Default` + `impl FooterText`
- `HudFooter`, `HudFooterText` components
- `update_footer_text()` function (make `pub(super)`)

```rust
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::config::GameConfig;
use crate::fonts::GameFonts;
use crate::ui_assets::UiAssets;

use super::borders::viewport_rect;
```

- [ ] **Step 4: Create `game/hud/overlay.rs`**

This is the new overlay system. Create with:

```rust
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::config::GameConfig;
use crate::game::InGame;
use crate::ui_assets::UiAssets;

use super::HudView;
use super::borders::{self, hud_dimensions, letterbox_rect, FOOTER_EXPOSED_H};

/// The image to display as an overlay when a non-World HudView is active.
#[derive(Resource)]
pub struct OverlayImage {
    pub image: Handle<Image>,
}

/// Marker for overlay UI entities managed by this module.
#[derive(Component)]
struct OverlayUI;

/// Spawn the overlay image when OverlayImage resource exists and HudView is not World.
pub(super) fn spawn_overlay(
    mut commands: Commands,
    overlay: Option<Res<OverlayImage>>,
    view: Res<HudView>,
    existing: Query<Entity, With<OverlayUI>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    cfg: Res<GameConfig>,
    ui_assets: Res<UiAssets>,
) {
    // Only spawn if view is not World, overlay resource exists, and no overlay entity yet
    if *view == HudView::World { return; }
    let Some(overlay) = overlay else { return; };
    if !existing.is_empty() { return; }

    let Ok(window) = windows.single() else { return; };
    let (left, top, w, h) = viewport_inner_rect(window, &cfg, &ui_assets);

    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(left),
            top: Val::Px(top),
            width: Val::Px(w),
            height: Val::Px(h),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        OverlayUI,
        InGame,
    )).with_children(|parent| {
        parent.spawn((
            ImageNode::new(overlay.image.clone()),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
        ));
    });
}

/// Despawn overlay when returning to World or when OverlayImage is removed.
pub(super) fn despawn_overlay(
    mut commands: Commands,
    view: Res<HudView>,
    overlay: Option<Res<OverlayImage>>,
    query: Query<Entity, With<OverlayUI>>,
) {
    if *view != HudView::World && overlay.is_some() { return; }
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}

/// Update overlay position on window resize.
pub(super) fn update_overlay_layout(
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    cfg: Res<GameConfig>,
    ui_assets: Res<UiAssets>,
    mut query: Query<&mut Node, With<OverlayUI>>,
) {
    let Ok(window) = windows.single() else { return; };
    let (left, top, w, h) = viewport_inner_rect(window, &cfg, &ui_assets);
    for mut node in query.iter_mut() {
        node.left = Val::Px(left);
        node.top = Val::Px(top);
        node.width = Val::Px(w);
        node.height = Val::Px(h);
    }
}

/// Compute the viewport area inset within all four HUD borders.
/// This is where overlay content should be placed — NOT the 3D camera viewport.
pub fn viewport_inner_rect(window: &Window, cfg: &GameConfig, ui: &UiAssets) -> (f32, f32, f32, f32) {
    let sf = window.scale_factor();
    let (_, _, lpw, lph) = letterbox_rect(window, cfg);
    let lw = lpw as f32 / sf;
    let lh = lph as f32 / sf;
    let d = hud_dimensions(lw, lh, ui);
    let bar_x = (window.width() - lw) / 2.0;
    let bar_y = (window.height() - lh) / 2.0;
    let footer_exposed = d.scale_h(FOOTER_EXPOSED_H);
    let left = bar_x + d.border4_w;
    let top = bar_y + d.border3_h;
    let w = lw - d.border1_w - d.border4_w;
    let h = lh - d.border3_h - d.border2_h - footer_exposed;
    (left, top, w, h)
}
```

- [ ] **Step 5: Create `game/hud/mod.rs`**

Wire everything together:

```rust
use bevy::prelude::*;

use crate::GameState;

pub(crate) mod borders;
pub(crate) mod footer;
pub(crate) mod minimap;
pub(crate) mod overlay;

// Re-exports for external use
pub use borders::viewport_rect;
pub use footer::FooterText;
pub use overlay::{OverlayImage, viewport_inner_rect};

/// Which view is currently active inside the HUD viewport area.
#[derive(Resource, Default, PartialEq, Eq, Clone, Copy, Debug)]
pub enum HudView {
    #[default]
    World,
    Building,
    Chest,
    Inventory,
    Stats,
    Rest,
}

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FooterText>()
            .init_resource::<HudView>()
            .add_systems(OnEnter(GameState::Game), borders::spawn_hud)
            .add_systems(
                Update,
                (
                    borders::update_hud_layout,
                    minimap::update_minimap,
                    footer::update_footer_text,
                    borders::update_viewport,
                    freeze_system,
                    overlay::spawn_overlay,
                    overlay::despawn_overlay,
                    overlay::update_overlay_layout,
                )
                    .chain()
                    .run_if(in_state(GameState::Game)),
            );
    }
}

/// Pause/unpause virtual time based on HudView.
fn freeze_system(
    view: Res<HudView>,
    mut time: ResMut<Time<Virtual>>,
) {
    if !view.is_changed() { return; }
    if *view == HudView::World {
        time.unpause();
    } else {
        time.pause();
    }
}
```

- [ ] **Step 6: Delete old `hud.rs`**

Remove `openmm/src/game/hud.rs`. The `game/hud/` directory replaces it. Rust's module system automatically uses `hud/mod.rs` when `mod hud;` is declared in `game/mod.rs`.

- [ ] **Step 7: Verify it compiles**

Run: `cargo build 2>&1 | head -50`

Fix any import issues from the split. Common things to watch for:
- `spawn_hud` in `borders.rs` needs access to minimap and footer types — it calls `load_map_overview`, uses `HudFooter`, `HudFooterText`, `MinimapArrows`, `TapFrames`, `make_tap_key_transparent`. Either move `spawn_hud` to `mod.rs` (since it touches all submodules) or have `borders.rs` import from sibling modules.
- The `update_hud_layout` function's `ParamSet` queries use markers from multiple submodules — make sure all marker components are accessible.

The pragmatic solution: keep `spawn_hud` in `mod.rs` since it orchestrates all submodules. Move only the layout/dimension/viewport code to `borders.rs`.

- [ ] **Step 8: Commit**

```bash
git add -A openmm/src/game/hud/ openmm/src/game/hud.rs
git commit --no-gpg-sign -m "refactor: split hud.rs into game/hud/ module with HudView and overlay"
```

---

### Task 2: Refactor `interaction.rs` to use `HudView` + `OverlayImage`

**Files:**
- Modify: `openmm/src/game/interaction.rs`

- [ ] **Step 1: Replace `InGameState` with `HudView`**

Remove:
```rust
#[derive(Clone, Copy, Default, Eq, PartialEq, Debug, Hash, SubStates)]
#[source(GameState = GameState::Game)]
pub(crate) enum InGameState {
    #[default]
    Playing,
    Interacting,
}
```

Remove:
```rust
struct InteractionUI;
```

Replace imports:
```rust
// Old
use crate::game::hud::{self, FooterText};

// New
use crate::game::hud::{self, FooterText, HudView, OverlayImage};
```

- [ ] **Step 2: Update `InteractionPlugin::build`**

Replace the plugin build:

```rust
impl Plugin for InteractionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (hover_hint_system, interact_system)
                .chain()
                .run_if(in_state(GameState::Game))
                .run_if(resource_equals(HudView::World)),
        )
        .add_systems(
            Update,
            interaction_input
                .run_if(in_state(GameState::Game))
                .run_if(resource_equals(HudView::Building)),
        );
    }
}
```

- [ ] **Step 3: Update `interact_system` to use `HudView` + `OverlayImage`**

Change the system signature — replace `mut game_state: ResMut<NextState<InGameState>>` with `mut view: ResMut<HudView>`:

```rust
fn interact_system(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    gamepads: Query<&Gamepad>,
    player_query: Query<&Transform, With<Player>>,
    camera_query: Query<(&GlobalTransform, &Camera), With<PlayerCamera>>,
    buildings: Query<(&BuildingInfo, &GlobalTransform)>,
    map_events: Option<Res<MapEvents>>,
    game_assets: Res<GameAssets>,
    mut images: ResMut<Assets<Image>>,
    mut commands: Commands,
    mut view: ResMut<HudView>,
    cursor_query: Query<&CursorOptions, With<PrimaryWindow>>,
) {
```

In the match block, replace:
```rust
// Old
commands.insert_resource(ActiveInteraction { image });
game_state.set(InGameState::Interacting);

// New
commands.insert_resource(OverlayImage { image });
*view = HudView::Building;
```

- [ ] **Step 4: Update `interaction_input` to use `HudView`**

```rust
fn interaction_input(
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut view: ResMut<HudView>,
    mut commands: Commands,
    mut cursor_query: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if check_exit_input(&keys, &gamepads) {
        commands.remove_resource::<OverlayImage>();
        *view = HudView::World;
        grab_cursor(&mut cursor_query, true);
    }
}
```

- [ ] **Step 5: Remove old UI code**

Delete these functions entirely from `interaction.rs`:
- `show_interaction_image`
- `hide_interaction_image`

Remove the `ActiveInteraction` resource struct. Remove the cursor grab/ungrab logic from `show_interaction_image` — move cursor ungrab into `interact_system` right after setting `HudView::Building`:

```rust
// After *view = HudView::Building;
grab_cursor(&mut cursor_query, false);
```

Add `mut cursor_query: Query<&mut CursorOptions, With<PrimaryWindow>>` to `interact_system`'s parameters.

- [ ] **Step 6: Verify it compiles**

Run: `cargo build 2>&1 | head -50`

- [ ] **Step 7: Commit**

```bash
git add openmm/src/game/interaction.rs
git commit --no-gpg-sign -m "refactor: interaction uses HudView + OverlayImage instead of InGameState"
```

---

### Task 3: Update all `InGameState::Playing` references to `HudView::World`

**Files:**
- Modify: `openmm/src/game/mod.rs`
- Modify: `openmm/src/game/player.rs`
- Modify: `openmm/src/game/entities/mod.rs`
- Modify: `openmm/src/game/odm.rs`
- Modify: `openmm/src/game/physics.rs`

- [ ] **Step 1: Update `game/mod.rs`**

Replace:
```rust
.add_systems(
    Update,
    terrain_material::update_terrain_time.run_if(in_state(interaction::InGameState::Playing)),
)
```

With:
```rust
.add_systems(
    Update,
    terrain_material::update_terrain_time
        .run_if(in_state(GameState::Game))
        .run_if(resource_equals(hud::HudView::World)),
)
```

Remove the `interaction::InGameState` import path from this usage.

- [ ] **Step 2: Update `game/player.rs`**

Replace:
```rust
.run_if(in_state(crate::game::interaction::InGameState::Playing)),
```

With:
```rust
.run_if(in_state(GameState::Game))
.run_if(resource_equals(crate::game::hud::HudView::World)),
```

- [ ] **Step 3: Update `game/entities/mod.rs`**

Replace:
```rust
use crate::game::interaction::InGameState;
// ...
.run_if(in_state(InGameState::Playing)),
```

With:
```rust
use crate::game::hud::HudView;
// ...
.run_if(in_state(GameState::Game))
.run_if(resource_equals(HudView::World)),
```

- [ ] **Step 4: Update `game/odm.rs`**

Replace:
```rust
.run_if(in_state(crate::game::interaction::InGameState::Playing)),
```

With:
```rust
.run_if(in_state(GameState::Game))
.run_if(resource_equals(crate::game::hud::HudView::World)),
```

- [ ] **Step 5: Update `game/physics.rs`**

Replace:
```rust
gravity_system.run_if(in_state(crate::game::interaction::InGameState::Playing)),
```

With:
```rust
gravity_system
    .run_if(in_state(GameState::Game))
    .run_if(resource_equals(crate::game::hud::HudView::World)),
```

- [ ] **Step 6: Update `game/debug.rs`**

Update the `viewport_rect` import if the path changed. Currently:
```rust
let (l, t, _, _) = crate::game::hud::viewport_rect(w, &cfg, &ui_assets);
```

This should still work since `mod.rs` re-exports `viewport_rect`. Verify.

- [ ] **Step 7: Remove `InGameState` sub-state registration**

In `interaction.rs`, remove:
```rust
app.add_sub_state::<InGameState>()
```

And delete the `InGameState` enum entirely (already done in Task 2 Step 1, but verify no references remain).

- [ ] **Step 8: Verify it compiles and runs**

Run: `cargo build 2>&1 | head -50`

Then run the game briefly to verify:
- HUD renders correctly
- Walking around works
- Interacting with a building shows the overlay image inside the borders (no bleed)
- Pressing Escape/E exits the interaction
- Time freezes during interaction (water animation stops, NPCs freeze)

- [ ] **Step 9: Commit**

```bash
git add openmm/src/game/
git commit --no-gpg-sign -m "refactor: replace InGameState with HudView for all system gating"
```

---

### Task 4: Update CLAUDE.md

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Update the architecture section**

Update the `openmm crate structure` tree to reflect the new `hud/` folder:

Replace the `game/` section with:
```
  game/
    mod.rs             — InGamePlugin, InGame marker component
    world.rs           — WorldPlugin (sky, sun sub-plugins)
    world/sky.rs       — Sky plane with bitmap texture
    world/sun.rs       — Directional light + animated fake sun
    odm.rs             — OdmPlugin (spawns terrain/models, lazy entity spawning)
    player.rs          — PlayerPlugin (Player entity, terrain following, camera, controls)
    collision.rs       — Ground height probing, building colliders
    interaction.rs     — InteractionPlugin (building/chest interactions via HudView)
    dev.rs             — DevPlugin (wireframe, FPS/position HUD, debug map switching)
    utils.rs           — Helpers (random_color)
    hud/
      mod.rs           — HudPlugin, HudView resource, freeze system
      borders.rs       — Border layout, HudDimensions, letterbox, viewport_rect
      minimap.rs       — Minimap, compass strip, tap frames
      footer.rs        — FooterText resource + rendering
      overlay.rs       — OverlayImage, viewport_inner_rect, overlay spawn/despawn
    entities/
      mod.rs           — EntitiesPlugin, shared components
      actor.rs         — Actor component, NPC_SPRITES constant
      sprites.rs       — SpriteCache, SpriteSheet, directional sprite loading
      decoration.rs    — Decoration-related types
    terrain_material/  — Custom terrain shader with water extension
```

- [ ] **Step 2: Add HudView documentation to conventions**

Add after the "Game states" section:

```
### HUD views

- `HudView` resource controls the active view: `World`, `Building`, `Chest`, `Inventory`, `Stats`, `Rest`
- When `HudView` is not `World`: game time freezes (`Time<Virtual>` paused), player input disabled
- Gate gameplay systems with `.run_if(resource_equals(HudView::World))`
- Use `OverlayImage` resource to display a background image in the viewport inner area
- `viewport_inner_rect()` returns the area inside all four HUD borders (for overlay positioning)
- `viewport_rect()` returns the 3D camera viewport area (extends behind border4 on the left)
```

- [ ] **Step 3: Commit**

```bash
git add CLAUDE.md
git commit --no-gpg-sign -m "docs: update CLAUDE.md for hud/ module restructure and HudView"
```
