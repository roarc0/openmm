# Menu & Loading Screens Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace placeholder menus and loading screens with the original MM6 artwork and button images, loaded from the LOD archives.

**Architecture:** Add a `ui_assets` module to load and cache UI textures from LOD (both custom bitmap format and PCX). The main menu uses `mm6title.pcx` as background with button images (`new1`, `load1`, `quit1`, etc.) as clickable `ImageNode` buttons. The loading screen uses `loading.pcx` as background. All textures loaded via `LodManager::icon()` which handles both PCX and bitmap formats.

**Tech Stack:** Bevy 0.18 UI (Node, Button, ImageNode), `image` crate PCX decoder, existing `LodManager`

---

### Task 1: UI Asset Loader

**Files:**
- Create: `openmm/src/ui_assets.rs`
- Modify: `openmm/src/lib.rs` (add module, register resource)

Loads UI textures from LOD at startup and caches them as Bevy `Handle<Image>`.

- [ ] **Step 1: Create ui_assets module**

```rust
// openmm/src/ui_assets.rs
use bevy::{asset::RenderAssetUsages, prelude::*};
use std::collections::HashMap;
use crate::assets::GameAssets;

/// Cached UI texture handles loaded from LOD.
#[derive(Resource, Default)]
pub struct UiAssets {
    textures: HashMap<String, Handle<Image>>,
}

impl UiAssets {
    /// Load a UI texture by name from the LOD icons archive.
    /// Caches the result — subsequent calls return the cached handle.
    pub fn get_or_load(
        &mut self,
        name: &str,
        game_assets: &GameAssets,
        images: &mut Assets<Image>,
    ) -> Option<Handle<Image>> {
        if let Some(handle) = self.textures.get(name) {
            return Some(handle.clone());
        }
        let img = game_assets.lod_manager().icon(name)?;
        let bevy_img = Image::from_dynamic(img, true, RenderAssetUsages::RENDER_WORLD);
        let handle = images.add(bevy_img);
        self.textures.insert(name.to_string(), handle.clone());
        Some(handle)
    }
}
```

- [ ] **Step 2: Register in lib.rs**

Add `pub mod ui_assets;` to the module list and insert the resource in `GamePlugin::build`:
```rust
app.init_resource::<ui_assets::UiAssets>()
```

- [ ] **Step 3: Build and verify**

Run: `cargo build`

- [ ] **Step 4: Commit**

```
git add openmm/src/ui_assets.rs openmm/src/lib.rs
git commit -m "feat: add UiAssets resource for LOD-based UI texture loading"
```

---

### Task 2: Main Menu with MM6 Title Screen

**Files:**
- Modify: `openmm/src/states/menu.rs` (rewrite `main_menu_setup`)

Replace the placeholder menu with the MM6 title background and original button images.

- [ ] **Step 1: Rewrite main_menu_setup**

The background is `mm6title.pcx` (640×480). Buttons are image-based using `new1` (New Game), `load_up` (Load Game), `quit1` (Quit). Buttons are positioned at bottom-right to match the original layout.

```rust
fn main_menu_setup(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    mut images: ResMut<Assets<Image>>,
    mut ui: ResMut<UiAssets>,
) {
    commands.spawn((Camera2d, OnMainMenuScreen));

    let bg = ui.get_or_load("mm6title.pcx", &game_assets, &mut images);
    let btn_new = ui.get_or_load("new1", &game_assets, &mut images);
    let btn_load = ui.get_or_load("load_up", &game_assets, &mut images);
    let btn_quit = ui.get_or_load("quit1", &game_assets, &mut images);

    // Full-screen background
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        ImageNode::new(bg.unwrap_or_default()),
        OnMainMenuScreen,
    )).with_children(|parent| {
        // Button column at bottom-right
        parent.spawn(Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(40.0),
            right: Val::Px(30.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            ..default()
        }).with_children(|col| {
            // New Game button
            spawn_image_button(col, btn_new, MenuButtonAction::Play);
            // Load Game button
            spawn_image_button(col, btn_load, MenuButtonAction::LoadGame);
            // Quit button
            spawn_image_button(col, btn_quit, MenuButtonAction::Quit);
        });
    });
}

fn spawn_image_button(
    parent: &mut ChildSpawnerCommands,
    texture: Option<Handle<Image>>,
    action: MenuButtonAction,
) {
    parent.spawn((
        Button,
        Node {
            width: Val::Px(220.0),
            height: Val::Px(40.0),
            ..default()
        },
        ImageNode::new(texture.unwrap_or_default()),
        action,
    ));
}
```

