use bevy::camera::Viewport;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::GameState;
use crate::assets::{self, GameAssets};
use crate::config::GameConfig;
use crate::game::InGame;
use crate::game::player::{Player, PlayerCamera};
use crate::fonts::GameFonts;
use crate::ui_assets::{UiAssets, make_black_transparent, make_transparent_where};

// MM6 reference height — all HUD dimensions scale relative to this
const REF_H: f32 = 480.0;

// Legacy constants kept for inline scale_h/scale_w calls that don't use a _REF tuple
const FOOTER_H: f32 = 24.0; // footer strip height (reference)

/// Marker for HUD UI entities.
#[derive(Component)]
struct HudUI;

// Border markers for dynamic layout
#[derive(Component)]
struct HudBorder1;
#[derive(Component)]
struct HudBorder2;
#[derive(Component)]
struct HudBorder3;
#[derive(Component)]
struct HudBorder4;
#[derive(Component)]
struct HudBorder5;
#[derive(Component)]
struct HudBorder6;
#[derive(Component)]
struct HudCorner;
#[derive(Component)]
struct HudMapback;
#[derive(Component)]
struct HudMinimapClip;
#[derive(Component)]
struct HudMinimapImage;
#[derive(Component)]
struct HudMinimapArrow;
#[derive(Component)]
struct HudCompassClip;
#[derive(Component)]
struct HudCompassStrip;
#[derive(Component)]
struct HudCamera;
#[derive(Component)]
struct HudRoot;
#[derive(Component)]
struct HudBorder4Left;
#[derive(Component)]
struct HudFooter;
#[derive(Component)]
struct HudFooterText;

/// Cached minimap direction arrow handles (N, NE, E, SE, S, SW, W, NW).
#[derive(Resource)]
struct MinimapArrows(Vec<Handle<Image>>);

/// Cached tap frame handles (tap1=morning, tap2=day, tap3=evening, tap4=night).
#[derive(Resource)]
struct TapFrames(Vec<Handle<Image>>);

/// Text displayed in the footer bar. Write to this resource from any system
/// to update the footer message (e.g. building names, hints, status text).
///
/// Set `text` to change the message. The HUD will re-render automatically.
/// Set `text` to an empty string to clear the footer.
///
/// # Example
/// ```ignore
/// fn my_system(mut footer: ResMut<FooterText>) {
///     footer.set("The Knife Shoppe");
/// }
/// ```
#[derive(Resource)]
pub struct FooterText {
    text: String,
    color: [u8; 4],
    font: String,
    /// Generation counter — bumped on every change so the HUD knows to re-render.
    generation: u64,
}

impl Default for FooterText {
    fn default() -> Self {
        Self {
            text: String::new(),
            color: crate::fonts::WHITE,
            font: "smallnum".into(),
            generation: 0,
        }
    }
}

impl FooterText {
    /// Set footer text with the default font and color.
    pub fn set(&mut self, text: &str) {
        if self.text != text {
            self.text = text.to_string();
            self.generation += 1;
        }
    }

    /// Set footer text with a specific color.
    pub fn set_colored(&mut self, text: &str, color: [u8; 4]) {
        let changed = self.text != text || self.color != color;
        if changed {
            self.text = text.to_string();
            self.color = color;
            self.generation += 1;
        }
    }

    /// Set footer text with a specific font and color.
    pub fn set_styled(&mut self, text: &str, font: &str, color: [u8; 4]) {
        let changed = self.text != text || self.color != color || self.font != font;
        if changed {
            self.text = text.to_string();
            self.font = font.to_string();
            self.color = color;
            self.generation += 1;
        }
    }

    /// Clear the footer text.
    pub fn clear(&mut self) {
        self.set("");
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FooterText>()
            .add_systems(OnEnter(GameState::Game), spawn_hud)
            .add_systems(
                Update,
                (update_hud_layout, update_minimap, update_footer_text, update_viewport)
                    .chain()
                    .run_if(in_state(GameState::Game)),
            );
    }
}

/// Make green or red color-keyed pixels transparent for tap frame overlays.
fn make_tap_key_transparent(img: &mut image::DynamicImage) {
    make_transparent_where(img, |r, g, b| {
        let is_green = g > r && g > b && g >= 128 && r < 100 && b < 100;
        let is_red = r > g && r > b && r >= 128 && g < 100 && b < 100;
        is_green || is_red
    });
}

