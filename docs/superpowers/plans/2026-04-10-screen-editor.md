# Screen Editor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Feature-gated visual editor for designing MM6 UI screens as declarative RON files, activated via `--editor` CLI flag.

**Architecture:** A Bevy plugin under `openmm/src/editor/` behind `#[cfg(feature = "editor")]`. Uses egui (via `bevy-inspector-egui`) for panels (bitmap browser, inspector). Canvas renders elements as Bevy UI nodes at 640x480 reference resolution. RON format describes screens with positioned elements, visual states, and EVT-style action strings.

**Tech Stack:** Rust 2024, Bevy 0.18, bevy-inspector-egui (already in deps), RON serde, openmm-data LOD access.

---

## File Structure

| Action | Path | Purpose |
|--------|------|---------|
| Create | `openmm/src/editor/mod.rs` | EditorPlugin, `--editor` CLI flag, GameState::Editor, editor camera setup |
| Create | `openmm/src/editor/format.rs` | RON serde structs: Screen, ScreenElement, ElementState |
| Create | `openmm/src/editor/canvas.rs` | Canvas rendering, element spawning, selection, drag-move, z-order, debug labels |
| Create | `openmm/src/editor/browser.rs` | egui bitmap browser panel: search LOD icons, click-to-place |
| Create | `openmm/src/editor/inspector.rs` | egui inspector panel: edit selected element properties |
| Create | `openmm/src/editor/io.rs` | Load/save .screen.ron files from data/screens/ |
| Modify | `openmm/src/lib.rs` | Add `editor` module, register EditorPlugin when feature enabled, add GameState::Editor |
| Modify | `openmm/src/config.rs` | Add `--editor` CLI flag |
| Modify | `openmm/Cargo.toml` | Add `editor` feature, add `ron` dependency |
| Modify | `Makefile` | Add `make editor` target |
| Create | `data/screens/` | Directory for .screen.ron files (initially empty) |

---

### Task 1: Feature flag, CLI flag, and GameState::Editor

**Files:**
- Modify: `openmm/Cargo.toml`
- Modify: `openmm/src/config.rs`
- Modify: `openmm/src/lib.rs`
- Create: `openmm/src/editor/mod.rs`
- Modify: `Makefile`

- [ ] **Step 1: Add `ron` dep and `editor` feature to Cargo.toml**

In `openmm/Cargo.toml`, add `ron` to dependencies and add the `editor` feature:

```toml
# Under [features]
editor = []

# Under [dependencies]
ron = "0.10"
```

Note: `bevy-inspector-egui` is already an unconditional dependency (used for world_inspector), so the editor feature just gates our code, not the egui dep.

- [ ] **Step 2: Add `--editor` CLI flag to config.rs**

In `openmm/src/config.rs`, add to `Cli` struct:

```rust
/// Launch the screen editor instead of the game
#[arg(long)]
editor: bool,
```

Add to `ConfigFile`:

```rust
editor: Option<bool>,
```

Add to `GameConfig`:

```rust
/// Launch screen editor mode instead of the game
pub editor: bool,
```

In `GameConfig::default()`:

```rust
editor: false,
```

In the `resolve` block inside `GameConfig::load()`:

```rust
editor: cli.editor || file_cfg.editor.unwrap_or(d.editor),
```

- [ ] **Step 3: Add GameState::Editor variant to lib.rs**

In `openmm/src/lib.rs`, add the `Editor` variant to `GameState`:

```rust
#[derive(Clone, Copy, Default, Eq, PartialEq, Debug, Hash, States)]
pub(crate) enum GameState {
    #[default]
    Video,
    Menu,
    Loading,
    Game,
    #[cfg(feature = "editor")]
    Editor,
}
```

Add the editor module (conditional):

```rust
#[cfg(feature = "editor")]
pub(crate) mod editor;
```

In `GamePlugin::build`, update `initial_state` logic to check editor flag first:

```rust
let initial_state = if cfg.editor {
    #[cfg(feature = "editor")]
    { GameState::Editor }
    #[cfg(not(feature = "editor"))]
    {
        warn!("--editor requires the 'editor' feature; starting normally");
        GameState::Menu
    }
} else if cfg.map.is_some() {
    GameState::Loading
} else if cfg.skip_intro {
    GameState::Menu
} else {
    GameState::Video
};
```

Register the editor plugin conditionally:

```rust
#[cfg(feature = "editor")]
if cfg.editor {
    app.add_plugins(editor::EditorPlugin);
}
```

- [ ] **Step 4: Create stub editor/mod.rs**

Create `openmm/src/editor/mod.rs`:

```rust
mod format;
mod canvas;
mod browser;
mod inspector;
mod io;

use bevy::prelude::*;
use crate::GameState;

/// Marker for all editor entities — despawned on editor exit.
#[derive(Component)]
struct InEditor;

pub struct EditorPlugin;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Editor), editor_setup);
    }
}

fn editor_setup(mut commands: Commands) {
    // 2D camera for editor canvas
    commands.spawn((
        Name::new("editor_camera"),
        Camera2d,
        InEditor,
    ));
    info!("Screen editor started");
}
```

- [ ] **Step 5: Add `make editor` to Makefile**

Add after the `run` target:

