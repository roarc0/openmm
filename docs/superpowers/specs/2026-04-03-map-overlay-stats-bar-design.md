# Map Overlay & Stats Bar Redesign

**Date:** 2026-04-03

## Overview

Two related HUD improvements:

1. **M key map overlay** — pressing M shows the current outdoor map image fullscreen-ish in the viewport, blocking gameplay while visible.
2. **Stats bar consolidation** — food and gold counts move to a single yellow text line positioned over the border1 sidebar.

---

## Feature 1: M Key Map Overlay

### State

Add `HudView::Map` to the existing enum. The existing `freeze_system` pauses `Time<Virtual>` for any non-`World` view — `Map` gets freeze/unfreeze for free. All systems gated with `.run_if(resource_equals(HudView::World))` are also automatically blocked.

### Image Handle

During `spawn_hud`, after loading the minimap overview image, also insert a `MapOverviewImage(Option<Handle<Image>>)` resource:
- Outdoor maps: `Some(handle)` — same image already loaded for the minimap
- Indoor maps: `None`

This resource persists for the session and is the source of truth for the overlay.

### Input

New system `map_input_system` in the HUD plugin, runs in `GameState::Game`:
- `HudView::World` + M pressed + `MapOverviewImage` is `Some` → set `HudView::Map`
- `HudView::Map` + M pressed → set `HudView::World`
- Indoor maps (`None`): M key silently ignored

### Overlay UI

New `MapOverlayUI` marker component. Three systems:
- `spawn_map_overlay`: triggered when `HudView::Map` and no existing `MapOverlayUI` entity — spawns a centered image node
- `despawn_map_overlay`: triggered when `HudView` is no longer `Map` — despawns all `MapOverlayUI` entities
- `update_map_overlay_layout`: updates layout on window resize

### Layout

The overlay is positioned within `viewport_inner_rect()` with 10% margin on each side:

```
available_w = inner_width * 0.8
available_h = inner_height * 0.8
size = min(available_w, available_h)   // preserve 1:1 aspect ratio
left = inner_left + (inner_width - size) / 2
top  = inner_top  + (inner_height - size) / 2
```

### Files

- `openmm/src/game/hud/mod.rs` — add `HudView::Map`, insert `MapOverviewImage` in `spawn_hud`, register new systems
- `openmm/src/game/hud/map_overlay.rs` — new file: `MapOverlayUI` component, `map_input_system`, spawn/despawn/layout systems

---

## Feature 2: Stats Bar Consolidation

### Current State

Two separate `HudGoldText` / `HudFoodText` image nodes, stacked vertically at bottom-right, rendered with the `smallnum` yellow font. Positioned left of border1.

### New Design

Replace both nodes with a single `HudStatsText` node rendering `"food  gold"` (e.g. `"7  200"`) in yellow `smallnum`. Two spaces between the values give visual separation.

### Position

Over the border1 sidebar, slightly inset from the right edge:
- `right: scale_w(8.0)` — a few pixels from the right edge of the screen (inside border1)
- `top: tap_h + scale_h(10.0)` — just below the tap/minimap frame, near the top of border1

### Update Trigger

Re-render the combined text whenever either `gold` or `food` changes (same `Local<i32>` change-detection pattern as before).

### Files

- `openmm/src/game/hud/stats_bar.rs` — replace `HudGoldText` + `HudFoodText` with single `HudStatsText`; update `spawn_stats_bar` and `update_stats_bar`
- `openmm/src/game/hud/mod.rs` — remove old marker queries, add `HudStatsText` query in `update_hud_layout` if layout needs updating on resize