fn spawn_hud(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    cfg: Res<GameConfig>,
    mut footer_text: ResMut<FooterText>,
    mut ui_assets: ResMut<UiAssets>,
    mut images: ResMut<Assets<Image>>,
) {
    // HUD camera — covers the full window, clears to black (letterbox bar color).
    // Order 1 ensures it renders after the 3D camera (order 0), which overwrites
    // its viewport area with the sky. The HUD then draws on top.
    commands.spawn((
        Name::new("hud_camera"),
        Camera2d,
        Camera {
            order: 1,
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            ..default()
        },
        bevy::ui::IsDefaultUiCamera,
        HudCamera,
        InGame,
    ));

    // Load border assets
    let border1 = ui_assets.get_or_load("border1.pcx", &game_assets, &mut images, &cfg);
    let border2 = ui_assets.get_or_load("border2.pcx", &game_assets, &mut images, &cfg);
    let border3 = ui_assets.get_or_load("border3", &game_assets, &mut images, &cfg);
    let border4 = ui_assets.get_or_load("border4", &game_assets, &mut images, &cfg);
    let border5 = ui_assets.get_or_load("border5", &game_assets, &mut images, &cfg);
    let border6 = ui_assets.get_or_load("border6", &game_assets, &mut images, &cfg);
    let footer = ui_assets.get_or_load("footer", &game_assets, &mut images, &cfg);

    // Load tap frames (minimap border with time-of-day sky)
    // tap1=morning, tap2=day, tap3=evening, tap4=night — green/red key made transparent
    let tap_handles: Vec<Handle<Image>> = (1..=4)
        .filter_map(|i| {
            let name = format!("tap{}", i);
            let key = format!("{}_transparent", name);
            ui_assets.get_or_load_transformed(
                &name,
                &key,
                &game_assets,
                &mut images,
                &cfg,
                make_tap_key_transparent,
            )
        })
        .collect();
    let tap_frame = tap_handles.first().cloned();

    // Load current map overview image for minimap
    // Map overview images are stored as icons (e.g., "oute3" -> 512x512)
    let map_overview = load_map_overview(&game_assets, &mut images, &cfg);

    // Load compass strip
    let compass = ui_assets.get_or_load("compass", &game_assets, &mut images, &cfg);

    // Load minimap direction arrows (mapdir1=N, 2=NE, 3=E, 4=SE, 5=S, 6=SW, 7=W, 8=NW)
    // Black background is made transparent via the transformed cache
    let arrow_handles: Vec<Handle<Image>> = (1..=8)
        .filter_map(|i| {
            let name = format!("mapdir{}", i);
            let key = format!("{}_transparent", name);
            ui_assets.get_or_load_transformed(
                &name,
                &key,
                &game_assets,
                &mut images,
                &cfg,
                make_black_transparent,
            )
        })
        .collect();
    let default_arrow = arrow_handles.first().cloned();

    // Root node — positioned within the letterboxed area by update_hud_layout
    commands
        .spawn((
            Name::new("hud_root"),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            Pickable::IGNORE,
            InGame,
            HudUI,
            HudRoot,
        ))
        .with_children(|parent| {
            // footer — text strip, drawn first so border1/border2 render on top
            if let Some(ref handle) = footer {
                parent.spawn((
                    Name::new("hud_footer"),
                    ImageNode::new(handle.clone()),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        bottom: Val::Px(0.0),
                        width: Val::Px(0.0),
                        height: Val::Px(0.0),
                        ..default()
                    },
                    HudFooter,
                    HudUI,
                ));
            }

            // border2 — bottom bar (469×109), anchored bottom-left (spawned before border1 so border1 is on top)
            if let Some(handle) = border2 {
                parent.spawn((
                    Name::new("hud_border2"),
                    ImageNode::new(handle),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        bottom: Val::Px(0.0),
                        width: Val::Px(0.0),
                        height: Val::Px(0.0),
                        ..default()
                    },
                    HudBorder2,
                    HudUI,
                ));
            }

            // border1 — right sidebar (172×339), anchored bottom-right (on top of border2)
            if let Some(handle) = border1 {
                parent.spawn((
                    Name::new("hud_border1"),
                    ImageNode::new(handle),
                    Node {
                        position_type: PositionType::Absolute,
                        right: Val::Px(0.0),
                        bottom: Val::Px(0.0),
                        width: Val::Px(0.0),
                        height: Val::Px(0.0),
                        ..default()
                    },
                    HudBorder1,
                    HudUI,
                ));
            }

            // border3 — horizontal divider strip (468×8), above bottom bar
            if let Some(handle) = border3 {
                parent.spawn((
                    Name::new("hud_border3"),
                    ImageNode::new(handle),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        bottom: Val::Px(0.0),
                        width: Val::Px(0.0),
                        height: Val::Px(0.0),
                        ..default()
                    },
                    HudBorder3,
                    HudUI,
                ));
            }

            // border4 — vertical divider strip (8×344), left edge of viewport only
            if let Some(handle) = border4 {
                parent.spawn((
                    Name::new("hud_border4"),
                    ImageNode::new(handle),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        top: Val::Px(0.0),
                        width: Val::Px(0.0),
                        height: Val::Px(0.0),
                        ..default()
                    },
                    HudBorder4,
                    HudUI,
                ));
            }

            // border5 — corner piece (8×20)
            if let Some(handle) = border5 {
                parent.spawn((
                    Name::new("hud_border5"),
                    ImageNode::new(handle),
                    Node {
                        position_type: PositionType::Absolute,
                        right: Val::Px(0.0),
                        bottom: Val::Px(0.0),
                        width: Val::Px(0.0),
                        height: Val::Px(0.0),
                        ..default()
                    },
                    HudBorder5,
                    HudUI,
                ));
            }

            // border6 — corner piece (7×21)
            if let Some(handle) = border6 {
                parent.spawn((
                    Name::new("hud_border6"),
                    ImageNode::new(handle),
                    Node {
                        position_type: PositionType::Absolute,
                        right: Val::Px(0.0),
                        bottom: Val::Px(0.0),
                        width: Val::Px(0.0),
                        height: Val::Px(0.0),
                        ..default()
                    },
                    HudBorder6,
                    HudUI,
                ));
            }

            // Bottom-right corner dark fill
            parent.spawn((
                Name::new("hud_corner"),
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(0.0),
                    bottom: Val::Px(0.0),
                    width: Val::Px(0.0),
                    height: Val::Px(0.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.08, 0.06, 0.04)),
                HudCorner,
                HudUI,
            ));

            // Minimap clipped container — sized to mapback area, clips the map image
            parent
                .spawn((
                    Name::new("hud_minimap_clip"),
                    Node {
                        position_type: PositionType::Absolute,
                        right: Val::Px(0.0),
                        top: Val::Px(0.0),
                        width: Val::Px(0.0),
                        height: Val::Px(0.0),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    HudMinimapClip,
                    HudUI,
                ))
                .with_children(|clip| {
                    // Map overview image inside — large, positioned by player pos
                    if let Some(handle) = map_overview {
                        clip.spawn((
                            Name::new("hud_minimap_image"),
                            ImageNode::new(handle),
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(0.0),
                                top: Val::Px(0.0),
                                width: Val::Px(0.0),
                                height: Val::Px(0.0),
                                ..default()
                            },
                            HudMinimapImage,
                            HudUI,
                        ));
                    }
                });

            // Compass strip — behind tap, scrolling direction indicator in tap's compass channel
            if let Some(handle) = compass {
                parent
                    .spawn((
                        Name::new("hud_compass_clip"),
                        Node {
                            position_type: PositionType::Absolute,
                            right: Val::Px(0.0),
                            top: Val::Px(0.0),
                            width: Val::Px(0.0),
                            height: Val::Px(0.0),
                            overflow: Overflow::clip(),
                            ..default()
                        },
                        HudCompassClip,
                        HudUI,
                    ))
                    .with_children(|clip| {
                        clip.spawn((
                            Name::new("hud_compass_strip"),
                            ImageNode::new(handle),
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(0.0),
                                top: Val::Px(0.0),
                                width: Val::Px(0.0),
                                height: Val::Px(0.0),
                                ..default()
                            },
                            HudCompassStrip,
                            HudUI,
                        ));
                    });
            }

            // Tap frame — minimap border with transparent window, on top of compass and map
            if let Some(ref handle) = tap_frame {
                parent.spawn((
                    Name::new("hud_tap_frame"),
                    ImageNode::new(handle.clone()),
                    Node {
                        position_type: PositionType::Absolute,
                        right: Val::Px(0.0),
                        top: Val::Px(0.0),
                        width: Val::Px(0.0),
                        height: Val::Px(0.0),
                        ..default()
                    },
                    HudMapback,
                    HudUI,
                ));
            }

            // Minimap arrow — player direction indicator at center, on top of tap
            if let Some(ref handle) = default_arrow {
                parent.spawn((
                    Name::new("hud_minimap_arrow"),
                    ImageNode::new(handle.clone()),
                    Node {
                        position_type: PositionType::Absolute,
                        right: Val::Px(0.0),
                        top: Val::Px(0.0),
                        width: Val::Px(0.0),
                        height: Val::Px(0.0),
                        ..default()
                    },
                    HudMinimapArrow,
                    HudUI,
                ));
            }

            // Footer text — spawned last so it renders on top of everything
            parent.spawn((
                Name::new("hud_footer_text"),
                ImageNode::new(Handle::default()),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    bottom: Val::Px(0.0),
                    width: Val::Auto,
                    height: Val::Auto,
                    ..default()
                },
                Visibility::Hidden,
                HudFooterText,
                HudUI,
            ));
        });

    // Store arrow handles as resource for runtime direction swapping
    if !arrow_handles.is_empty() {
        commands.insert_resource(MinimapArrows(arrow_handles));
    }
    // Store tap frames for time-of-day switching
    if !tap_handles.is_empty() {
        commands.insert_resource(TapFrames(tap_handles));
    }

    // Set initial footer message
    footer_text.set("Welcome to New Sorpigal");
}