```makefile
editor:
	cargo run -p openmm --features "openmm/dev,openmm/editor" -- --editor --skip-intro true
```

- [ ] **Step 6: Create empty stub files so the module compiles**

Create these with minimal content so `mod.rs` compiles:

`openmm/src/editor/format.rs`:
```rust
//! RON screen format: serde structs for Screen, ScreenElement, ElementState.
```

`openmm/src/editor/canvas.rs`:
```rust
//! Canvas rendering: element spawning, selection, drag-move, z-order, debug labels.
```

`openmm/src/editor/browser.rs`:
```rust
//! egui bitmap browser panel: search LOD icons, click-to-place.
```

`openmm/src/editor/inspector.rs`:
```rust
//! egui inspector panel: edit selected element properties.
```

`openmm/src/editor/io.rs`:
```rust
//! Load/save .screen.ron files.
```

- [ ] **Step 7: Verify it compiles and launches**

Run: `make editor`

Expected: window opens with a blank screen, log says "Screen editor started". No game systems run.

- [ ] **Step 8: Run lint**

Run: `make lint`

Expected: passes cleanly.

- [ ] **Step 9: Commit**

```bash
git add openmm/Cargo.toml openmm/src/config.rs openmm/src/lib.rs openmm/src/editor/ Makefile
git commit --no-gpg-sign -m "editor: scaffold feature-gated screen editor with --editor CLI flag"
```

---

### Task 2: RON format structs

**Files:**
- Modify: `openmm/src/editor/format.rs`

- [ ] **Step 1: Write format test**

Add to `openmm/src/editor/format.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A UI screen definition — the root of a .screen.ron file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Screen {
    pub id: String,
    #[serde(default)]
    pub background: Option<String>,
    #[serde(default)]
    pub elements: Vec<ScreenElement>,
}

/// A single positioned element on a screen.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenElement {
    pub id: String,
    pub position: (f32, f32),
    #[serde(default)]
    pub size: Option<(f32, f32)>,
    #[serde(default)]
    pub z: i32,
    #[serde(default)]
    pub states: BTreeMap<String, ElementState>,
    #[serde(default)]
    pub on_click: Vec<String>,
}

/// Visual state of an element — currently just a texture name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementState {
    pub texture: String,
}

impl Screen {
    /// Create a new empty screen with the given id.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            background: None,
            elements: Vec::new(),
        }
    }
}

impl ScreenElement {
    /// Create a new element with a default texture state.
    pub fn new(id: impl Into<String>, texture: impl Into<String>, position: (f32, f32)) -> Self {
        let mut states = BTreeMap::new();
        states.insert(
            "default".to_string(),
            ElementState {
                texture: texture.into(),
            },
        );
        Self {
            id: id.into(),
            position,
            size: None,
            z: 0,
            states,
            on_click: Vec::new(),
        }
    }

    /// Get the texture name for the current or default state.
    pub fn texture_for_state(&self, state: &str) -> Option<&str> {
        self.states
            .get(state)
            .or_else(|| self.states.get("default"))
            .map(|s| s.texture.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_screen_ron() {
        let screen = Screen {
            id: "title".to_string(),
            background: Some("title.pcx".to_string()),
            elements: vec![
                ScreenElement {
                    id: "new_game_btn".to_string(),
                    position: (482.0, 9.0),
                    size: Some((135.0, 45.0)),
                    z: 10,
                    states: BTreeMap::from([
                        ("default".to_string(), ElementState { texture: "mmnew0".to_string() }),
                        ("hover".to_string(), ElementState { texture: "mmnew1".to_string() }),
                    ]),
                    on_click: vec![
                        "PlaySound 75".to_string(),
                        "GoToScreen segue".to_string(),
                    ],
                },
            ],
        };

        let ron_str = ron::ser::to_string_pretty(&screen, ron::ser::PrettyConfig::default()).unwrap();
        let parsed: Screen = ron::from_str(&ron_str).unwrap();

        assert_eq!(parsed.id, "title");
        assert_eq!(parsed.background.as_deref(), Some("title.pcx"));
        assert_eq!(parsed.elements.len(), 1);

        let btn = &parsed.elements[0];
        assert_eq!(btn.id, "new_game_btn");
        assert_eq!(btn.position, (482.0, 9.0));
        assert_eq!(btn.size, Some((135.0, 45.0)));
        assert_eq!(btn.z, 10);
        assert_eq!(btn.states.len(), 2);
        assert_eq!(btn.texture_for_state("hover"), Some("mmnew1"));
        assert_eq!(btn.texture_for_state("missing"), Some("mmnew0")); // falls back to default
        assert_eq!(btn.on_click.len(), 2);
        assert_eq!(btn.on_click[0], "PlaySound 75");
    }

    #[test]
    fn deserialize_minimal_screen() {
        let ron_str = r#"Screen(id: "empty", elements: [])"#;
        let screen: Screen = ron::from_str(ron_str).unwrap();
        assert_eq!(screen.id, "empty");
        assert!(screen.background.is_none());
        assert!(screen.elements.is_empty());
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p openmm --features editor -- format`

Expected: both tests pass.

- [ ] **Step 3: Commit**

