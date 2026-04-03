//! Gold and food display on the border1 sidebar.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::config::GameConfig;
use crate::fonts::{GameFonts, YELLOW};
use crate::game::world_state::WorldState;
use crate::ui_assets::UiAssets;

use super::borders::*;

/// Marker for the combined food + gold text image node.
#[derive(Component)]
pub(super) struct HudStatsText;

/// Spawn the combined stats text node as a child of the HUD root.
pub(super) fn spawn_stats_bar(parent: &mut ChildSpawnerCommands) {
    parent.spawn((
        Name::new("hud_stats_text"),
        ImageNode::new(Handle::default()),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Auto,
            height: Val::Auto,
            ..default()
        },
        Visibility::Hidden,
        HudStatsText,
        super::HudUI,
    ));
}

/// Update the combined food + gold text when either value changes.
/// Position is updated every frame because it depends on window size.
pub(super) fn update_stats_bar(
    world_state: Option<Res<WorldState>>,
    mut last_gold: Local<i32>,
    mut last_food: Local<i32>,
    game_fonts: Res<GameFonts>,
    ui_assets: Res<UiAssets>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cfg: Res<GameConfig>,
    mut images: ResMut<Assets<Image>>,
    mut stats_q: Query<(&mut ImageNode, &mut Visibility, &mut Node), With<HudStatsText>>,
) {
    let Some(ws) = world_state else { return };
    let gold = ws.game_vars.gold;
    let food = ws.game_vars.food;

    let Ok(window) = windows.single() else { return };
    let sf = window.scale_factor();
    let (_, _, lpw, lph) = letterbox_rect(window, &cfg);
    let lw = lpw as f32 / sf;
    let lh = lph as f32 / sf;
    let d = hud_dimensions(lw, lh, &ui_assets);

    let text_h = d.scale_h(12.0);
    let right = d.scale_w(8.0);
    let top = d.tap_h + d.scale_h(10.0);

    let needs_update = gold != *last_gold || food != *last_food;

    for (mut img_node, mut vis, mut node) in stats_q.iter_mut() {
        // Always update position — depends on window size
        node.right = Val::Px(right);
        node.top = Val::Px(top);
        node.bottom = Val::Auto;
        node.height = Val::Px(text_h);

        if needs_update {
            let text = format!("{}  {}", food, gold);
            if let Some(handle) = game_fonts.render(&text, "smallnum", YELLOW, &mut images) {
                img_node.image = handle;
                *vis = Visibility::Inherited;
            }
        }
    }

    if needs_update {
        *last_gold = gold;
        *last_food = food;
    }
}