/// Load the map overview image for the minimap.
fn load_map_overview(
    game_assets: &GameAssets,
    images: &mut Assets<Image>,
    cfg: &GameConfig,
) -> Option<Handle<Image>> {
    // Try loading the current map's overview (e.g., "oute3")
    // For now, try common map names
    let map_names = ["oute3", "oute2", "oute1", "outa1"];
    for name in map_names {
        if let Some(img) = game_assets.lod_manager().icon(name) {
            let mut bevy_img = assets::dynamic_to_bevy_image(img);
            bevy_img.sampler = crate::ui_assets::hud_sampler(cfg);
            return Some(images.add(bevy_img));
        }
    }
    None
}

/// Compute the logical letterboxed size (what the HUD camera sees).
fn logical_size(window: &Window, cfg: &GameConfig) -> (f32, f32) {
    let sf = window.scale_factor();
    let (_, _, pw, ph) = letterbox_rect(window, cfg);
    (pw as f32 / sf, ph as f32 / sf)
}

const REF_W: f32 = 640.0;

/// Compute all HUD dimensions from letterboxed logical size and actual asset dimensions.
/// Widths scale by `scale_x` (width / 640), heights by `scale_y` (height / 480).
/// Asset sizes are read dynamically from `UiAssets` — no hardcoded pixel values.
fn hud_dimensions(width: f32, height: f32, ui: &UiAssets) -> HudDimensions {
    let scale_x = width / REF_W;
    let scale_y = height / REF_H;

    // Read true pixel dimensions from loaded assets (fallback to 0 if not loaded yet)
    let dim = |name: &str| -> (f32, f32) {
        ui.dimensions(name)
            .map(|(w, h)| (w as f32, h as f32))
            .unwrap_or((0.0, 0.0))
    };

    let border1 = dim("border1.pcx");
    let border2 = dim("border2.pcx");
    let border3 = dim("border3");
    let border4 = dim("border4");
    let border5 = dim("border5");
    let border6 = dim("border6");
    let tap1 = dim("tap1");
    let footer = dim("footer");
    let compass = dim("compass");

    HudDimensions {
        scale_x,
        scale_y,
        border1_w: border1.0 * scale_x,
        border1_h: (border1.1 - 1.0) * scale_y,
        border2_w: border2.0 * scale_x,
        border2_h: border2.1 * scale_y,
        border3_w: border3.0 * scale_x,
        border3_h: border3.1 * scale_y,
        border4_w: border4.0 * scale_x,
        border4_h: border4.1 * scale_y,
        border5_w: border5.0 * scale_x,
        border5_h: border5.1 * scale_y,
        border6_w: border6.0 * scale_x,
        border6_h: border6.1 * scale_y,
        tap_w: tap1.0 * scale_x,
        tap_h: tap1.1 * scale_y,
        footer_w: footer.0 * scale_x,
        footer_h: footer.1 * scale_y,
        compass_w: compass.0 * scale_x,
        compass_h: compass.1 * scale_y,
        corner_h: height - border1.1 * scale_y - tap1.1 * scale_y,
    }
}