```bash
git add openmm/src/editor/format.rs
git commit --no-gpg-sign -m "editor: RON screen format structs with round-trip test"
```

---

### Task 3: File I/O (load/save .screen.ron)

**Files:**
- Modify: `openmm/src/editor/io.rs`

- [ ] **Step 1: Implement load/save functions**

```rust
//! Load/save .screen.ron files from data/screens/.

use std::fs;
use std::path::{Path, PathBuf};

use super::format::Screen;

const SCREENS_DIR: &str = "data/screens";

/// Ensure the screens directory exists.
fn ensure_dir() {
    let _ = fs::create_dir_all(SCREENS_DIR);
}

/// Path to a screen file by id.
pub fn screen_path(id: &str) -> PathBuf {
    Path::new(SCREENS_DIR).join(format!("{}.screen.ron", id))
}

/// Save a screen to its .screen.ron file.
pub fn save_screen(screen: &Screen) -> Result<(), String> {
    ensure_dir();
    let path = screen_path(&screen.id);
    let ron_str = ron::ser::to_string_pretty(screen, ron::ser::PrettyConfig::default())
        .map_err(|e| format!("RON serialize error: {e}"))?;
    fs::write(&path, &ron_str).map_err(|e| format!("Write error {}: {e}", path.display()))?;
    log::info!("saved screen to {}", path.display());
    Ok(())
}

/// Load a screen from a .screen.ron file.
pub fn load_screen(id: &str) -> Result<Screen, String> {
    let path = screen_path(id);
    let contents = fs::read_to_string(&path).map_err(|e| format!("Read error {}: {e}", path.display()))?;
    ron::from_str(&contents).map_err(|e| format!("RON parse error {}: {e}", path.display()))
}

/// List all available screen IDs (filenames without .screen.ron suffix).
pub fn list_screens() -> Vec<String> {
    let dir = Path::new(SCREENS_DIR);
    if !dir.exists() {
        return Vec::new();
    }
    let mut names: Vec<String> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.strip_suffix(".screen.ron").map(str::to_string)
        })
        .collect();
    names.sort();
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::format::Screen;

    #[test]
    fn save_and_load_round_trip() {
        let screen = Screen::new("test_io_roundtrip");
        save_screen(&screen).unwrap();
        let loaded = load_screen("test_io_roundtrip").unwrap();
        assert_eq!(loaded.id, "test_io_roundtrip");
        // Cleanup
        let _ = std::fs::remove_file(screen_path("test_io_roundtrip"));
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p openmm --features editor -- io`

Expected: passes.

- [ ] **Step 3: Commit**

```bash
git add openmm/src/editor/io.rs
git commit --no-gpg-sign -m "editor: screen file I/O — save, load, list .screen.ron"
```

---

### Task 4: Canvas rendering — spawn elements as Bevy UI nodes

**Files:**
- Modify: `openmm/src/editor/canvas.rs`
- Modify: `openmm/src/editor/mod.rs`

- [ ] **Step 1: Implement canvas module**

Write `openmm/src/editor/canvas.rs`:

