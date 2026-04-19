//! Canvas rendering: element spawning, sync, and core data types.

use bevy::picking::Pickable;
use bevy::prelude::*;

use crate::assets::GameAssets;
use crate::editor::InEditor;
use crate::screens::ui_assets::UiAssets;
use crate::screens::{REF_H, REF_W, Screen, ScreenElement, load_texture_with_transparency, resolve_image_size};
use crate::system::config::GameConfig;

/// Resolve element size: uses crop dimensions when present, otherwise
/// delegates to `resolve_image_size` for images, falls back to explicit size for videos.
pub fn resolve_elem_size(elem: &ScreenElement, ui_assets: &UiAssets) -> (f32, f32) {
    if let Some(img) = elem.as_image() {
        // Crop dimensions override — editor shows the clipped viewport size.
        if img.crop_w > 0.0 && img.crop_h > 0.0 {
            return (img.crop_w, img.crop_h);
        }
        resolve_image_size(img, ui_assets)
    } else {
        let (w, h) = elem.size();
        if w > 0.0 && h > 0.0 { (w, h) } else { (32.0, 32.0) }
    }
}

/// Generate a small checkerboard texture for the editor canvas background.
fn generate_checkerboard(
    images: &mut Assets<Image>,
    cell_size: u32,
    color_a: [u8; 4],
    color_b: [u8; 4],
) -> Handle<Image> {
    let size = cell_size * 2;
    let mut data = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let idx = ((y * size + x) * 4) as usize;
            let color = if ((x / cell_size) + (y / cell_size)) % 2 == 0 {
                color_a
            } else {
                color_b
            };
            data[idx..idx + 4].copy_from_slice(&color);
        }
    }
    let mut img = Image::new(
        bevy::render::render_resource::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        data,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    img.sampler = bevy::image::ImageSampler::nearest();
    images.add(img)
}

/// Generate a crosshatch pattern texture for video placeholders.
fn generate_stripes(images: &mut Assets<Image>) -> Handle<Image> {
    let size = 128u32;
    let cell = 32u32;
    let bg: [u8; 4] = [40, 40, 45, 255];
    let line: [u8; 4] = [70, 70, 80, 255];
    let mut data = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let idx = ((y * size + x) * 4) as usize;
            let on_grid = x % cell == 0 || y % cell == 0;
            let color = if on_grid { line } else { bg };
            data[idx..idx + 4].copy_from_slice(&color);
        }
    }
    let mut img = Image::new(
        bevy::render::render_resource::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        data,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    img.sampler = bevy::image::ImageSampler::nearest();
    images.add(img)
}

/// Runtime state of the screen being edited.
#[derive(Resource)]
pub struct EditorScreen {
    pub screen: Screen,
    pub dirty: bool,
    /// ID at load time — used to detect renames and delete the old .ron file on save.
    pub original_id: Option<String>,
}

/// Marker component on each spawned element node.
#[derive(Component)]
pub struct CanvasElement {
    pub index: usize,
}

/// Marker component for the background image node.
#[derive(Component)]
pub struct CanvasBackground;

/// Per-element editor-only state (not saved to RON).
#[derive(Resource, Default)]
pub struct ElementEditorState {
    /// Hidden element indices (texture hidden, gizmo remains).
    pub hidden: std::collections::HashSet<usize>,
}

/// Current selection state.
#[derive(Resource, Default)]
pub struct Selection {
    pub index: Option<usize>,
    pub drag_offset: Option<Vec2>,
    /// Whether the event editor window is open for the selected element.
    pub edt_open: bool,
    /// Whether the variant editor window is open for the selected element.
    pub var_open: bool,
    /// Which state to preview on canvas (None = default).
    pub preview_state: Option<String>,
    /// Last click-sound ID previewed from editor autocomplete hover.
    pub click_sound_preview_id: Option<u32>,
}

// ─── Rebuild canvas ─────────────────────────────────────────────────────────