struct HudDimensions {
    scale_x: f32,
    scale_y: f32,
    border1_w: f32,
    border1_h: f32,
    border2_w: f32,
    border2_h: f32,
    border3_w: f32,
    border3_h: f32,
    border4_w: f32,
    border4_h: f32,
    border5_w: f32,
    border5_h: f32,
    border6_w: f32,
    border6_h: f32,
    tap_w: f32,
    tap_h: f32,
    footer_w: f32,
    footer_h: f32,
    compass_w: f32,
    compass_h: f32,
    corner_h: f32,
}

impl HudDimensions {
    /// Scale a reference Y value (height or vertical position).
    fn scale_h(&self, ref_px: f32) -> f32 {
        ref_px * self.scale_y
    }

    /// Scale a reference X value (width or horizontal position).
    fn scale_w(&self, ref_px: f32) -> f32 {
        ref_px * self.scale_x
    }
}

/// Update all HUD element sizes based on window size.
fn update_hud_layout(
    windows: Query<&Window, With<PrimaryWindow>>,
    cfg: Res<GameConfig>,
    ui_assets: Res<UiAssets>,
    mut set: ParamSet<(
        Query<&mut Node, With<HudBorder1>>,
        Query<&mut Node, With<HudBorder2>>,
        Query<&mut Node, With<HudBorder3>>,
        Query<&mut Node, With<HudBorder4>>,
        Query<&mut Node, With<HudBorder5>>,
        Query<&mut Node, With<HudBorder6>>,
        Query<&mut Node, With<HudCorner>>,
        Query<&mut Node, With<HudMapback>>,
    )>,
    mut minimap_clip_q: Query<
        &mut Node,
        (
            With<HudMinimapClip>,
            Without<HudBorder1>,
            Without<HudBorder2>,
            Without<HudBorder3>,
            Without<HudBorder4>,
            Without<HudBorder5>,
            Without<HudBorder6>,
            Without<HudCorner>,
            Without<HudMapback>,
        ),
    >,
    mut arrow_q: Query<
        &mut Node,
        (
            With<HudMinimapArrow>,
            Without<HudMinimapClip>,
            Without<HudCompassClip>,
            Without<HudBorder1>,
            Without<HudBorder2>,
            Without<HudBorder3>,
            Without<HudBorder4>,
            Without<HudBorder5>,
            Without<HudBorder6>,
            Without<HudCorner>,
            Without<HudMapback>,
        ),
    >,
    mut compass_clip_q: Query<
        &mut Node,
        (
            With<HudCompassClip>,
            Without<HudMinimapArrow>,
            Without<HudMinimapClip>,
            Without<HudBorder1>,
            Without<HudBorder2>,
            Without<HudBorder3>,
            Without<HudBorder4>,
            Without<HudBorder5>,
            Without<HudBorder6>,
            Without<HudCorner>,
            Without<HudMapback>,
        ),
    >,
    mut root_q: Query<
        &mut Node,
        (
            With<HudRoot>,
            Without<HudCompassClip>,
            Without<HudMinimapArrow>,
            Without<HudMinimapClip>,
            Without<HudFooter>,
            Without<HudBorder1>,
            Without<HudBorder2>,
            Without<HudBorder3>,
            Without<HudBorder4>,
            Without<HudBorder5>,
            Without<HudBorder6>,
            Without<HudCorner>,
            Without<HudMapback>,
        ),
    >,
    mut footer_q: Query<
        &mut Node,
        (
            With<HudFooter>,
            Without<HudFooterText>,
            Without<HudRoot>,
            Without<HudCompassClip>,
            Without<HudMinimapArrow>,
            Without<HudMinimapClip>,
            Without<HudBorder1>,
            Without<HudBorder2>,
            Without<HudBorder3>,
            Without<HudBorder4>,
            Without<HudBorder5>,
            Without<HudBorder6>,
            Without<HudCorner>,
            Without<HudMapback>,
        ),
    >,
    mut footer_text_q: Query<
        &mut Node,
        (
            With<HudFooterText>,
            Without<HudFooter>,
            Without<HudRoot>,
            Without<HudCompassClip>,
            Without<HudMinimapArrow>,
            Without<HudMinimapClip>,
            Without<HudBorder1>,
            Without<HudBorder2>,
            Without<HudBorder3>,
            Without<HudBorder4>,
            Without<HudBorder5>,
            Without<HudBorder6>,
            Without<HudCorner>,
            Without<HudMapback>,
        ),
    >,
) {
    let Ok(window) = windows.single() else { return };
    let sf = window.scale_factor();
    let (_, _, lpw, lph) = letterbox_rect(&window, &cfg);
    let lw = lpw as f32 / sf;
    let lh = lph as f32 / sf;
    let d = hud_dimensions(lw, lh, &ui_assets);

    // Position HUD root within the letterboxed area
    let bar_x = (window.width() - lw) / 2.0;
    let bar_y = (window.height() - lh) / 2.0;
    for mut node in root_q.iter_mut() {
        node.left = Val::Px(bar_x);
        node.top = Val::Px(bar_y);
        node.width = Val::Px(lw);
        node.height = Val::Px(lh);
    }

    // tap is full sidebar width, so offset is 0
    let tap_offset_x = 0.0_f32;

    // border1 — right sidebar (172×339 ref), true scaled size
    for mut node in set.p0().iter_mut() {
        node.top = Val::Px(d.tap_h);
        node.bottom = Val::Auto;
        node.width = Val::Px(d.border1_w);
        node.height = Val::Px(d.border1_h);
    }
    // border2 — bottom bar (469×109 ref), true scaled size
    for mut node in set.p1().iter_mut() {
        node.width = Val::Px(d.border2_w);
        node.height = Val::Px(d.border2_h);
    }
    // border3 — horizontal strip (468×8 ref) across the top, true scaled size
    for mut node in set.p2().iter_mut() {
        node.left = Val::Px(0.0);
        node.top = Val::Px(0.0);
        node.bottom = Val::Auto;
        node.width = Val::Px(d.border3_w);
        node.height = Val::Px(d.border3_h);
    }
    // border4 — vertical divider strip, left edge of viewport, below border3
    for mut node in set.p3().iter_mut() {
        node.left = Val::Px(0.0);
        node.top = Val::Px(d.border3_h);
        node.right = Val::Auto;
        node.width = Val::Px(d.border4_w);
        node.height = Val::Px(d.border4_h);
    }
    // border5 — top-left corner piece, floating over the viewport
    for mut node in set.p4().iter_mut() {
        node.left = Val::Px(0.0);
        node.top = Val::Px(0.0);
        node.right = Val::Auto;
        node.bottom = Val::Auto;
        node.width = Val::Px(d.border5_w);
        node.height = Val::Px(d.border5_h);
    }
    // border6 — bottom-left corner piece, floating over the viewport
    for mut node in set.p5().iter_mut() {
        node.left = Val::Px(0.0);
        node.bottom = Val::Px(d.border2_h);
        node.right = Val::Auto;
        node.top = Val::Auto;
        node.width = Val::Px(d.border6_w);
        node.height = Val::Px(d.border6_h);
    }
    // Corner fill — bottom-right area (below border1+mapback)
    for mut node in set.p6().iter_mut() {
        node.width = Val::Px(d.border1_w);
        node.height = Val::Px(d.corner_h);
    }
    // Tap frame — at the very top
    for mut node in set.p7().iter_mut() {
        node.right = Val::Px(tap_offset_x);
        node.top = Val::Px(0.0);
        node.width = Val::Px(d.tap_w);
        node.height = Val::Px(d.tap_h);
    }
    // Minimap clip container — same position/size as tap frame
    for mut node in minimap_clip_q.iter_mut() {
        node.right = Val::Px(tap_offset_x);
        node.top = Val::Px(0.0);
        node.width = Val::Px(d.tap_w);
        node.height = Val::Px(d.tap_h);
    }
    // Minimap arrow — centered in tap area
    let arrow_size = d.scale_w(7.0);
    for mut node in arrow_q.iter_mut() {
        node.right = Val::Px(tap_offset_x + (d.tap_w - arrow_size) / 2.0);
        node.top = Val::Px((d.tap_h - arrow_size) / 2.0);
        node.width = Val::Px(arrow_size);
        node.height = Val::Px(arrow_size);
    }
    // Footer (483×24 ref) — sits directly above border2, overlapping its top by 10px.
    let footer_overlap = d.scale_h(10.0);
    let footer_lift = d.scale_h(5.0);
    let footer_bottom = d.border2_h - footer_overlap + footer_lift;
    for mut node in footer_q.iter_mut() {
        node.bottom = Val::Px(footer_bottom);
        node.width = Val::Px(d.footer_w);
        node.height = Val::Px(d.footer_h);
    }
    // Footer text — scaled to footer height, centered vertically
    let text_h = d.footer_h * 0.6;
    let text_bottom = footer_bottom + (d.footer_h - text_h) / 2.0;
    for mut node in footer_text_q.iter_mut() {
        node.bottom = Val::Px(text_bottom);
        node.width = Val::Auto;
        node.height = Val::Px(text_h);
    }
    // Compass clip — floating inside tap frame, in the compass channel
    let compass_clip_w = d.scale_w(38.0);
    let compass_y = d.scale_h(10.0);
    let compass_right = tap_offset_x + d.scale_w(172.0 - 107.0); // right edge at x=107
    for mut node in compass_clip_q.iter_mut() {
        node.right = Val::Px(compass_right);
        node.top = Val::Px(compass_y);
        node.width = Val::Px(compass_clip_w);
        node.height = Val::Px(d.compass_h);
    }
}

