use super::common::{FpsText, HudThrottle, fps_color};
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;

pub const FPS_HISTORY_SIZE: usize = 120;
pub const FPS_AVG_WINDOW: usize = 30;
pub const FPS_SAMPLE_INTERVAL: usize = 15;

#[derive(Resource)]
pub struct FpsHistory {
    pub samples: std::collections::VecDeque<f64>,
    pub frame_counter: usize,
    pub accumulator: f64,
    pub accum_count: usize,
}

impl Default for FpsHistory {
    fn default() -> Self {
        Self {
            samples: std::collections::VecDeque::with_capacity(FPS_HISTORY_SIZE),
            frame_counter: 0,
            accumulator: 0.0,
            accum_count: 0,
        }
    }
}

impl FpsHistory {
    pub fn tick(&mut self, fps: f64) {
        self.accumulator += fps;
        self.accum_count += 1;
        self.frame_counter += 1;
        if self.frame_counter >= FPS_SAMPLE_INTERVAL {
            let avg = self.accumulator / self.accum_count as f64;
            if self.samples.len() >= FPS_HISTORY_SIZE {
                self.samples.pop_front();
            }
            self.samples.push_back(avg);
            self.frame_counter = 0;
            self.accumulator = 0.0;
            self.accum_count = 0;
        }
    }

    pub fn averaged(&self) -> f64 {
        let n = self.samples.len().min(FPS_AVG_WINDOW);
        if n == 0 {
            return 0.0;
        }
        let start = self.samples.len() - n;
        self.samples.range(start..).sum::<f64>() / n as f64
    }

    pub fn percentile_low(&self, pct: f32) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let mut sorted: Vec<f64> = self.samples.iter().copied().collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let count = ((sorted.len() as f32 * pct / 100.0).ceil() as usize).max(1);
        sorted[..count].iter().sum::<f64>() / count as f64
    }

    pub fn chart_min_max(&self) -> (f64, f64) {
        let width = 60.min(self.samples.len());
        if width == 0 {
            return (0.0, 60.0);
        }
        let start = self.samples.len() - width;
        let min = self.samples.range(start..).copied().fold(f64::MAX, f64::min);
        let max = self.samples.range(start..).copied().fold(0.0_f64, f64::max);
        (min, max)
    }
}

pub fn tick_fps_history(diagnostics: Res<DiagnosticsStore>, mut fps_history: ResMut<FpsHistory>) {
    let fps_val = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|fps| fps.smoothed());

    if let Some(fps) = fps_val {
        fps_history.tick(fps);
    }
}

pub fn update_fps_text(
    time: Res<Time>,
    mut throttle: ResMut<HudThrottle>,
    fps_history: Res<FpsHistory>,
    mut query: Query<(&mut Text, &mut TextColor), With<FpsText>>,
) {
    if !throttle.0.tick(time.delta()).just_finished() {
        return;
    }

    let avg = fps_history.averaged();
    let low_1 = fps_history.percentile_low(1.0);
    let fps_str = if avg > 0.0 {
        format!("FPS: {:.0} ({:.0} min)", avg, low_1)
    } else {
        "FPS: --".into()
    };
    let color = if avg > 0.0 { fps_color(avg) } else { Color::WHITE };

    for (mut text, mut tc) in &mut query {
        if **text != fps_str {
            **text = fps_str.clone();
            *tc = TextColor(color);
        }
    }
}