```rust
//! Canvas rendering: element spawning, selection, drag-move, z-order, debug labels.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use super::format::{Screen, ScreenElement};
use super::InEditor;
use crate::assets::GameAssets;
use crate::game::hud::UiAssets;
use crate::config::GameConfig;

/// Reference resolution for MM6 UI design.
pub const REF_W: f32 = 640.0;
pub const REF_H: f32 = 480.0;

/// Currently loaded screen being edited.
#[derive(Resource)]
pub struct EditorScreen {
    pub screen: Screen,
    /// True when screen has unsaved changes.
    pub dirty: bool,
}

/// Marker for a canvas element entity.
#[derive(Component)]
pub struct CanvasElement {
    /// Index into EditorScreen.screen.elements
    pub index: usize,
}

/// Marker for the debug label text of a canvas element.
#[derive(Component)]
pub struct ElementLabel {
    pub index: usize,
}

/// Currently selected element index (None = no selection).
#[derive(Resource, Default)]
pub struct Selection {
    pub index: Option<usize>,
    /// Drag state: mouse offset from element origin when drag started.
    pub drag_offset: Option<Vec2>,
}

/// Marker for the background image entity.
#[derive(Component)]
struct CanvasBackground;

/// Rebuild all canvas entities from the current EditorScreen.
/// Called on enter and when elements are added/removed.
pub fn rebuild_canvas(
    mut commands: Commands,
    editor_screen: Res<EditorScreen>,
    game_assets: Res<GameAssets>,
    mut ui_assets: ResMut<UiAssets>,
    mut images: ResMut<Assets<Image>>,
    cfg: Res<GameConfig>,
    existing: Query<Entity, Or<(With<CanvasElement>, With<CanvasBackground>, With<ElementLabel>)>>,
) {
    // Despawn old canvas entities
    for entity in existing.iter() {
        commands.entity(entity).despawn();
    }

    let screen = &editor_screen.screen;

    // Background
    if let Some(ref bg_name) = screen.background {
        if let Some(handle) = ui_assets.get_or_load(bg_name, &game_assets, &mut images, &cfg) {
            commands.spawn((
                Name::new("editor_bg"),
                ImageNode::new(handle),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                InEditor,
                CanvasBackground,
            ));
        }
    }

    // Elements
    for (i, elem) in screen.elements.iter().enumerate() {
        let texture_name = elem.texture_for_state("default");
        let handle = texture_name.and_then(|name| {
            ui_assets.get_or_load(name, &game_assets, &mut images, &cfg)
        });

        // Get natural size from loaded asset if size not specified
        let (w, h) = elem.size.unwrap_or_else(|| {
            texture_name
                .and_then(|name| ui_assets.dimensions(name))
                .map(|(w, h)| (w as f32, h as f32))
                .unwrap_or((32.0, 32.0))
        });

        // Z-index: base 1.0 + element z * small increment to keep all in front of bg
        let z_index = ZIndex(elem.z);

        let mut entity_commands = commands.spawn((
            Name::new(format!("editor_elem_{}", elem.id)),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(elem.position.0 / REF_W * 100.0),
                top: Val::Percent(elem.position.1 / REF_H * 100.0),
                width: Val::Percent(w / REF_W * 100.0),
                height: Val::Percent(h / REF_H * 100.0),
                ..default()
            },
            z_index,
            CanvasElement { index: i },
            InEditor,
        ));

        if let Some(handle) = handle {
            entity_commands.insert(ImageNode::new(handle));
        } else {
            // Placeholder color for missing textures
            entity_commands.insert(BackgroundColor(Color::srgba(1.0, 0.0, 1.0, 0.5)));
        }
    }
}

/// Update debug labels for all elements. Shows: `id[w,h]@(x,y)`
pub fn update_labels(
    editor_screen: Res<EditorScreen>,
    selection: Res<Selection>,
    elements: Query<(&CanvasElement, &Node, &GlobalTransform, &ComputedNode)>,
    mut gizmos: Gizmos,
) {
    for (canvas_elem, _node, global_tf, computed) in elements.iter() {
        let idx = canvas_elem.index;
        let Some(elem) = editor_screen.screen.elements.get(idx) else { continue };

        let size = computed.size();
        let pos = global_tf.translation().truncate();

        // Draw border rect (gizmos in screen space)
        let is_selected = selection.index == Some(idx);
        let color = if is_selected {
            Color::srgb(1.0, 1.0, 0.0) // yellow for selected
        } else {
            Color::srgba(0.5, 0.5, 0.5, 0.5) // dim grey for unselected
        };

        // Rect corners (top-left origin from pos, size)
        let half = size / 2.0;
        let center = pos + half;
        let center3 = Vec3::new(center.x, center.y, 0.0);
        gizmos.rect_2d(Isometry2d::from_translation(center), size, color);

        // Label text rendered via gizmos is not available — we'll print to console
        // and show in the inspector panel instead. The rect border is the main visual.
        // TODO: Add text rendering when Bevy gizmo text is available.
        let _ = center3; // suppress unused warning
    }
}

/// Handle mouse selection: click on an element to select it.
pub fn selection_system(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    elements: Query<(&CanvasElement, &GlobalTransform, &ComputedNode)>,
    mut selection: ResMut<Selection>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }

    let Ok(window) = windows.single() else { return };
    let Some(cursor_pos) = window.cursor_position() else { return };

    // Find topmost element under cursor
    let mut best: Option<(usize, i32)> = None;
    for (canvas_elem, global_tf, computed) in elements.iter() {
        let pos = global_tf.translation().truncate();
        let size = computed.size();
        let rect = Rect::from_corners(pos, pos + size);
        if rect.contains(cursor_pos) {
            let z = canvas_elem.index as i32; // use index as tiebreaker
            if best.is_none() || z > best.unwrap().1 {
                best = Some((canvas_elem.index, z));
            }
        }
    }

    selection.index = best.map(|(idx, _)| idx);
    selection.drag_offset = None;
}

/// Handle drag: move selected element with mouse.
pub fn drag_system(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut editor_screen: ResMut<EditorScreen>,
    mut selection: ResMut<Selection>,
    elements: Query<(&CanvasElement, &GlobalTransform, &ComputedNode)>,
) {
    let Some(selected_idx) = selection.index else { return };
    let Ok(window) = windows.single() else { return };
    let Some(cursor_pos) = window.cursor_position() else { return };

    let win_w = window.width();
    let win_h = window.height();

    if mouse.just_pressed(MouseButton::Left) {
        // Check if click is on the selected element — start drag
        for (canvas_elem, global_tf, computed) in elements.iter() {
            if canvas_elem.index != selected_idx {
                continue;
            }
            let pos = global_tf.translation().truncate();
            let size = computed.size();
            let rect = Rect::from_corners(pos, pos + size);
            if rect.contains(cursor_pos) {
                selection.drag_offset = Some(cursor_pos - pos);
            }
        }
    }

    if mouse.pressed(MouseButton::Left) {
        if let Some(offset) = selection.drag_offset {
            let new_screen_pos = cursor_pos - offset;
            // Convert screen pixels back to reference coords
            let ref_x = (new_screen_pos.x / win_w * REF_W).clamp(0.0, REF_W);
            let ref_y = (new_screen_pos.y / win_h * REF_H).clamp(0.0, REF_H);

            if let Some(elem) = editor_screen.screen.elements.get_mut(selected_idx) {
                elem.position = (ref_x.round(), ref_y.round());
                editor_screen.dirty = true;
            }
        }
    }

    if mouse.just_released(MouseButton::Left) {
        selection.drag_offset = None;
    }
}

/// Update element node positions from EditorScreen data (after drag or external edit).
pub fn sync_element_positions(
    editor_screen: Res<EditorScreen>,
    mut elements: Query<(&CanvasElement, &mut Node)>,
) {
    for (canvas_elem, mut node) in elements.iter_mut() {
        if let Some(elem) = editor_screen.screen.elements.get(canvas_elem.index) {
            let (w, h) = elem.size.unwrap_or((32.0, 32.0));
            node.left = Val::Percent(elem.position.0 / REF_W * 100.0);
            node.top = Val::Percent(elem.position.1 / REF_H * 100.0);
            node.width = Val::Percent(w / REF_W * 100.0);
            node.height = Val::Percent(h / REF_H * 100.0);
        }
    }
}

/// Scroll wheel changes z-order of selected element.
pub fn z_order_system(
    mut mouse_wheel: EventReader<bevy::input::mouse::MouseWheel>,
    selection: Res<Selection>,
    mut editor_screen: ResMut<EditorScreen>,
) {
    let Some(selected_idx) = selection.index else {
        mouse_wheel.clear();
        return;
    };
    for event in mouse_wheel.read() {
        let delta = if event.y > 0.0 { 1 } else if event.y < 0.0 { -1 } else { 0 };
        if delta != 0 {
            if let Some(elem) = editor_screen.screen.elements.get_mut(selected_idx) {
                elem.z = (elem.z + delta).max(0);
                editor_screen.dirty = true;
            }
        }
    }
}

/// Delete selected element with Delete key.
pub fn delete_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut editor_screen: ResMut<EditorScreen>,
    mut selection: ResMut<Selection>,
) {
    if keys.just_pressed(KeyCode::Delete) || keys.just_pressed(KeyCode::Backspace) {
        if let Some(idx) = selection.index {
            if idx < editor_screen.screen.elements.len() {
                editor_screen.screen.elements.remove(idx);
                selection.index = None;
                editor_screen.dirty = true;
            }
        }
    }
}

/// Ctrl+S saves the current screen.
pub fn save_shortcut_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut editor_screen: ResMut<EditorScreen>,
) {
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if ctrl && keys.just_pressed(KeyCode::KeyS) {
        match super::io::save_screen(&editor_screen.screen) {
            Ok(()) => {
                editor_screen.dirty = false;
                info!("screen '{}' saved", editor_screen.screen.id);
            }
            Err(e) => error!("save failed: {e}"),
        }
    }
}
```

