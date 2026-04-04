/// Debug overlay: shows HUD pixel coordinates next to the mouse cursor when
/// `cfg.debug = true` and the cursor is free (not grabbed by the game).
///
/// Coordinate space: the full HUD canvas in original image pixels.
///   width  = border1.w + border2.w - 1   (right sidebar + bottom/left bar, 1px overlap)
///   height = border1.h
/// Coordinates directly correspond to pixel positions in the source HUD images.
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use crate::GameState;
use crate::config::GameConfig;
use crate::game::InGame;
use crate::ui_assets::UiAssets;

use super::borders::letterbox_rect;

#[derive(Component)]
struct HudCoordsLabel;

pub struct DebugHudCoordsPlugin;

impl Plugin for DebugHudCoordsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Game), spawn_label)
            .add_systems(Update, update_label.run_if(in_state(GameState::Game)));
    }
}

fn spawn_label(mut commands: Commands) {
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.0, 1.0)), // magenta — high contrast on any background
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            ..default()
        },
        HudCoordsLabel,
        InGame,
        Visibility::Hidden,
    ));
}

fn update_label(
    cfg: Res<GameConfig>,
    windows: Query<(&Window, &CursorOptions), With<PrimaryWindow>>,
    ui_assets: Res<UiAssets>,
    mut query: Query<(&mut Text, &mut Node, &mut Visibility), With<HudCoordsLabel>>,
) {
    let Ok((mut text, mut node, mut vis)) = query.single_mut() else {
        return;
    };

    let Ok((window, cursor_opts)) = windows.single() else {
        *vis = Visibility::Hidden;
        return;
    };

    // Show only when debug is on and cursor is free (not grabbed by gameplay).
    if !cfg.debug || cursor_opts.grab_mode != CursorGrabMode::None {
        *vis = Visibility::Hidden;
        return;
    }

    let Some(cursor) = window.cursor_position() else {
        *vis = Visibility::Hidden;
        return;
    };

    // Full HUD canvas size in original image pixels:
    //   width  = border1.w + border2.w - 1  (1px overlap between the two panels)
    //   height = border1.h
    let (b1w, b1h) = ui_assets.dimensions("border1.pcx").unwrap_or((0, 480));
    let (b2w, _)   = ui_assets.dimensions("border2.pcx").unwrap_or((0, 0));
    let ref_w = (b1w + b2w).saturating_sub(1) as f32;
    let ref_h = b1h as f32;

    // Letterbox origin and size: physical pixels → logical
    let sf = window.scale_factor();
    let (lx, ly, lpw, lph) = letterbox_rect(window, &cfg);
    let bar_x = lx as f32 / sf;
    let bar_y = ly as f32 / sf;
    let lw = lpw as f32 / sf;
    let lh = lph as f32 / sf;

    let ref_x = (cursor.x - bar_x) * ref_w / lw;
    let ref_y = (cursor.y - bar_y) * ref_h / lh;

    **text = format!("{:.0},{:.0}", ref_x, ref_y);
    node.left = Val::Px(cursor.x + 12.0);
    node.top = Val::Px(cursor.y + 12.0);
    *vis = Visibility::Inherited;
}
