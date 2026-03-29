# HUD View System Design

## Problem

The game needs to switch between different "views" inside the HUD viewport area: the 3D world, inventory, stats, rest screen, building interactions, chests, etc. Currently, `interaction.rs` manages its own overlay UI using `viewport_rect()`, which doesn't account for `border4_w` on the left edge — causing the image to bleed past the border corners. There is no shared mechanism for other views to use.

## Goals

1. A `HudView` resource that represents the active view — any system can switch views.
2. When a non-World view is active, game time and 3D rendering freeze.
3. The viewport overlay area is correctly inset within all four border edges.
4. The monolithic `hud.rs` (~1100 lines) is split into focused submodules.
5. `interaction.rs` is refactored to use the new API instead of managing its own UI.

## Design

### HudView Resource

```rust
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
```

Stored as a Bevy resource. Any system switches views by mutating it:

```rust
fn open_chest(mut view: ResMut<HudView>) {
    *view = HudView::Chest;
}
```

### Viewport Inner Rect

A new public function that returns the area inside the border corners (where overlay content should be placed):

```rust
pub fn viewport_inner_rect(window: &Window, cfg: &GameConfig, ui: &UiAssets) -> (f32, f32, f32, f32)
```

Returns `(left, top, width, height)` in logical pixels:
- Left: `bar_x + border4_w`
- Top: `bar_y + border3_h`
- Width: `lw - border1_w - border4_w`
- Height: `lh - border3_h - border2_h - footer_exposed`

The existing `viewport_rect()` stays unchanged — it defines the 3D camera viewport which correctly extends behind `border4`.

### Freeze Mechanism

A system watches for `HudView` changes:
- Entering a non-World view: calls `time.pause()` on `Time<Virtual>` to freeze game simulation. Player movement, NPC AI, and animation systems already use virtual delta time, so they stop naturally.
- Returning to World: calls `time.unpause()`.

The 3D camera keeps its last rendered frame visible but player input systems are gated on `HudView::World`.

### Overlay System

`overlay.rs` provides a generic overlay for views that show a single background image:
- Reads `HudView` and an `OverlayImage` resource (contains `Handle<Image>`)
- When a view with an overlay is active, spawns an `ImageNode` sized to `viewport_inner_rect()`
- When returning to World, despawns the overlay entities
- Updates position on window resize

### Module Organization

Extract the current `game/hud.rs` into:

```
game/hud/
  mod.rs        - HudPlugin, HudView resource, viewport_inner_rect(), freeze/unfreeze system
  borders.rs    - Border spawning, HudDimensions, letterbox_rect, update_hud_layout
  minimap.rs    - Minimap rendering, compass strip, tap frames, direction arrows
  footer.rs     - FooterText resource, spawn + update system
  overlay.rs    - OverlayImage resource, spawn/despawn/resize overlay in viewport inner area
```

Each submodule registers its own systems. `mod.rs` wires them together in `HudPlugin`.

### Interaction Refactor

`interaction.rs` stops managing its own UI:
- On interaction: sets `*view = HudView::Building`, inserts `OverlayImage { image }` resource
- On exit input: sets `*view = HudView::World`, removes `OverlayImage`
- The `InteractionUI` component, `show_interaction_image`, and `hide_interaction_image` systems are removed
- `InGameState` sub-state is replaced by checking `HudView` — `Playing` = `HudView::World`, `Interacting` = any non-World view

### System Gating

Systems that should only run during world gameplay are gated:

```rust
.run_if(resource_equals(HudView::World))
```

This replaces the current `in_state(InGameState::Playing)` pattern.

## Non-Goals

- Stacking multiple views (MM6 shows one view at a time)
- Async transitions or animations between views
- View-specific UI content beyond the overlay image (inventory grid, stat panels, etc. will be separate future work per view)

## Migration Path

1. Create `game/hud/` module structure, move code from `hud.rs`
2. Add `HudView` resource and `viewport_inner_rect()`
3. Add freeze/unfreeze system
4. Add `overlay.rs` with `OverlayImage` support
5. Refactor `interaction.rs` to use new API
6. Remove `InGameState` sub-state from `interaction.rs`
