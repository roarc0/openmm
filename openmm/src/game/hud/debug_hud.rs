/// Debug overlay: shows HUD reference coordinates (640×480 space) next to the mouse cursor
/// when `cfg.debug = true` and the HUD is in any non-World view.
///
/// Coordinate mapping: physical mouse pos → logical px → reference px (0‥640, 0‥480)
/// using the same letterbox origin and scale that all other HUD elements use.
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::GameState;
use crate::config::GameConfig;
use crate::game::InGame;

use super::HudView;
use super::borders::{REF_H, REF_W, letterbox_rect};

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
    hud_view: Res<HudView>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut query: Query<(&mut Text, &mut Node, &mut Visibility), With<HudCoordsLabel>>,
) {
    let Ok((mut text, mut node, mut vis)) = query.single_mut() else {
        return;
    };

    if !cfg.debug || matches!(*hud_view, HudView::World) {
        *vis = Visibility::Hidden;
        return;
    }

    let Ok(window) = windows.single() else {
        *vis = Visibility::Hidden;
        return;
    };

    let Some(cursor) = window.cursor_position() else {
        *vis = Visibility::Hidden;
        return;
    };

    // Letterbox origin and size in physical pixels → convert to logical
    let sf = window.scale_factor();
    let (lx, ly, lpw, lph) = letterbox_rect(window, &cfg);
    let bar_x = lx as f32 / sf;
    let bar_y = ly as f32 / sf;
    let lw = lpw as f32 / sf;
    let lh = lph as f32 / sf;

    // Map cursor into 640×480 reference space
    let ref_x = (cursor.x - bar_x) * REF_W / lw;
    let ref_y = (cursor.y - bar_y) * REF_H / lh;

    **text = format!("{:.0},{:.0}", ref_x, ref_y);
    node.left = Val::Px(cursor.x + 12.0);
    node.top = Val::Px(cursor.y + 12.0);
    *vis = Visibility::Inherited;
}