/// Rebuild canvas entities when structure changes (element count, background).
pub fn rebuild_canvas(
    mut commands: Commands,
    editor: Res<EditorScreen>,
    mut ui_assets: ResMut<UiAssets>,
    game_assets: Res<GameAssets>,
    mut images: ResMut<Assets<Image>>,
    cfg: Res<GameConfig>,
    old_bg: Query<Entity, With<CanvasBackground>>,
    old_elems: Query<Entity, With<CanvasElement>>,
    mut last_fingerprint: Local<u64>,
) {
    // Fingerprint: element count + background + all transparency settings.
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    editor.screen.elements.len().hash(&mut hasher);
    for elem in &editor.screen.elements {
        match elem {
            ScreenElement::Image(img) => {
                img.transparent_color.hash(&mut hasher);
                img.texture_for_state("default").hash(&mut hasher);
            }
            ScreenElement::Video(vid) => {
                vid.video.hash(&mut hasher);
            }
            ScreenElement::Text(txt) => {
                txt.source.hash(&mut hasher);
                txt.font.hash(&mut hasher);
            }
        }
    }
    let fp = hasher.finish();
    let has_existing_canvas = old_bg.iter().next().is_some() || old_elems.iter().next().is_some();
    if *last_fingerprint == fp && has_existing_canvas {
        return;
    }
    *last_fingerprint = fp;

    for e in old_bg.iter().chain(old_elems.iter()) {
        commands.entity(e).despawn();
    }

    // Checkerboard background (always present behind everything).
    let checker_handle = generate_checkerboard(&mut images, 16, [40, 40, 40, 255], [50, 50, 50, 255]);
    let mut checker_img = ImageNode::new(checker_handle);
    checker_img.image_mode = bevy::ui::widget::NodeImageMode::Tiled {
        tile_x: true,
        tile_y: true,
        stretch_value: 1.0,
    };
    commands.spawn((
        Name::new("canvas_checker"),
        checker_img,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(0.0),
            top: Val::Percent(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        Pickable::IGNORE,
        ZIndex(-2),
        CanvasBackground,
        InEditor,
    ));

    // Elements.
    for (i, elem) in editor.screen.elements.iter().enumerate() {
        spawn_element(&mut commands, &mut ui_assets, &game_assets, &mut images, &cfg, elem, i);
    }
}

fn spawn_element(
    commands: &mut Commands,
    ui_assets: &mut UiAssets,
    game_assets: &GameAssets,
    images: &mut Assets<Image>,
    cfg: &GameConfig,
    elem: &ScreenElement,
    index: usize,
) {
    let (w, h) = resolve_elem_size(elem, ui_assets);
    let pos = elem.position();

    let node = Node {
        position_type: PositionType::Absolute,
        left: Val::Percent(pos.0 / REF_W * 100.0),
        top: Val::Percent(pos.1 / REF_H * 100.0),
        width: Val::Percent(w / REF_W * 100.0),
        height: Val::Percent(h / REF_H * 100.0),
        ..default()
    };

    let label = Name::new(format!("canvas_elem_{}", elem.id()));
    let marker = CanvasElement { index };
    let z = ZIndex(elem.z());

    if elem.as_text().is_some() {
        // Text placeholder: semi-transparent dark background.
        commands.spawn((
            label,
            BackgroundColor(Color::srgba(0.1, 0.1, 0.3, 0.6)),
            node,
            z,
            marker,
            Pickable::IGNORE,
            InEditor,
        ));
    } else if elem.as_video().is_some() {
        // Video placeholder: horizontal black/white stripes.
        let stripe_handle = generate_stripes(images);
        let mut stripe_img = ImageNode::new(stripe_handle);
        stripe_img.image_mode = bevy::ui::widget::NodeImageMode::Tiled {
            tile_x: true,
            tile_y: true,
            stretch_value: 1.0,
        };
        commands.spawn((label, stripe_img, node, z, marker, Pickable::IGNORE, InEditor));
    } else if let Some(img) = elem.as_image() {
        let tex_name = img.texture_for_state("default").unwrap_or("").to_string();
        let maybe_handle = if !tex_name.is_empty() {
            load_texture_with_transparency(&tex_name, &img.transparent_color, ui_assets, game_assets, images, cfg)
        } else {
            None
        };
        if let Some(handle) = maybe_handle {
            // Crop mode: clip container + inner image at native texture size.
            if img.crop && img.size.0 > 0.0 && img.size.1 > 0.0 {
                let bare = tex_name
                    .strip_prefix("icons/")
                    .unwrap_or_else(|| tex_name.split('/').next_back().unwrap_or(&tex_name));
                let native = ui_assets.dimensions(bare).or_else(|| ui_assets.dimensions(&tex_name));
                if let Some((tw, th)) = native {
                    let (tw, th) = (tw as f32, th as f32);
                    let clip_node = Node {
                        overflow: Overflow::clip(),
                        ..node.clone()
                    };
                    commands
                        .spawn((label, clip_node, z, marker, Pickable::IGNORE, InEditor))
                        .with_children(|parent| {
                            parent.spawn((
                                ImageNode::new(handle),
                                Node {
                                    width: Val::Percent(tw / w * 100.0),
                                    height: Val::Percent(th / h * 100.0),
                                    ..default()
                                },
                                Pickable::IGNORE,
                                InEditor,
                            ));
                        });
                } else {
                    commands.spawn((
                        label,
                        ImageNode::new(handle),
                        node,
                        z,
                        marker,
                        Pickable::IGNORE,
                        InEditor,
                    ));
                }
            } else {
                commands.spawn((
                    label,
                    ImageNode::new(handle),
                    node,
                    z,
                    marker,
                    Pickable::IGNORE,
                    InEditor,
                ));
            };
        } else if img.bindings.get("source").is_some() {
            // Bound element (minimap, etc.) — texture loaded at runtime, show transparent placeholder.
            commands.spawn((
                label,
                BackgroundColor(Color::srgba(0.2, 0.2, 0.3, 0.4)),
                node,
                z,
                marker,
                Pickable::IGNORE,
                InEditor,
            ));
        } else {
            warn!(
                "editor: failed to load texture '{}' (transparent_color='{}') — showing magenta",
                tex_name, img.transparent_color
            );
            commands.spawn((
                label,
                BackgroundColor(Color::srgba(1.0, 0.0, 1.0, 0.8)),
                node,
                z,
                marker,
                Pickable::IGNORE,
                InEditor,
            ));
        }
    }
}

// ─── Sync positions ────────────────────────────────────────────────────────

/// Syncs Bevy `Node` positions from `EditorScreen` data every frame.
pub fn sync_element_positions(
    editor: Res<EditorScreen>,
    editor_state: Res<ElementEditorState>,
    selection: Res<Selection>,
    mut ui_assets: ResMut<UiAssets>,
    game_assets: Res<GameAssets>,
    mut images: ResMut<Assets<Image>>,
    cfg: Res<GameConfig>,
    mut elem_q: Query<(&CanvasElement, &mut Node, &mut Visibility, Option<&mut ImageNode>)>,
) {
    for (ce, mut node, mut vis, image_node) in &mut elem_q {
        let Some(elem) = editor.screen.elements.get(ce.index) else {
            continue;
        };
        let pos = elem.position();
        let (w, h) = resolve_elem_size(elem, &ui_assets);
        node.left = Val::Percent(pos.0 / REF_W * 100.0);
        node.top = Val::Percent(pos.1 / REF_H * 100.0);
        node.width = Val::Percent(w / REF_W * 100.0);
        node.height = Val::Percent(h / REF_H * 100.0);
        *vis = if editor_state.hidden.contains(&ce.index) {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };

        // Swap texture for preview state (only on selected image elements).
        if selection.index == Some(ce.index) {
            if let Some(ref state_name) = selection.preview_state {
                if let Some(img_elem) = elem.as_image() {
                    if let Some(mut img) = image_node {
                        let tex_name = img_elem
                            .states
                            .get(state_name)
                            .map(|s| s.texture.as_str())
                            .unwrap_or("");
                        if let Some(handle) = load_texture_with_transparency(
                            tex_name,
                            &img_elem.transparent_color,
                            &mut ui_assets,
                            &game_assets,
                            &mut images,
                            &cfg,
                        ) {
                            img.image = handle;
                        }
                    }
                }
            }
        }
    }
}