/// Update minimap image position, arrow direction, and compass strip based on player transform.
fn update_minimap(
    ui_assets: Res<UiAssets>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cfg: Res<GameConfig>,
    player_q: Query<&Transform, With<Player>>,
    mut minimap_q: Query<&mut Node, (With<HudMinimapImage>, Without<HudCompassStrip>)>,
    arrows_res: Option<Res<MinimapArrows>>,
    mut arrow_q: Query<&mut ImageNode, With<HudMinimapArrow>>,
    mut compass_q: Query<&mut Node, (With<HudCompassStrip>, Without<HudMinimapImage>)>,
) {
    let Ok(window) = windows.single() else { return };
    let Ok(player_tf) = player_q.single() else {
        return;
    };
    let (lw, lh) = logical_size(&window, &cfg);
    let d = hud_dimensions(lw, lh, &ui_assets);

    // Map overview is 512×512 pixels covering the 128×128 tile terrain.
    // Terrain world coords are centered: -half..+half on both axes.
    // Bevy X = right (east), Bevy Z = -MM6_Y (south).
    // Map image: left=west, right=east, top=north, bottom=south.
    use lod::odm::ODM_TILE_SCALE;
    let terrain_size = 128.0 * ODM_TILE_SCALE; // 65536 world units
    let half = terrain_size / 2.0;

    // Player normalized 0..1 on the map (X -> horizontal, Z -> vertical)
    // Bevy Z maps directly to image Y: -half (north) -> 0 (top), +half (south) -> 1 (bottom)
    let nx = (player_tf.translation.x + half) / terrain_size;
    let nz = (player_tf.translation.z + half) / terrain_size;

    // Scale the map image to show reasonable zoom (2x = each pixel covers ~2 tiles)
    let zoom = 3.0;
    let map_img_size = d.tap_w * zoom;

    // Offset so player is at the center of the clip container
    let img_left = d.tap_w / 2.0 - nx * map_img_size;
    let img_top = d.tap_h / 2.0 - nz * map_img_size;

    for mut node in minimap_q.iter_mut() {
        node.left = Val::Px(img_left);
        node.top = Val::Px(img_top);
        node.width = Val::Px(map_img_size);
        node.height = Val::Px(map_img_size);
    }

    let (yaw, _, _) = player_tf.rotation.to_euler(EulerRot::YXZ);
    // Convert yaw to clockwise angle from north (0..TAU)
    let cw_angle = (-yaw).rem_euclid(std::f32::consts::TAU);

    // Update arrow direction based on player yaw
    // mapdir assets are counterclockwise: 1=NE, 2=N, 3=NW, 4=W, 5=SW, 6=S, 7=SE, 8=E
    // sector is clockwise: 0=N, 1=NE, 2=E, 3=SE, 4=S, 5=SW, 6=W, 7=NW
    if let Some(ref arrows) = arrows_res {
        if arrows.0.len() == 8 {
            let sector = ((cw_angle / (std::f32::consts::TAU / 8.0) + 0.5) as usize) % 8;
            // Map clockwise sector to counterclockwise index
            let idx = (9 - sector) % 8;
            for mut arrow_img in arrow_q.iter_mut() {
                arrow_img.image = arrows.0[idx].clone();
            }
        }
    }

    // Update compass strip scroll
    // Strip layout: E(9) · SE(36) · S(68) · SW(98) · W(130) · NW(157) · N(190) · NE(216) · E(249) · SE(276)
    // First E at pixel ~14 (tweaked), second E at ~254 → cycle = 240px = 360°
    // angle_from_east wraps via rem_euclid so pixel_pos stays in [14, 254] — within the 325px strip
    let compass_full_w = d.compass_w;
    let compass_strip_h = d.compass_h;
    let compass_clip_w = d.scale_w(38.0);
    let e_start = d.scale_w(28.0);
    let cycle_w = d.scale_w(240.0);
    let angle_from_east = (cw_angle - std::f32::consts::FRAC_PI_2)
        .rem_euclid(std::f32::consts::TAU);
    let pixel_pos = e_start + (angle_from_east / std::f32::consts::TAU) * cycle_w;
    for mut node in compass_q.iter_mut() {
        node.left = Val::Px(compass_clip_w / 2.0 - pixel_pos);
        node.width = Val::Px(compass_full_w);
        node.height = Val::Px(compass_strip_h);
    }
}

