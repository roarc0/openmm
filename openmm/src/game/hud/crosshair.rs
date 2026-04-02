use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::config::GameConfig;
use crate::game::InGame;
use crate::ui_assets::UiAssets;

use super::HudUI;
use super::borders::viewport_rect;

#[derive(Component)]
pub(super) struct Crosshair;

pub(super) fn spawn_crosshair(commands: &mut Commands, cfg: &GameConfig) {
    let visibility = if cfg.crosshair {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    let color = BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.9));

    commands
        .spawn((
            Name::new("crosshair"),
            Node {
                position_type: PositionType::Absolute,
                ..default()
            },
            visibility,
            GlobalZIndex(50),
            Crosshair,
            InGame,
            HudUI,
        ))
        .with_children(|parent| {
            // H bar: full 20px wide, 2px tall, vertically centered in the 20x20 box
            parent.spawn((
                Name::new("crosshair_h"),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(9.0),
                    width: Val::Px(20.0),
                    height: Val::Px(2.0),
                    ..default()
                },
                color,
            ));
            // V bar: 2px wide, full 20px tall, horizontally centered in the 20x20 box
            parent.spawn((
                Name::new("crosshair_v"),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(9.0),
                    top: Val::Px(0.0),
                    width: Val::Px(2.0),
                    height: Val::Px(20.0),
                    ..default()
                },
                color,
            ));
        });
}

pub(super) fn update_crosshair(
    windows: Query<&Window, With<PrimaryWindow>>,
    cfg: Res<GameConfig>,
    ui_assets: Res<UiAssets>,
    mut query: Query<(&mut Node, &mut Visibility), With<Crosshair>>,
) {
    let Ok(window) = windows.single() else { return };
    let (vp_left, vp_top, vp_w, vp_h) = viewport_rect(window, &cfg, &ui_assets);
    let cx = vp_left + vp_w / 2.0;
    let cy = vp_top + vp_h / 2.0;

    for (mut node, mut vis) in query.iter_mut() {
        // Position the parent so children's absolute offsets are relative to the center
        node.left = Val::Px(cx - 10.0);
        node.top = Val::Px(cy - 10.0);
        *vis = if cfg.crosshair {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}