- [ ] **Step 2: Wire canvas systems into EditorPlugin**

Update `openmm/src/editor/mod.rs`:

```rust
mod format;
mod canvas;
mod browser;
mod inspector;
mod io;

use bevy::prelude::*;
use crate::GameState;

pub use format::{Screen, ScreenElement};

/// Marker for all editor entities — despawned on editor exit.
#[derive(Component)]
struct InEditor;

pub struct EditorPlugin;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<canvas::Selection>()
            .add_systems(OnEnter(GameState::Editor), editor_setup)
            .add_systems(
                Update,
                (
                    canvas::rebuild_canvas.run_if(resource_changed::<canvas::EditorScreen>),
                    canvas::selection_system,
                    canvas::drag_system,
                    canvas::sync_element_positions,
                    canvas::update_labels,
                    canvas::z_order_system,
                    canvas::delete_system,
                    canvas::save_shortcut_system,
                )
                    .run_if(in_state(GameState::Editor)),
            );
    }
}

fn editor_setup(mut commands: Commands) {
    // 2D camera for editor canvas
    commands.spawn((
        Name::new("editor_camera"),
        Camera2d,
        InEditor,
    ));

    // Start with an empty screen or load from arg
    let screen = canvas::EditorScreen {
        screen: Screen::new("untitled"),
        dirty: false,
    };
    commands.insert_resource(screen);

    info!("screen editor started — Tab to browse bitmaps, Ctrl+S to save");
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build -p openmm --features "dev,editor"`

Expected: compiles without errors.

- [ ] **Step 4: Commit**

```bash
git add openmm/src/editor/canvas.rs openmm/src/editor/mod.rs
git commit --no-gpg-sign -m "editor: canvas rendering with selection, drag, z-order, delete, save"
```

---

### Task 5: Bitmap browser panel (egui)

**Files:**
- Modify: `openmm/src/editor/browser.rs`
- Modify: `openmm/src/editor/mod.rs`

- [ ] **Step 1: Implement bitmap browser**

Write `openmm/src/editor/browser.rs`:

