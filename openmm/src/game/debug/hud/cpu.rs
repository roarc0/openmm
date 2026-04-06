use bevy::prelude::*;
use super::common::{CpuText, HudThrottle};

pub fn update_cpu_text(
    throttle: Res<HudThrottle>,
    cpu_stats: Res<crate::game::debug::cpu_usage::CpuStats>,
    mut query: Query<&mut Text, With<CpuText>>,
) {
    if !throttle.0.just_finished() {
        return;
    }

    let cpu_proc = cpu_stats.process_usage as f64;
    let cpu_sys = cpu_stats.system_usage as f64;
    let cpu_str = format!(" CPU: {cpu_proc:.1}% (sys: {cpu_sys:.1}%)");

    for mut text in &mut query {
        if **text != cpu_str {
            **text = cpu_str.clone();
        }
    }
}
