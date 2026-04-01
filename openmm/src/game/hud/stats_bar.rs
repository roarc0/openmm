//! Gold and food display on the bottom-right of the HUD bar.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::config::GameConfig;
use crate::fonts::{GameFonts, YELLOW};
use crate::game::world_state::WorldState;
use crate::ui_assets::UiAssets;

use super::borders::*;

/// Marker for the gold text image node.
#[derive(Component)]
pub(super) struct HudGoldText;

/// Marker for the food text image node.
#[derive(Component)]
pub(super) struct HudFoodText;

/// Spawn gold and food text nodes as children of the HUD root.
pub(super) fn spawn_stats_bar(parent: &mut ChildSpawnerCommands) {
    // Gold text
    parent.spawn((
        Name::new("hud_gold_text"),
        ImageNode::new(Handle::default()),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(0.0),
            bottom: Val::Px(0.0),
            width: Val::Auto,
            height: Val::Auto,
            ..default()
        },
        Visibility::Hidden,
        HudGoldText,
        super::HudUI,
    ));

    // Food text
    parent.spawn((
        Name::new("hud_food_text"),
        ImageNode::new(Handle::default()),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(0.0),
            bottom: Val::Px(0.0),
            width: Val::Auto,
            height: Val::Auto,
            ..default()
        },
        Visibility::Hidden,
        HudFoodText,
        super::HudUI,
    ));
}

/// Update gold and food text when values change.
pub(super) fn update_stats_bar(
    world_state: Option<Res<WorldState>>,
    mut last_gold: Local<i32>,
    mut last_food: Local<i32>,
    game_fonts: Res<GameFonts>,
    ui_assets: Res<UiAssets>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cfg: Res<GameConfig>,
    mut images: ResMut<Assets<Image>>,
    mut gold_q: Query<
        (&mut ImageNode, &mut Visibility, &mut Node),
        (With<HudGoldText>, Without<HudFoodText>),
    >,
    mut food_q: Query<
        (&mut ImageNode, &mut Visibility, &mut Node),
        (With<HudFoodText>, Without<HudGoldText>),
    >,
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

    let needs_gold_update = gold != *last_gold;
    let needs_food_update = food != *last_food;
    if !needs_gold_update && !needs_food_update {
        return;
    }

    let font_name = "smallnum";
    // Text height: scale relative to the bottom bar
    let text_h = d.scale_h(12.0);

    // Position: bottom-right area, inside border2 but left of border1
    // In MM6 reference: gold at ~(388, 216) from top-left of 640×480 = bottom bar y ~316..424
    // Relative to bottom-left: gold at x≈388, from bottom≈480-216-12=252 → within border2
    // Simpler: position relative to right edge and bottom
    let right_margin = d.border1_w + d.scale_w(16.0);

    if needs_gold_update {
        *last_gold = gold;
        let gold_text = format!("{}", gold);
        for (mut img_node, mut vis, mut node) in gold_q.iter_mut() {
            if let Some(handle) = game_fonts.render(&gold_text, font_name, YELLOW, &mut images) {
                img_node.image = handle;
                *vis = Visibility::Inherited;
                // Bottom of border2 area, above the very bottom
                node.right = Val::Px(right_margin);
                node.bottom = Val::Px(d.scale_h(22.0));
                node.height = Val::Px(text_h);
            }
        }
    }

    if needs_food_update {
        *last_food = food;
        let food_text = format!("{}", food);
        for (mut img_node, mut vis, mut node) in food_q.iter_mut() {
            if let Some(handle) = game_fonts.render(&food_text, font_name, YELLOW, &mut images) {
                img_node.image = handle;
                *vis = Visibility::Inherited;
                node.right = Val::Px(right_margin);
                node.bottom = Val::Px(d.scale_h(7.0));
                node.height = Val::Px(text_h);
            }
        }
    }
}