```rust
//! egui bitmap browser panel: search LOD icons, click-to-place.

use bevy::prelude::*;
use bevy_inspector_egui::bevy_egui::EguiContexts;

use super::canvas::EditorScreen;
use super::format::{ScreenElement, ElementState};
use crate::assets::GameAssets;
use crate::game::hud::UiAssets;
use crate::config::GameConfig;

/// Browser panel state.
#[derive(Resource)]
pub struct BrowserState {
    /// Whether the browser panel is visible.
    pub open: bool,
    /// Current search text.
    pub search: String,
    /// Cached list of all icon names from LOD.
    pub all_icons: Vec<String>,
    /// Filtered names matching current search.
    pub filtered: Vec<String>,
}

impl Default for BrowserState {
    fn default() -> Self {
        Self {
            open: false,
            search: String::new(),
            all_icons: Vec::new(),
            filtered: Vec::new(),
        }
    }
}

/// Load the icon name list once on editor start.
pub fn init_browser(
    game_assets: Res<GameAssets>,
    mut browser: ResMut<BrowserState>,
) {
    if !browser.all_icons.is_empty() {
        return;
    }
    let mut icons = game_assets.assets()
        .files_in("icons")
        .unwrap_or_default();
    icons.sort();
    browser.filtered = icons.clone();
    browser.all_icons = icons;
    info!("browser: loaded {} icon names", browser.all_icons.len());
}

/// Toggle browser with Tab key.
pub fn toggle_browser(
    keys: Res<ButtonInput<KeyCode>>,
    mut browser: ResMut<BrowserState>,
) {
    if keys.just_pressed(KeyCode::Tab) {
        browser.open = !browser.open;
    }
}

/// Draw the browser egui window.
pub fn browser_ui(
    mut contexts: EguiContexts,
    mut browser: ResMut<BrowserState>,
    mut editor_screen: ResMut<EditorScreen>,
    game_assets: Res<GameAssets>,
    mut ui_assets: ResMut<UiAssets>,
    mut images: ResMut<Assets<Image>>,
    cfg: Res<GameConfig>,
) {
    if !browser.open {
        return;
    }

    let ctx = contexts.ctx_mut();

    bevy_inspector_egui::bevy_egui::egui::Window::new("Bitmap Browser")
        .resizable(true)
        .default_width(300.0)
        .default_height(500.0)
        .show(ctx, |ui| {
            // Search box
            let changed = ui
                .horizontal(|ui| {
                    ui.label("Search:");
                    ui.text_edit_singleline(&mut browser.search).changed()
                })
                .inner;

            if changed {
                let query = browser.search.to_lowercase();
                browser.filtered = if query.is_empty() {
                    browser.all_icons.clone()
                } else {
                    browser.all_icons
                        .iter()
                        .filter(|name| name.to_lowercase().contains(&query))
                        .cloned()
                        .collect()
                };
            }

            ui.separator();
            ui.label(format!("{} results", browser.filtered.len()));

            // Scrollable list of icon names — click to place
            bevy_inspector_egui::bevy_egui::egui::ScrollArea::vertical()
                .max_height(400.0)
                .show(ui, |ui| {
                    // Show a reasonable subset to avoid egui lag with thousands of items
                    let limit = 200;
                    for name in browser.filtered.iter().take(limit) {
                        // Show name and dimensions if loaded
                        let dims = ui_assets.dimensions(name)
                            .map(|(w, h)| format!(" [{}x{}]", w, h))
                            .unwrap_or_default();

                        if ui.button(format!("{}{}", name, dims)).clicked() {
                            // Try to load so we know dimensions
                            let _ = ui_assets.get_or_load(name, &game_assets, &mut images, &cfg);
                            let size = ui_assets.dimensions(name).map(|(w, h)| (w as f32, h as f32));

                            // Create new element at center of canvas
                            let elem_id = format!("elem_{}", editor_screen.screen.elements.len());
                            let mut elem = ScreenElement::new(
                                elem_id,
                                name.clone(),
                                (REF_W / 2.0, REF_H / 2.0),
                            );
                            elem.size = size;
                            editor_screen.screen.elements.push(elem);
                            editor_screen.dirty = true;
                            info!("placed '{}' on canvas", name);
                        }
                    }
                    if browser.filtered.len() > limit {
                        ui.label(format!("... and {} more (refine search)", browser.filtered.len() - limit));
                    }
                });
        });
}

use super::canvas::{REF_W, REF_H};
```

- [ ] **Step 2: Register browser systems in EditorPlugin**

Update `openmm/src/editor/mod.rs` — add to `EditorPlugin::build`:

```rust
// In build(), add:
app.init_resource::<browser::BrowserState>();

// In editor_setup, add:
// (init_browser is a system, registered below)

// In the Update system set, add:
browser::init_browser.run_if(resource_added::<browser::BrowserState>),
browser::toggle_browser,
browser::browser_ui,
```

Also, `EguiPlugin` needs to be loaded. Check if `world_inspector` is already loading it. If not, add in `EditorPlugin::build`:

```rust
use bevy_inspector_egui::bevy_egui::EguiPlugin;

// At the top of build():
if !app.is_plugin_added::<EguiPlugin>() {
    app.add_plugins(EguiPlugin::default());
}
```

- [ ] **Step 3: Verify it compiles and launches**

Run: `make editor`

Expected: window opens. Press Tab → egui panel shows with searchable bitmap list. Click an entry → element appears on canvas.

- [ ] **Step 4: Commit**

```bash
git add openmm/src/editor/browser.rs openmm/src/editor/mod.rs
git commit --no-gpg-sign -m "editor: bitmap browser panel — Tab to toggle, search LOD icons, click to place"
```

---

### Task 6: Inspector panel (egui)