/// Re-render the footer text image whenever `FooterText` changes.
fn update_footer_text(
    footer: Res<FooterText>,
    mut last_gen: Local<u64>,
    game_fonts: Res<GameFonts>,
    ui_assets: Res<UiAssets>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cfg: Res<GameConfig>,
    mut images: ResMut<Assets<Image>>,
    mut query: Query<(&mut ImageNode, &mut Visibility, &mut Node), With<HudFooterText>>,
) {
    if footer.generation == *last_gen {
        return;
    }
    *last_gen = footer.generation;

    // Compute viewport width for centering
    let vp_w = windows.single().ok().map(|w| {
        let (_, _, vp_w, _) = viewport_rect(w, &cfg, &ui_assets);
        vp_w
    });

    for (mut img_node, mut vis, mut node) in query.iter_mut() {
        if footer.text.is_empty() {
            *vis = Visibility::Hidden;
        } else if let Some(handle) = game_fonts.render(
            &footer.text,
            &footer.font,
            footer.color,
            &mut images,
        ) {
            // Center the text: account for scaling (height constrains the image,
            // width scales proportionally)
            let text_px_w = game_fonts.measure(&footer.text, &footer.font) as f32;
            if let Some(vp_w) = vp_w {
                if let Some(font) = game_fonts.get(&footer.font) {
                    let native_h = font.height as f32;
                    if let Val::Px(display_h) = node.height {
                        let scale = display_h / native_h;
                        let display_w = text_px_w * scale;
                        node.left = Val::Px((vp_w - display_w) / 2.0);
                    }
                }
            }
            img_node.image = handle;
            *vis = Visibility::Inherited;
        }
    }
}

