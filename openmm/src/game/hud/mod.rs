mod borders;
mod crosshair;
mod footer;
mod minimap;
mod overlay;
mod stats_bar;

pub use borders::{parse_aspect_ratio, viewport_rect};
pub use footer::FooterText;
pub use overlay::{NpcPortrait, OverlayImage, viewport_inner_rect};

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::GameState;
use crate::assets::GameAssets;
use crate::config::GameConfig;
use crate::game::InGame;
use crate::ui_assets::{UiAssets, make_black_transparent};

use borders::*;
use footer::*;
use minimap::*;

/// Marker for HUD UI entities.
#[derive(Component)]
pub(super) struct HudUI;

/// Which view the HUD is currently displaying.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HudView {
    #[default]
    World,
    /// Entered a building interior (shop, temple, tavern, etc.).
    Building,
    /// Talking to a street NPC.
    NpcDialogue,
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
                    freeze_system,
                )
                    .chain()
                    .run_if(in_state(GameState::Game)),
            );
    }
}

/// Pause/unpause virtual time based on HudView.
/// Runs every frame to enforce the invariant (not just on change).
fn freeze_system(view: Res<HudView>, mut time: ResMut<Time<Virtual>>) {
    if matches!(*view, HudView::World) {
        time.unpause();
    } else {
        time.pause();
    }
}

