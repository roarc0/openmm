use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::config::GameConfig;
use crate::ui_assets::UiAssets;

use super::HudView;
use super::borders::{
    FOOTER_EXPOSED_H, hud_dimensions, letterbox_rect,
};

/// Resource holding the image to display as a fullscreen overlay in the viewport area.
/// Insert this resource (and set HudView to a non-World variant) to show an overlay.
/// Not auto-initialized by HudPlugin — callers insert/remove it on demand.
#[derive(Resource)]
pub struct OverlayImage {
    pub image: Handle<Image>,
}

/// Resource holding an NPC portrait image to display at actual size.
/// Like OverlayImage but rendered at 2× scale in the upper-left of the viewport.
#[derive(Resource)]
pub struct NpcPortrait {
    pub image: Handle<Image>,
    /// Natural pixel size of the portrait.
    pub size: Vec2,
}

/// Marker component for the overlay UI node.
#[derive(Component)]
pub(super) struct OverlayUI;

/// Marker component for the NPC portrait UI node.
#[derive(Component)]
pub(super) struct NpcPortraitUI;

/// Compute the inner viewport rect (excluding left border4) in logical pixels.
/// Returns (left, top, width, height).
/// Like `viewport_rect` but insets left by border4_w.
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
    let width = lw - d.border1_w - d.border4_w;
    let height = lh - d.border3_h - d.border2_h - footer_exposed;
    (left, top, width, height)
}

/// Spawn the overlay image node when HudView is not World and OverlayImage exists.
pub(super) fn spawn_overlay(
    mut commands: Commands,
    view: Res<HudView>,
    overlay_img: Option<Res<OverlayImage>>,
    existing: Query<Entity, With<OverlayUI>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cfg: Res<GameConfig>,
    ui_assets: Res<UiAssets>,
) {
    // Only spawn when not in World view, overlay image exists, and no existing overlay
    if matches!(*view, HudView::World) {
        return;
    }
    let Some(overlay) = overlay_img else { return };
    if !existing.is_empty() {
        return;
    }
    let Ok(window) = windows.single() else { return };

    let (left, top, width, height) = viewport_inner_rect(&window, &cfg, &ui_assets);

    commands.spawn((
        Name::new("overlay_ui"),
        ImageNode::new(overlay.image.clone()),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(left),
            top: Val::Px(top),
            width: Val::Px(width),
            height: Val::Px(height),
            ..default()
        },
        OverlayUI,
        super::HudUI,
        crate::game::InGame,
    ));
}

/// Despawn overlay UI nodes when HudView is World or OverlayImage is removed.
pub(super) fn despawn_overlay(
    mut commands: Commands,
    view: Res<HudView>,
    overlay_img: Option<Res<OverlayImage>>,
    existing: Query<Entity, With<OverlayUI>>,
) {
    if matches!(*view, HudView::World) || overlay_img.is_none() {
        for entity in existing.iter() {
            commands.entity(entity).despawn();
        }
    }
}

/// Update overlay layout on window resize.
pub(super) fn update_overlay_layout(
    windows: Query<&Window, With<PrimaryWindow>>,
    cfg: Res<GameConfig>,
    ui_assets: Res<UiAssets>,
    mut query: Query<&mut Node, With<OverlayUI>>,
) {
    let Ok(window) = windows.single() else { return };
    let (left, top, width, height) = viewport_inner_rect(&window, &cfg, &ui_assets);

    for mut node in query.iter_mut() {
        node.left = Val::Px(left);
        node.top = Val::Px(top);
        node.width = Val::Px(width);
        node.height = Val::Px(height);
    }
}

/// Spawn the NPC portrait node at 2× scale in the upper-left of the viewport inner area.
pub(super) fn spawn_npc_portrait(
    mut commands: Commands,
    portrait: Option<Res<NpcPortrait>>,
    view: Res<HudView>,
    existing: Query<Entity, With<NpcPortraitUI>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cfg: Res<GameConfig>,
    ui_assets: Res<UiAssets>,
) {
    if matches!(*view, HudView::World) || portrait.is_none() || !existing.is_empty() {
        return;
    }
    let Some(portrait) = portrait else { return };
    let Ok(window) = windows.single() else { return };
    let (left, top, _, _) = viewport_inner_rect(&window, &cfg, &ui_assets);

    // Display at 2× native size so the portrait is readable
    let w = portrait.size.x * 2.0;
    let h = portrait.size.y * 2.0;

    commands.spawn((
        Name::new("npc_portrait_ui"),
        ImageNode::new(portrait.image.clone()),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(left + 10.0),
            top: Val::Px(top + 10.0),
            width: Val::Px(w),
            height: Val::Px(h),
            ..default()
        },
        NpcPortraitUI,
        super::HudUI,
        crate::game::InGame,
    ));
}

/// Despawn NPC portrait node when returning to World view or portrait removed.
pub(super) fn despawn_npc_portrait(
    mut commands: Commands,
    view: Res<HudView>,
    portrait: Option<Res<NpcPortrait>>,
    existing: Query<Entity, With<NpcPortraitUI>>,
) {
    if matches!(*view, HudView::World) || portrait.is_none() {
        for entity in existing.iter() {
            commands.entity(entity).despawn();
        }
    }
}
