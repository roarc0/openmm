use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::config::GameConfig;
use crate::ui_assets::UiAssets;

use super::{HudUI, HudView, MapOverviewImage};
use super::overlay::viewport_inner_rect;

/// Marker for the fullscreen map overlay UI node.
#[derive(Component)]
pub(super) struct MapOverlayUI;

/// Compute the overlay rect (left, top, size, size) centered in the inner viewport.
///
/// Applies 10% margin on each side — the display occupies 80% of the available area.
/// The map image is 1:1, so size = min(available_w, available_h).
pub(super) fn map_overlay_rect(
    inner_left: f32,
    inner_top: f32,
    inner_w: f32,
    inner_h: f32,
) -> (f32, f32, f32, f32) {
    let available_w = inner_w * 0.8;
    let available_h = inner_h * 0.8;
    let size = available_w.min(available_h);
    let left = inner_left + (inner_w - size) / 2.0;
    let top = inner_top + (inner_h - size) / 2.0;
    (left, top, size, size)
}

/// Toggle the fullscreen map view on M key press.
/// Opens only when in World view and an outdoor map is loaded.
/// Closes from Map view back to World.
pub(super) fn map_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut view: ResMut<HudView>,
    map_image: Option<Res<MapOverviewImage>>,
) {
    if !keys.just_pressed(KeyCode::KeyM) {
        return;
    }
    match *view {
        HudView::World => {
            if map_image.as_ref().and_then(|m| m.0.as_ref()).is_some() {
                *view = HudView::Map;
            }
        }
        HudView::Map => {
            *view = HudView::World;
        }
        _ => {}
    }
}

/// Spawn the map overlay image node when HudView is Map and none exists yet.
pub(super) fn spawn_map_overlay(
    mut commands: Commands,
    view: Res<HudView>,
    map_image: Option<Res<MapOverviewImage>>,
    existing: Query<Entity, With<MapOverlayUI>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cfg: Res<GameConfig>,
    ui_assets: Res<UiAssets>,
) {
    if !matches!(*view, HudView::Map) || !existing.is_empty() {
        return;
    }
    let Some(map_image) = map_image else { return };
    let Some(ref handle) = map_image.0 else { return };
    let Ok(window) = windows.single() else { return };

    let (il, it, iw, ih) = viewport_inner_rect(window, &cfg, &ui_assets);
    let (left, top, size, _) = map_overlay_rect(il, it, iw, ih);

    commands.spawn((
        Name::new("map_overlay"),
        ImageNode::new(handle.clone()),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(left),
            top: Val::Px(top),
            width: Val::Px(size),
            height: Val::Px(size),
            ..default()
        },
        MapOverlayUI,
        HudUI,
        crate::game::InGame,
    ));
}

/// Despawn the map overlay node when leaving Map view.
pub(super) fn despawn_map_overlay(
    mut commands: Commands,
    view: Res<HudView>,
    existing: Query<Entity, With<MapOverlayUI>>,
) {
    if matches!(*view, HudView::Map) {
        return;
    }
    for entity in existing.iter() {
        commands.entity(entity).despawn();
    }
}

/// Update overlay position and size on window resize.
pub(super) fn update_map_overlay_layout(
    view: Res<HudView>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cfg: Res<GameConfig>,
    ui_assets: Res<UiAssets>,
    mut query: Query<&mut Node, With<MapOverlayUI>>,
) {
    if !matches!(*view, HudView::Map) {
        return;
    }
    let Ok(window) = windows.single() else { return };
    let (il, it, iw, ih) = viewport_inner_rect(window, &cfg, &ui_assets);
    let (left, top, size, _) = map_overlay_rect(il, it, iw, ih);

    for mut node in query.iter_mut() {
        node.left = Val::Px(left);
        node.top = Val::Px(top);
        node.width = Val::Px(size);
        node.height = Val::Px(size);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_viewport_limited_by_height() {
        // 800×600: available = 640×480, size = 480 (height wins)
        let (left, top, w, h) = map_overlay_rect(0.0, 0.0, 800.0, 600.0);
        let size = 600.0 * 0.8;
        assert_eq!(w, size);
        assert_eq!(h, size);
        assert!((left - (800.0 - size) / 2.0).abs() < 0.001);
        assert!((top - (600.0 - size) / 2.0).abs() < 0.001);
    }

    #[test]
    fn tall_viewport_limited_by_width() {
        // 400×700 with offset: available = 320×560, size = 320 (width wins)
        let (left, top, w, h) = map_overlay_rect(10.0, 20.0, 400.0, 700.0);
        let size = 400.0 * 0.8;
        assert_eq!(w, size);
        assert_eq!(h, size);
        assert!((left - (10.0 + (400.0 - size) / 2.0)).abs() < 0.001);
        assert!((top - (20.0 + (700.0 - size) / 2.0)).abs() < 0.001);
    }

    #[test]
    fn square_viewport_both_equal() {
        let (_, _, w, h) = map_overlay_rect(0.0, 0.0, 500.0, 500.0);
        assert_eq!(w, 400.0);
        assert_eq!(h, 400.0);
    }
}