**Files:**
- Modify: `openmm/src/editor/inspector.rs`
- Modify: `openmm/src/editor/mod.rs`

- [ ] **Step 1: Implement inspector panel**

Write `openmm/src/editor/inspector.rs`:

```rust
//! egui inspector panel: edit selected element properties.

use bevy::prelude::*;
use bevy_inspector_egui::bevy_egui::EguiContexts;

use super::canvas::{EditorScreen, Selection};
use super::format::ElementState;

/// Draw the inspector panel for the selected element.
pub fn inspector_ui(
    mut contexts: EguiContexts,
    selection: Res<Selection>,
    mut editor_screen: ResMut<EditorScreen>,
) {
    let ctx = contexts.ctx_mut();

    bevy_inspector_egui::bevy_egui::egui::Window::new("Inspector")
        .resizable(true)
        .default_width(280.0)
        .anchor(
            bevy_inspector_egui::bevy_egui::egui::Align2::RIGHT_TOP,
            bevy_inspector_egui::bevy_egui::egui::Vec2::new(-10.0, 10.0),
        )
        .show(ctx, |ui| {
            // Screen-level properties (always visible)
            ui.heading("Screen");
            ui.horizontal(|ui| {
                ui.label("ID:");
                ui.text_edit_singleline(&mut editor_screen.screen.id);
            });

            let mut bg = editor_screen.screen.background.clone().unwrap_or_default();
            ui.horizontal(|ui| {
                ui.label("Background:");
                if ui.text_edit_singleline(&mut bg).changed() {
                    editor_screen.screen.background = if bg.is_empty() { None } else { Some(bg.clone()) };
                    editor_screen.dirty = true;
                }
            });

            ui.separator();

            // Element properties (when selected)
            let Some(idx) = selection.index else {
                ui.label("No element selected");
                ui.label("Click an element on canvas to select");
                return;
            };

            let elem_count = editor_screen.screen.elements.len();
            if idx >= elem_count {
                ui.label("Selection out of range");
                return;
            }

            ui.heading("Element");

            // We need to work with the element mutably
            let elem = &mut editor_screen.screen.elements[idx];

            // ID
            ui.horizontal(|ui| {
                ui.label("ID:");
                if ui.text_edit_singleline(&mut elem.id).changed() {
                    editor_screen.dirty = true;
                }
            });

            // Position
            ui.horizontal(|ui| {
                ui.label("Position:");
                let mut changed = false;
                changed |= ui.add(bevy_inspector_egui::bevy_egui::egui::DragValue::new(&mut elem.position.0).prefix("x: ").speed(1.0)).changed();
                changed |= ui.add(bevy_inspector_egui::bevy_egui::egui::DragValue::new(&mut elem.position.1).prefix("y: ").speed(1.0)).changed();
                if changed {
                    editor_screen.dirty = true;
                }
            });

            // Size
            let mut w = elem.size.map(|s| s.0).unwrap_or(32.0);
            let mut h = elem.size.map(|s| s.1).unwrap_or(32.0);
            ui.horizontal(|ui| {
                ui.label("Size:");
                let mut changed = false;
                changed |= ui.add(bevy_inspector_egui::bevy_egui::egui::DragValue::new(&mut w).prefix("w: ").speed(1.0)).changed();
                changed |= ui.add(bevy_inspector_egui::bevy_egui::egui::DragValue::new(&mut h).prefix("h: ").speed(1.0)).changed();
                if changed {
                    elem.size = Some((w, h));
                    editor_screen.dirty = true;
                }
            });

            // Z order
            ui.horizontal(|ui| {
                ui.label("Z:");
                if ui.add(bevy_inspector_egui::bevy_egui::egui::DragValue::new(&mut elem.z).speed(1.0)).changed() {
                    editor_screen.dirty = true;
                }
            });

            ui.separator();

            // States
            ui.heading("States");
            let state_keys: Vec<String> = elem.states.keys().cloned().collect();
            let mut remove_key = None;
            for key in &state_keys {
                ui.horizontal(|ui| {
                    ui.label(format!("{}:", key));
                    if let Some(state) = elem.states.get_mut(key) {
                        if ui.text_edit_singleline(&mut state.texture).changed() {
                            editor_screen.dirty = true;
                        }
                    }
                    if key != "default" && ui.small_button("x").clicked() {
                        remove_key = Some(key.clone());
                    }
                });
            }
            if let Some(key) = remove_key {
                elem.states.remove(&key);
                editor_screen.dirty = true;
            }

            // Add state button
            ui.horizontal(|ui| {
                if ui.button("+ Add State").clicked() {
                    let name = format!("state_{}", elem.states.len());
                    elem.states.insert(
                        name,
                        ElementState { texture: String::new() },
                    );
                    editor_screen.dirty = true;
                }
            });

            ui.separator();

            // Actions (on_click)
            ui.heading("Actions (on_click)");
            let mut remove_action = None;
            for (i, action) in elem.on_click.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    if ui.text_edit_singleline(action).changed() {
                        editor_screen.dirty = true;
                    }
                    if ui.small_button("x").clicked() {
                        remove_action = Some(i);
                    }
                });
            }
            if let Some(i) = remove_action {
                elem.on_click.remove(i);
                editor_screen.dirty = true;
            }
            if ui.button("+ Add Action").clicked() {
                elem.on_click.push(String::new());
                editor_screen.dirty = true;
            }

            ui.separator();

            // Debug label
            let (x, y) = elem.position;
            let (sw, sh) = elem.size.unwrap_or((0.0, 0.0));
            ui.label(format!("{}[{},{}]@({},{})", elem.id, sw as i32, sh as i32, x as i32, y as i32));
        });
}
```

