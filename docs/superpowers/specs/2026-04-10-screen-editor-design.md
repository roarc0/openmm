# Screen Editor Design

Feature-gated visual editor for designing MM6 UI screens as declarative RON files. Lives inside the `openmm` crate under `src/editor/`, compiled only with `--features editor`. No game runtime dependency — the editor loads LOD bitmaps directly via `openmm-data`.

## RON Screen Format

All positions in MM6 reference pixels (640x480). Runtime scales proportionally.

```ron
Screen(
    id: "title",
    background: "title.pcx",
    elements: [
        (
            id: "new_game_btn",
            position: (482, 9),
            size: (135, 45),
            z: 10,
            states: {
                "default": (texture: "mmnew0"),
                "hover": (texture: "mmnew1"),
            },
            on_click: [
                "PlaySound 75",
                "GoToScreen segue",
            ],
        ),
        (
            id: "exit_btn",
            position: (482, 195),
            size: (135, 45),
            z: 10,
            states: {
                "default": (texture: "mmesc0"),
                "hover": (texture: "mmesc1"),
            },
            on_click: [
                "ExitApp",
            ],
        ),
    ],
)
```

### Fields

- `id` — unique string identifier for the screen
- `background` — optional LOD bitmap name, fills the 640x480 canvas
- `elements[]` — ordered list of placed elements

### Element Fields

- `id` — unique string within the screen
- `position` — `(x, y)` in MM6 reference pixels, top-left origin
- `size` — `(w, h)` in reference pixels. If omitted, uses the texture's natural size
- `z` — integer z-order (higher = on top)
- `states` — map of state name → visual. Each visual has a `texture` field (LOD bitmap name). `"default"` is required
- `on_click` — list of action strings, executed sequentially

### Action Strings

Actions use EVT-style syntax. The screen runtime recognizes a small set of screen-specific actions; everything else is forwarded to the EVT event dispatcher.

**Screen-specific:**
- `GoToScreen <screen_id>` — load another .screen.ron
- `ExitApp` — quit the application
- `StartGame` — transition to Loading state

**EVT passthrough (examples):**
- `PlaySound <id>` — play a sound by DSounds ID
- `Set GOLD 500` — set a game variable
- `SpeakNPC 7` — open NPC dialogue

New EVT opcodes work automatically without format changes.

## Module Layout

```
openmm/src/editor/
├── mod.rs          — EditorPlugin, feature gate, GameState::Editor entry
├── format.rs       — Screen/ScreenElement serde structs, RON parse/write
├── canvas.rs       — 640x480 canvas rendering, element spawn/select/drag
├── browser.rs      — egui panel: LOD bitmap search + click-to-place
├── inspector.rs    — egui panel: selected element properties editor
├── io.rs           — load/save .screen.ron to data/screens/
```

## Editor Activation

- CLI: `cargo run --features editor -- --editor`
- Makefile: `make editor`
- On startup with `--editor`, the app enters `GameState::Editor` instead of `GameState::Video`
- The editor shares the Bevy window but has its own camera and UI — no game systems run

## Canvas

- 2D camera looking at a 640x480 reference area
- Background bitmap rendered as a fullscreen sprite (if set)
- Each element rendered as a Bevy sprite at its reference position
- Z-ordering via Bevy transform Z (element.z mapped to small Z increments)
- Grid lines at 16px intervals (toggle-able) for alignment

### Debug Labels

Every element on canvas shows a live label:

```
element_id[w,h]@(x,y)
```

Example: `new_game_btn[135,45]@(482,9)`

Updates in real-time during drag. Rendered as a small text node anchored above each element.

### Selection and Interaction

- Click element to select (highlighted with colored border rect via Bevy gizmos)
- Hover shows a dimmer border outline
- Drag selected element to reposition (updates position in real-time, label follows)
- Scroll wheel on selected element changes z-order
- Delete key removes selected element
- Ctrl+D duplicates selected element

## Browser Panel (Tab key toggle)

- egui window overlaying the canvas
- Text input for searching LOD bitmap names (fuzzy/substring match)
- Thumbnail grid showing matching bitmaps (small previews)
- Click a thumbnail to place it on canvas at center (creates new element with that texture as default state)
- Shows bitmap name and pixel dimensions under each thumbnail

## Inspector Panel

- egui window, visible when an element is selected
- Editable fields: id, position (x,y), size (w,h), z
- States editor: add/remove states, change texture per state
- Actions editor: text area for on_click action strings (one per line)
- Background field (when no element selected, edits screen background)

## File I/O

- Screens saved to `data/screens/{screen_id}.screen.ron`
- Ctrl+S saves current screen
- Ctrl+O opens file picker (or egui dropdown of existing .screen.ron files)
- Ctrl+N creates a new empty screen (prompts for id)

## Feature Gate

```toml
# openmm/Cargo.toml
[features]
editor = ["bevy-inspector-egui"]
```

`bevy-inspector-egui` is already a dependency but can be made editor-only if desired. All editor code behind `#[cfg(feature = "editor")]`.

## What v1 Does NOT Include

- Screen runtime/executor (comes later when replacing menu.rs)
- Text elements or $variable substitution
- Screen-to-screen preview navigation
- Undo/redo
- Transparency toggle per texture (add when needed)
- Animation frames

## Future: Screen Runtime

When ready to replace `menu.rs`, a `ScreenRuntime` plugin will:

1. Load .screen.ron, resolve textures from LOD via UiAssets
2. Spawn Bevy entities with scaled positions (640x480 → window size)
3. Handle hover state switching and click detection
4. Parse and execute action strings (screen-specific or EVT passthrough)
5. Despawn on screen transition

This is a separate task — the editor and format come first.
