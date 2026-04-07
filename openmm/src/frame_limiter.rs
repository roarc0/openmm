//! Frame rate limiter — honours `GameConfig::fps_cap` regardless of vsync mode.
//!
//! Bevy/wgpu's `PresentMode` only chooses how the swapchain pulls frames; it
//! does NOT cap the main loop. With `vsync=off` and no limiter the loop spins
//! at thousands of fps and saturates CPU cores. With `vsync=auto` it matches
//! monitor refresh, which on a 144/240 Hz panel still burns far more CPU than
//! a 60 Hz target.
//!
//! Uses absolute frame deadlines (so a single oversleep doesn't permanently
//! slow the cap) and a short spin on the final sub-millisecond to counteract
//! `thread::sleep` granularity (~1ms on Linux, coarser on Windows).
//!
//! `fps_cap == 0` disables the limiter entirely.

use bevy::prelude::*;
use std::thread;
use std::time::{Duration, Instant};

use crate::config::GameConfig;

/// Spin for the final slice of the frame budget to absorb sleep granularity.
/// 1ms is enough on Linux; bump to 2ms if running on Windows without a
/// high-resolution timer.
const SPIN_TAIL: Duration = Duration::from_micros(1_000);

pub struct FrameLimiterPlugin;

impl Plugin for FrameLimiterPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Last, limit_frame_rate);
    }
}

fn limit_frame_rate(cfg: Res<GameConfig>, mut next_deadline: Local<Option<Instant>>) {
    let fps_cap = cfg.fps_cap;
    if fps_cap == 0 {
        *next_deadline = None;
        return;
    }

    let frame_time = Duration::from_secs_f64(1.0 / fps_cap as f64);
    let now = Instant::now();
    let deadline = match *next_deadline {
        Some(d) => d,
        None => now + frame_time,
    };

    // Coarse sleep for most of the remaining budget.
    if deadline > now + SPIN_TAIL {
        thread::sleep(deadline - now - SPIN_TAIL);
    }
    // Fine spin for the tail — small, but the difference between 60 and 66
    // FPS on a lazy sleep is exactly this.
    while Instant::now() < deadline {
        std::hint::spin_loop();
    }

    // Absolute scheduling: if we overshot the deadline we catch up instantly
    // instead of letting the cap drift slower forever.
    let reached = Instant::now();
    *next_deadline = Some(if reached > deadline + frame_time {
        reached + frame_time
    } else {
        deadline + frame_time
    });
}
