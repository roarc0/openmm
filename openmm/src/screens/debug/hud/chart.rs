use super::common::*;
use super::fps::FpsHistory;
use bevy::prelude::*;

pub const FPS_CHART_WIDTH: usize = 60;
pub const FPS_CHART_HEIGHT: f32 = 50.0;
pub const FPS_CHART_BAR_W: f32 = 3.0;

#[derive(Component)]
pub struct FpsChartBar(pub usize);

pub fn update_chart_labels(
    throttle: Res<HudThrottle>,
    fps_history: Res<FpsHistory>,
    mut max_query: Query<&mut Text, (With<ChartMaxLabel>, Without<ChartMinLabel>)>,
    mut min_query: Query<&mut Text, (With<ChartMinLabel>, Without<ChartMaxLabel>)>,
) {
    if !throttle.0.just_finished() {
        return;
    }

    let (chart_min, chart_max) = fps_history.chart_min_max();
    let scale_min = ((chart_min / 10.0).floor() * 10.0).max(0.0);
    let scale_max = ((chart_max / 20.0).ceil() * 20.0).max(scale_min + 20.0);

    let max_str = format!("{scale_max:.0}");
    let min_str = format!("{scale_min:.0}");

    for mut text in &mut max_query {
        if **text != max_str {
            **text = max_str.clone();
        }
    }
    for mut text in &mut min_query {
        if **text != min_str {
            **text = min_str.clone();
        }
    }
}

pub fn update_fps_chart(
    throttle: Res<HudThrottle>,
    fps_history: Res<FpsHistory>,
    mut bar_query: Query<(&FpsChartBar, &mut Node, &mut BackgroundColor)>,
) {
    if !throttle.0.just_finished() {
        return;
    }

    let (chart_min, chart_max) = fps_history.chart_min_max();
    let scale_min = ((chart_min / 10.0).floor() * 10.0).max(0.0);
    let scale_max = ((chart_max / 20.0).ceil() * 20.0).max(scale_min + 20.0);

    let width = FPS_CHART_WIDTH.min(fps_history.samples.len());
    let start = fps_history.samples.len().saturating_sub(FPS_CHART_WIDTH);
    let range = (scale_max - scale_min).max(1.0);

    for (bar, mut node, mut bg) in bar_query.iter_mut() {
        let idx = bar.0;
        if idx < width {
            let fps = fps_history.samples[start + idx];
            let ratio = ((fps - scale_min) / range).clamp(0.0, 1.0) as f32;
            node.height = Val::Px(ratio * FPS_CHART_HEIGHT);
            *bg = BackgroundColor(fps_color(fps));
        } else {
            node.height = Val::Px(0.0);
        }
    }
}