/// Parse aspect ratio string like "4:3" into a float (width/height).
fn parse_aspect_ratio(s: &str) -> Option<f32> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() == 2 {
        let w: f32 = parts[0].trim().parse().ok()?;
        let h: f32 = parts[1].trim().parse().ok()?;
        if h > 0.0 { Some(w / h) } else { None }
    } else {
        None
    }
}

/// Compute the letterboxed region within the physical window.
/// Returns (offset_x, offset_y, width, height) in physical pixels.
fn letterbox_rect(window: &Window, cfg: &GameConfig) -> (u32, u32, u32, u32) {
    let pw = window.physical_width();
    let ph = window.physical_height();
    if let Some(target) = parse_aspect_ratio(&cfg.aspect_ratio) {
        let current = pw as f32 / ph as f32;
        if (current - target).abs() < 0.01 {
            // Close enough — no bars needed
            (0, 0, pw, ph)
        } else if current > target {
            // Too wide — pillarbox
            let w = (ph as f32 * target) as u32;
            let ox = (pw - w) / 2;
            (ox, 0, w, ph)
        } else {
            // Too tall — letterbox
            let h = (pw as f32 / target) as u32;
            let oy = (ph - h) / 2;
            (0, oy, pw, h)
        }
    } else {
        (0, 0, pw, ph)
    }
}

