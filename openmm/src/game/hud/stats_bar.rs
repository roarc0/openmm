//! Gold and food display on the border1 sidebar.
//! Positioned using MM6 UI reference coordinates:
//!   food  — right edge at (545, 256)
//!   gold  — right edge at (610, 256)

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::config::GameConfig;
use crate::fonts::{GameFonts, YELLOW};
use crate::game::world_state::WorldState;
use crate::ui_assets::UiAssets;

use super::borders::*;

const FOOD_REF_X: f32 = 545.0;
const GOLD_REF_X: f32 = 610.0;
/// Top edge of the stat box in MM6 UI reference pixels (center=256, top=252, height=8).
const STATS_REF_Y: f32 = 252.0;

#[derive(Component)]
pub(super) struct HudFoodText;

#[derive(Component)]
pub(super) struct HudGoldText;

/// Spawn food and gold text nodes as children of the HUD root.
pub(super) fn spawn_stats_bar(parent: &mut ChildSpawnerCommands) {
    // Food count — right edge at MM6 UI (545, 256)
    parent.spawn((
        Name::new("hud_food_text"),
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
        HudFoodText,
        super::HudUI,
    ));
    // Gold count — right edge at MM6 UI (610, 256)
    parent.spawn((
        Name::new("hud_gold_text"),
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
        HudGoldText,
        super::HudUI,
    ));
}

/// Update food and gold text nodes. Position recalculated every frame for window resize.
pub(super) fn update_stats_bar(
    world_state: Option<Res<WorldState>>,
    mut last_gold: Local<i32>,
    mut last_food: Local<i32>,
    game_fonts: Res<GameFonts>,
    ui_assets: Res<UiAssets>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cfg: Res<GameConfig>,
    mut images: ResMut<Assets<Image>>,
    mut food_q: Query<(&mut ImageNode, &mut Visibility, &mut Node), (With<HudFoodText>, Without<HudGoldText>)>,
    mut gold_q: Query<(&mut ImageNode, &mut Visibility, &mut Node), (With<HudGoldText>, Without<HudFoodText>)>,
) {
    let Some(ws) = world_state else { return };
    let gold = ws.game_vars.gold;
    let food = ws.game_vars.food;

    let Ok(window) = windows.single() else { return };
    let sf = window.scale_factor();
    let (_, _, lpw, lph) = letterbox_rect(window, &cfg);
    let lw = lpw as f32 / sf;
    let lh = lph as f32 / sf;

    let (ref_w, ref_h) = hud_canvas_dims(&ui_assets);
    let text_h = d_scale_h(lh, 12.0);

    // Convert MM6 ref coords to HudRoot-local, then derive `right` from the right edge
    let (food_px, food_py) = hud_ref_to_local(FOOD_REF_X, STATS_REF_Y, lw, lh, ref_w, ref_h);
    let (gold_px, gold_py) = hud_ref_to_local(GOLD_REF_X, STATS_REF_Y, lw, lh, ref_w, ref_h);
    let food_right = lw - food_px;
    let gold_right = lw - gold_px;

    let needs_update = gold != *last_gold || food != *last_food;

    for (mut img, mut vis, mut node) in food_q.iter_mut() {
        node.right = Val::Px(food_right);
        node.top = Val::Px(food_py);
        node.bottom = Val::Auto;
        node.height = Val::Px(text_h);
        if needs_update {
            if let Some(handle) = game_fonts.render(&food.to_string(), "smallnum", YELLOW, &mut images) {
                img.image = handle;
                *vis = Visibility::Inherited;
            }
        }
    }

    for (mut img, mut vis, mut node) in gold_q.iter_mut() {
        node.right = Val::Px(gold_right);
        node.top = Val::Px(gold_py);
        node.bottom = Val::Auto;
        node.height = Val::Px(text_h);
        if needs_update {
            if let Some(handle) = game_fonts.render(&gold.to_string(), "smallnum", YELLOW, &mut images) {
                img.image = handle;
                *vis = Visibility::Inherited;
            }
        }
    }

    if needs_update {
        *last_gold = gold;
        *last_food = food;
    }
}

/// Scale a reference height value (12px font height, etc.) to logical pixels.
fn d_scale_h(lh: f32, ref_px: f32) -> f32 {
    ref_px * lh / 480.0
}