- [ ] **Step 2: Register inspector in EditorPlugin**

In `openmm/src/editor/mod.rs`, add `inspector::inspector_ui` to the Update system set.

- [ ] **Step 3: Verify it compiles and launches**

Run: `make editor`

Expected: Inspector panel appears on right side. Select an element → properties are editable. Changes reflected on canvas.

- [ ] **Step 4: Commit**

```bash
git add openmm/src/editor/inspector.rs openmm/src/editor/mod.rs
git commit --no-gpg-sign -m "editor: inspector panel — edit element properties, states, actions"
```

---

### Task 7: Screen management (new, open, save status)

**Files:**
- Modify: `openmm/src/editor/mod.rs`
- Modify: `openmm/src/editor/canvas.rs` (minor)

- [ ] **Step 1: Add screen management egui bar**

Add a top menu/toolbar to `openmm/src/editor/mod.rs` (new system):

```rust
/// Top bar with New / Open / Save and current screen name + dirty indicator.
fn editor_toolbar(
    mut contexts: EguiContexts,
    mut editor_screen: ResMut<canvas::EditorScreen>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    let ctx = contexts.ctx_mut();

    bevy_inspector_egui::bevy_egui::egui::TopBottomPanel::top("editor_toolbar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            // Dirty indicator
            let title = if editor_screen.dirty {
                format!("* {} *", editor_screen.screen.id)
            } else {
                editor_screen.screen.id.clone()
            };
            ui.heading(title);

            ui.separator();

            if ui.button("New").clicked() {
                editor_screen.screen = format::Screen::new("untitled");
                editor_screen.dirty = false;
            }

            // Open dropdown with existing screens
            bevy_inspector_egui::bevy_egui::egui::ComboBox::from_label("")
                .selected_text("Open...")
                .show_ui(ui, |ui| {
                    for name in io::list_screens() {
                        if ui.selectable_label(false, &name).clicked() {
                            match io::load_screen(&name) {
                                Ok(screen) => {
                                    editor_screen.screen = screen;
                                    editor_screen.dirty = false;
                                    info!("loaded screen '{}'", name);
                                }
                                Err(e) => error!("load failed: {e}"),
                            }
                        }
                    }
                });

            if ui.button("Save").clicked() || (keys.pressed(KeyCode::ControlLeft) && keys.just_pressed(KeyCode::KeyS)) {
                match io::save_screen(&editor_screen.screen) {
                    Ok(()) => {
                        editor_screen.dirty = false;
                        info!("saved '{}'", editor_screen.screen.id);
                    }
                    Err(e) => error!("save failed: {e}"),
                }
            }

            ui.separator();
            ui.label("Tab: browser | Click: select | Drag: move | Scroll: z-order | Del: remove");
        });
    });
}
```

- [ ] **Step 2: Register toolbar system**

Add `editor_toolbar` to the Update system set in `EditorPlugin::build`.

- [ ] **Step 3: Verify full workflow**

Run: `make editor`

Test workflow:
1. Press Tab → browser opens
2. Search for "title" → click "title.pcx" → background appears? No — it's added as element. We need a way to set background. The inspector already has a background field.
3. Search for "mmnew0" → click → element placed at center
4. Drag element to position
5. Select element → inspector shows properties
6. Edit on_click: add "GoToScreen segue"
7. Ctrl+S → saved to data/screens/untitled.screen.ron
8. Edit screen ID to "title" in inspector → Ctrl+S → saved as title.screen.ron
9. Click New → blank screen
10. Open dropdown → select "title" → loads back

- [ ] **Step 4: Commit**

```bash
git add openmm/src/editor/mod.rs
git commit --no-gpg-sign -m "editor: toolbar — new, open, save screens with dirty indicator"
```

---

### Task 8: Final integration, lint, and cleanup

**Files:**
- All editor files
- Modify: `CLAUDE.md` (doc index)

- [ ] **Step 1: Create data/screens directory**

```bash
mkdir -p data/screens
```

- [ ] **Step 2: Run full lint**

Run: `make lint`

Fix any clippy or fmt issues.

- [ ] **Step 3: Run tests**

Run: `cargo test -p openmm --features editor`

Expected: all format and IO tests pass.

- [ ] **Step 4: Manual smoke test**

Run: `make editor`

Verify:
- Window opens with blank canvas
- Tab toggles browser, search works, click places elements
- Click selects, drag moves, labels update
- Inspector edits work
- Ctrl+S saves .screen.ron
- Open loads saved screen back
- No game systems interfere

- [ ] **Step 5: Verify game still works without editor feature**

Run: `make run`

Expected: game starts normally, no editor code loaded.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit --no-gpg-sign -m "editor: finalize screen editor v1 — browser, inspector, canvas, RON I/O"
```