/// Compute the 3D viewport rect in logical (CSS) pixels: (left, top, width, height).
/// This is the playable area — letterboxed region minus HUD sidebar and bottom bar.
pub fn viewport_rect(window: &Window, cfg: &GameConfig, ui: &UiAssets) -> (f32, f32, f32, f32) {
    let sf = window.scale_factor();
    let (_, _, lpw, lph) = letterbox_rect(window, cfg);
    let lw = lpw as f32 / sf;
    let lh = lph as f32 / sf;
    let d = hud_dimensions(lw, lh, ui);
    let bar_x = (window.width() - lw) / 2.0;
    let bar_y = (window.height() - lh) / 2.0;
    let footer_exposed = d.scale_h(FOOTER_H - 10.0 + 5.0);
    let vp_top = bar_y + d.border3_h;
    let vp_w = d.border2_w;
    let vp_h = lh - d.border2_h - d.border3_h - footer_exposed;
    (bar_x, vp_top, vp_w, vp_h)
}

/// Update the 3D camera viewport to the letterboxed area minus HUD borders.
/// The HUD camera has no viewport (full window) and clears to black for letterbox bars.
fn update_viewport(
    windows: Query<&Window, With<PrimaryWindow>>,
    cfg: Res<GameConfig>,
    ui_assets: Res<UiAssets>,
    mut player_cameras: Query<&mut Camera, With<PlayerCamera>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };

    let (lx, ly, lw, lh) = letterbox_rect(&window, &cfg);
    let sf = window.scale_factor();
    let lw_logical = lw as f32 / sf;
    let lh_logical = lh as f32 / sf;
    let d = hud_dimensions(lw_logical, lh_logical, &ui_assets);

    let sidebar_w = (d.border1_w * sf) as u32;
    let top_h = (d.border3_h * sf) as u32;
    let footer_exposed = (d.scale_h(FOOTER_H - 10.0 + 5.0) * sf) as u32;
    let bottom_h = (d.border2_h * sf) as u32 + footer_exposed;

    let vp_x = lx;
    let vp_y = ly + top_h;
    let vp_w = lw.saturating_sub(sidebar_w).max(1);
    let vp_h = lh.saturating_sub(top_h + bottom_h).max(1);

    for mut camera in player_cameras.iter_mut() {
        camera.viewport = Some(Viewport {
            physical_position: UVec2::new(vp_x, vp_y),
            physical_size: UVec2::new(vp_w, vp_h),
            ..default()
        });
    }
}