- [ ] **Step 2: Update MenuButtonAction enum**

Add `LoadGame` variant. Update `menu_action` to handle it (for now, same as Play — loads the game).

- [ ] **Step 3: Remove old button_system color changes**

The image buttons don't need background color changes. Either remove `button_system` or make it skip image-based buttons.

- [ ] **Step 4: Build and test**

Run: `cargo run --bin openmm -- --skip-intro false`
Expected: MM6 title screen with three image buttons.

- [ ] **Step 5: Commit**

```
git commit -m "feat: main menu with MM6 title screen and original button art"
```

---

### Task 3: Loading Screen with loading.pcx

**Files:**
- Modify: `openmm/src/states/loading.rs` (update `loading_setup`)

Replace the plain "Loading..." text with the `loading.pcx` background image.

- [ ] **Step 1: Update loading_setup to use loading.pcx**

```rust
fn loading_setup(
    mut commands: Commands,
    // ... existing params ...
    game_assets: Res<GameAssets>,
    mut ui: ResMut<UiAssets>,
    mut images_asset: ResMut<Assets<Image>>,
) {
    let bg = ui.get_or_load("loading.pcx", &game_assets, &mut images_asset);

    commands.spawn((Camera2d, InLoading));
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            align_items: AlignItems::FlexEnd,
            justify_content: JustifyContent::Center,
            ..default()
        },
        ImageNode::new(bg.unwrap_or_default()),
        InLoading,
    )).with_children(|parent| {
        parent.spawn((
            Text::new("Loading..."),
            TextFont { font_size: 28.0, ..default() },
            TextColor(Color::WHITE),
            Node { margin: UiRect::all(Val::Px(20.0)), ..default() },
            LoadingText,
        ));
    });
}
```

- [ ] **Step 2: Build and test**

Run: `cargo run --bin openmm`
Expected: Loading screen shows `loading.pcx` background with progress text.

- [ ] **Step 3: Commit**

```
git commit -m "feat: loading screen with loading.pcx background from LOD"
```

---

### Task 4: In-Game Menu (Escape)

**Files:**
- Modify: `openmm/src/states/menu.rs` (add in-game menu variant)

The `options.png` image shows the in-game menu layout with buttons: Resume Game, Controls, New Game, Load Game, Save Game, Quit. Use the individual button images (`resume1`, `control1`, `new1`, `load1`, `save1`, `quit1`).

- [ ] **Step 1: Add in-game menu state**

Add `MenuState::InGame` variant. When the player presses Escape during gameplay, transition to `GameState::Menu` with `MenuState::InGame`.

- [ ] **Step 2: Create in-game menu setup**

Use `options` as background, overlay the 6 button images in a 2-column grid matching the original layout:
- Left column: Resume Game, New Game, Save Game
- Right column: Controls, Load Game, Quit

- [ ] **Step 3: Handle Resume button**

`MenuButtonAction::Resume` transitions back to `GameState::Game` without reloading the map.

- [ ] **Step 4: Build and test**

Run: `cargo run --bin openmm`, press Escape in game.
Expected: In-game menu with MM6 button art. Resume returns to game.

- [ ] **Step 5: Commit**

```
git commit -m "feat: in-game menu with original MM6 button artwork"
```

---

### Task 5: Button Hover Effects

**Files:**
- Modify: `openmm/src/states/menu.rs`

MM6 buttons brighten on hover. Since we don't have separate hover images for all buttons, apply a brightness tint via `ImageNode` color modulation.

- [ ] **Step 1: Update button_system for image buttons**

```rust
fn button_system(
    mut query: Query<(&Interaction, &mut ImageNode), (Changed<Interaction>, With<Button>)>,
) {
    for (interaction, mut image) in &mut query {
        image.color = match interaction {
            Interaction::Pressed => Color::srgb(0.7, 0.7, 0.7), // darken on press
            Interaction::Hovered => Color::srgb(1.2, 1.2, 1.0), // brighten on hover
            Interaction::None => Color::WHITE,                    // normal
        };
    }
}
```

- [ ] **Step 2: Build and test**

- [ ] **Step 3: Commit**

```
git commit -m "feat: button hover/press brightness effects"
```