fn spawn_hud(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    cfg: Res<GameConfig>,
    mut footer_text: ResMut<FooterText>,
    mut ui_assets: ResMut<UiAssets>,
    mut images: ResMut<Assets<Image>>,
) {
    // HUD camera -- covers the full window, clears to black (letterbox bar color).
    // Order 1 ensures it renders after the 3D camera (order 0), which overwrites
    // its viewport area with the sky. The HUD then draws on top.
    commands.spawn((
        Name::new("hud_camera"),
        Camera2d,
        Camera {
            order: 1,
            clear_color: ClearColorConfig::None,
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
    let border5 = ui_assets.get_or_load_transformed(
        "border5",
        "border5_transparent",
        &game_assets,
        &mut images,
        &cfg,
        make_black_transparent,
    );
    let border6 = ui_assets.get_or_load_transformed(
        "border6",
        "border6_transparent",
        &game_assets,
        &mut images,
        &cfg,
        make_black_transparent,
    );
    let footer = ui_assets.get_or_load("footer", &game_assets, &mut images, &cfg);

    // Load tap frames (minimap border with time-of-day sky)
    // tap1=morning, tap2=day, tap3=evening, tap4=night -- green/red key made transparent
    let tap_handles: Vec<Handle<Image>> = (1..=4)
        .filter_map(|i| {
            let name = format!("tap{}", i);
            let key = format!("{}_transparent", name);
            ui_assets.get_or_load_transformed(&name, &key, &game_assets, &mut images, &cfg, make_tap_key_transparent)
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
            ui_assets.get_or_load_transformed(&name, &key, &game_assets, &mut images, &cfg, make_black_transparent)
        })
        .collect();
    let default_arrow = arrow_handles.first().cloned();

    // Root node -- positioned within the letterboxed area by update_hud_layout
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
            // footer -- text strip, drawn first so border1/border2 render on top
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

            // border2 -- bottom bar (469x109), anchored bottom-left (spawned before border1 so border1 is on top)
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

            // border1 -- right sidebar (172x339), anchored bottom-right (on top of border2)
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

            // border3 -- horizontal divider strip (468x8), above bottom bar
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

            // border4 -- vertical divider strip (8x344), left edge of viewport only
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

            // border5 -- corner piece (8x20)
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

            // border6 -- corner piece (7x21)
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

            // Minimap clipped container -- sized to mapback area, clips the map image
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
                    // Map overview image inside -- large, positioned by player pos
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

            // Compass strip -- behind tap, scrolling direction indicator in tap's compass channel
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

            // Tap frame -- minimap border with transparent window, on top of compass and map
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

            // Minimap arrow -- player direction indicator at center, on top of tap
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

            // Gold and food text display
            stats_bar::spawn_stats_bar(parent);

            // Footer text -- spawned last so it renders on top of everything
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

    crosshair::spawn_crosshair(&mut commands, &cfg);

    // Set initial footer message
    footer_text.set("Welcome to New Sorpigal");
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
    let (_, _, lpw, lph) = letterbox_rect(window, &cfg);
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

    // border1 -- right sidebar (172x339 ref), true scaled size
    for mut node in set.p0().iter_mut() {
        node.top = Val::Px(d.tap_h);
        node.bottom = Val::Auto;
        node.width = Val::Px(d.border1_w);
        node.height = Val::Px(d.border1_h);
    }
    // border2 -- bottom bar (469x109 ref), true scaled size
    for mut node in set.p1().iter_mut() {
        node.width = Val::Px(d.border2_w);
        node.height = Val::Px(d.border2_h);
    }
    // border3 -- horizontal strip (468x8 ref) across the top, true scaled size
    for mut node in set.p2().iter_mut() {
        node.left = Val::Px(0.0);
        node.top = Val::Px(0.0);
        node.bottom = Val::Auto;
        node.width = Val::Px(d.border3_w);
        node.height = Val::Px(d.border3_h);
    }
    // border4 -- vertical divider strip, left edge of viewport, below border3
    for mut node in set.p3().iter_mut() {
        node.left = Val::Px(0.0);
        node.top = Val::Px(d.border3_h);
        node.right = Val::Auto;
        node.width = Val::Px(d.border4_w);
        node.height = Val::Px(d.border4_h);
    }
    // border5 -- top-left corner piece, floating over the viewport
    for mut node in set.p4().iter_mut() {
        node.left = Val::Px(d.border4_w);
        node.top = Val::Px(d.border3_h);
        node.right = Val::Auto;
        node.bottom = Val::Auto;
        node.width = Val::Px(d.border5_w);
        node.height = Val::Px(d.border5_h);
    }
    // border6 -- top-right corner piece, floating over the viewport
    for mut node in set.p5().iter_mut() {
        node.right = Val::Px(d.border1_w);
        node.top = Val::Px(d.border3_h);
        node.left = Val::Auto;
        node.bottom = Val::Auto;
        node.width = Val::Px(d.border6_w);
        node.height = Val::Px(d.border6_h);
    }
    // Corner fill -- bottom-right area (below border1+mapback)
    for mut node in set.p6().iter_mut() {
        node.width = Val::Px(d.border1_w);
        node.height = Val::Px(d.corner_h);
    }
    // Tap frame -- at the very top
    for mut node in set.p7().iter_mut() {
        node.right = Val::Px(tap_offset_x);
        node.top = Val::Px(0.0);
        node.width = Val::Px(d.tap_w);
        node.height = Val::Px(d.tap_h);
    }
    // Minimap clip container -- same position/size as tap frame
    for mut node in minimap_clip_q.iter_mut() {
        node.right = Val::Px(tap_offset_x);
        node.top = Val::Px(0.0);
        node.width = Val::Px(d.tap_w);
        node.height = Val::Px(d.tap_h);
    }
    // Minimap arrow -- centered in tap area
    let arrow_size = d.scale_w(7.0);
    for mut node in arrow_q.iter_mut() {
        node.right = Val::Px(tap_offset_x + (d.tap_w - arrow_size) / 2.0);
        node.top = Val::Px((d.tap_h - arrow_size) / 2.0);
        node.width = Val::Px(arrow_size);
        node.height = Val::Px(arrow_size);
    }
    // Footer (483x24 ref) -- sits directly above border2, overlapping its top by 10px.
    let footer_overlap = d.scale_h(FOOTER_OVERLAP);
    let footer_lift = d.scale_h(FOOTER_LIFT);
    let footer_bottom = d.border2_h - footer_overlap + footer_lift;
    for mut node in footer_q.iter_mut() {
        node.bottom = Val::Px(footer_bottom);
        node.width = Val::Px(d.footer_w);
        node.height = Val::Px(d.footer_h);
    }
    // Footer text -- scaled to footer height, centered vertically
    let text_h = d.footer_h * 0.6;
    let text_bottom = footer_bottom + (d.footer_h - text_h) / 2.0;
    for mut node in footer_text_q.iter_mut() {
        node.bottom = Val::Px(text_bottom);
        node.width = Val::Auto;
        node.height = Val::Px(text_h);
    }
    // Compass clip -- floating inside tap frame, in the compass channel
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
