# Screen Runtime Design

Standalone plugin that loads and runs a `.ron` screen file. Activated via `--screen=title` CLI flag. Independent from game systems — does not replace menu.rs.

## Activation

`--screen=title` → loads `openmm/assets/title.ron`, enters `GameState::Screen`, skips all game plugins (same isolation as `--editor`).

## Runtime behavior

1. Load RON, spawn Camera2d + Bevy UI nodes at 640x480 reference resolution
2. Apply color-key transparency per element
3. Each frame: detect cursor hover, swap textures for elements with "hover" state
4. On click: parse and execute action strings sequentially

## Action dispatch

Screen-specific actions (handled directly):
- `GoToScreen <name>` — despawn current, load another .ron
- `StartGame` — transition to GameState::Loading  
- `ExitApp` — quit

EVT passthrough (future):
- `PlaySound <id>` — log for now, wire to sound system later
- Everything else — log as unhandled

## Hover

Track which element index the cursor is over (same hit-test as editor's selection_system). If the hovered element has a state where `condition == "hover"`, swap to that texture. Restore default texture when cursor leaves.

## What v1 does NOT include

- Bindings (texture/text/scroll/visible from game variables) — needs game state resources
- EVT event dispatch — needs EventQueue which lives in game plugins
- Sound — needs SoundManager
- State conditions other than "hover"

## Files

- Create: `openmm/src/screen_runtime.rs` — plugin, spawn, hover, click, action dispatch
- Modify: `openmm/src/config.rs` — add `--screen` CLI arg
- Modify: `openmm/src/lib.rs` — add GameState::Screen, register plugin, route initial state
- Reuse: `openmm/src/editor/format.rs` — Screen/ScreenElement/ElementState structs (already pub)
- Reuse: `openmm/src/editor/canvas.rs` — load_texture_with_transparency (make pub)
