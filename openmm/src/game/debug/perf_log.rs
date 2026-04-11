//! Lightweight resource stability monitor and per-system timing.
//!
//! Enabled via `--features perf_log` or `make profile`.
//! Logs entity counts and resource stats every 2 seconds.
//! Detects entity leaks by comparing current counts against a baseline
//! captured after lazy spawn completes. Warns on unexpected growth.
//!
//! Also tracks per-frame counters from hot systems (sprite updates,
//! spatial index, interaction raycasts) and reports averages alongside
//! the entity snapshot. Systems increment counters on the shared
//! `PerfCounters` resource; the reporter resets them each interval.

use std::time::Instant;

use bevy::prelude::*;

use crate::GameState;
use crate::game::hud_view::HudView;
use crate::game::indoor::OccluderFaces;
use crate::game::interaction::clickable;
use crate::game::sprites::WorldEntity;

const REPORT_INTERVAL_SECS: f32 = 2.0;

/// How many reports to skip before capturing the baseline (lets lazy spawn finish).
const BASELINE_AFTER_REPORTS: u32 = 5;

/// Entity count growth above baseline that triggers a warning.
const LEAK_THRESHOLD: i32 = 20;

// ── Per-frame counters ──────────────────────────────────────────────────────

/// Shared resource where hot systems log counters each frame.
/// Reset every report interval; the reporter reads accumulated totals
/// and divides by frame count for averages.
#[derive(Resource, Default)]
pub struct PerfCounters {
    pub frames: u32,

    // Sprite system
    /// SpriteSheet entities iterated (before distance cull).
    pub sprite_iter: u32,
    /// Sprites that passed the visibility + distance check.
    pub sprite_visible: u32,
    /// Material handle swaps (frame/direction changed).
    pub sprite_mat_swaps: u32,
    /// Transform.rotation writes (billboard facing changed).
    pub sprite_rot_writes: u32,
    /// Transform.scale.x flips (mirror changed).
    pub sprite_scale_writes: u32,

    // Billboard face-camera system
    pub billboard_iter: u32,
    pub billboard_rot_writes: u32,

    // Spatial index rebuild
    pub spatial_entities: u32,
    pub spatial_vis_changes: u32,

    // Hover hint system
    pub hover_candidates: u32,
    pub hover_billboard_tests: u32,
    pub hover_face_tests: u32,

    // Flicker system
    pub flicker_iter: u32,
    pub flicker_writes: u32,

    // AI system
    pub ai_iter: u32,
    pub ai_steer_calls: u32,

    // System wall-clock times (microseconds, accumulated over interval)
    pub time_sprite_update_us: u64,
    pub time_billboard_face_us: u64,
    pub time_spatial_rebuild_us: u64,
    pub time_hover_hint_us: u64,
    pub time_flicker_us: u64,
    pub time_ai_us: u64,
}

impl PerfCounters {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Convenience: start a timer, return an `Instant`.
#[inline]
pub fn perf_start() -> Instant {
    Instant::now()
}

/// Convenience: elapsed microseconds since `start`.
#[inline]
pub fn perf_elapsed_us(start: Instant) -> u64 {
    start.elapsed().as_micros() as u64
}

// ── Entity snapshot (unchanged from before) ───────────────────────��────────

#[derive(Default, Clone)]
struct EntitySnapshot {
    total: usize,
    audio: usize,
    lights: usize,
    world: usize,
    sprites: usize,
}

impl EntitySnapshot {
    fn diff_warn(&self, baseline: &EntitySnapshot) -> Option<String> {
        let mut warnings = Vec::new();
        let check = |name: &str, cur: usize, base: usize| -> Option<String> {
            let delta = cur as i32 - base as i32;
            if delta > LEAK_THRESHOLD {
                Some(format!("{name} +{delta} ({base}->{cur})"))
            } else {
                None
            }
        };
        if let Some(w) = check("audio", self.audio, baseline.audio) { warnings.push(w); }
        if let Some(w) = check("lights", self.lights, baseline.lights) { warnings.push(w); }
        if let Some(w) = check("total", self.total, baseline.total) { warnings.push(w); }
        if let Some(w) = check("world", self.world, baseline.world) { warnings.push(w); }
        if let Some(w) = check("sprites", self.sprites, baseline.sprites) { warnings.push(w); }
        if warnings.is_empty() { None } else { Some(warnings.join(", ")) }
    }
}

#[derive(Resource, Default)]
struct PerfState {
    timer: f32,
    report_count: u32,
    baseline: Option<EntitySnapshot>,
}

pub struct PerfLogPlugin;

impl Plugin for PerfLogPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PerfState>()
            .init_resource::<PerfCounters>()
            .add_systems(
                Update,
                (count_frame, perf_report.after(count_frame))
                    .run_if(in_state(GameState::Game))
                    .run_if(resource_equals(HudView::World)),
            );
        info!("[PERF] perf_log enabled — reporting every {REPORT_INTERVAL_SECS}s, leak detection after {BASELINE_AFTER_REPORTS} reports");
    }
}

/// Bump frame counter each frame (runs before report).
fn count_frame(mut counters: ResMut<PerfCounters>) {
    counters.frames += 1;
}

fn perf_report(
    time: Res<Time>,
    mut state: ResMut<PerfState>,
    mut counters: ResMut<PerfCounters>,
    audio_q: Query<Entity, With<AudioPlayer<AudioSource>>>,
    point_lights: Query<Entity, With<PointLight>>,
    world_entities: Query<Entity, With<WorldEntity>>,
    sprites: Query<&Visibility, With<crate::game::sprites::loading::SpriteSheet>>,
    occluder_faces: Option<Res<OccluderFaces>>,
    clickable_faces: Option<Res<clickable::Faces>>,
    all_entities: Query<Entity>,
) {
    state.timer += time.delta_secs();
    if state.timer < REPORT_INTERVAL_SECS {
        return;
    }
    state.timer = 0.0;
    state.report_count += 1;

    let snap = EntitySnapshot {
        total: all_entities.iter().count(),
        audio: audio_q.iter().count(),
        lights: point_lights.iter().count(),
        world: world_entities.iter().count(),
        sprites: sprites.iter().count(),
    };

    let visible_sprites = sprites.iter().filter(|v| **v != Visibility::Hidden).count();
    let occ = occluder_faces.as_ref().map(|o| o.faces.len()).unwrap_or(0);
    let click = clickable_faces.as_ref().map(|c| c.faces.len()).unwrap_or(0);

    // Capture baseline after lazy spawn finishes.
    if state.baseline.is_none() && state.report_count >= BASELINE_AFTER_REPORTS {
        info!("[PERF] baseline captured: entities={} audio={} lights={} world={} sprites={}",
            snap.total, snap.audio, snap.lights, snap.world, snap.sprites);
        state.baseline = Some(snap.clone());
    }

    // Leak detection.
    let leak_msg = state.baseline.as_ref()
        .and_then(|b| snap.diff_warn(b))
        .unwrap_or_default();

    if !leak_msg.is_empty() {
        error!("[PERF] LEAK DETECTED: {leak_msg}");
    }

    // Per-frame averages from counters.
    let f = counters.frames.max(1) as f32;
    let avg = |v: u32| v as f32 / f;
    let avg_us = |v: u64| v as f32 / f;

    warn!(
        "[PERF] entities={} audio={} lights={} world={} sprites={}/{} occ_faces={occ} click_faces={click}",
        snap.total, snap.audio, snap.lights, snap.world, visible_sprites, snap.sprites
    );

    // System timing (average per frame in microseconds)
    warn!(
        "[PERF] timing(avg us/frame): sprite_update={:.0} billboard_face={:.0} spatial={:.0} hover={:.0} flicker={:.0} ai={:.0}",
        avg_us(counters.time_sprite_update_us),
        avg_us(counters.time_billboard_face_us),
        avg_us(counters.time_spatial_rebuild_us),
        avg_us(counters.time_hover_hint_us),
        avg_us(counters.time_flicker_us),
        avg_us(counters.time_ai_us),
    );

    // Sprite update details
    warn!(
        "[PERF] sprites(avg/frame): iter={:.0} visible={:.0} mat_swaps={:.1} rot_writes={:.1} scale_flips={:.1}",
        avg(counters.sprite_iter),
        avg(counters.sprite_visible),
        avg(counters.sprite_mat_swaps),
        avg(counters.sprite_rot_writes),
        avg(counters.sprite_scale_writes),
    );

    // Billboard + spatial + hover details
    warn!(
        "[PERF] billboard(avg/frame): iter={:.0} rot_writes={:.1} | spatial: entities={:.0} vis_changes={:.1} | hover: candidates={:.0} hit_tests={:.1} face_tests={:.0}",
        avg(counters.billboard_iter),
        avg(counters.billboard_rot_writes),
        avg(counters.spatial_entities),
        avg(counters.spatial_vis_changes),
        avg(counters.hover_candidates),
        avg(counters.hover_billboard_tests),
        avg(counters.hover_face_tests),
    );

    // AI details
    if counters.ai_iter > 0 {
        warn!(
            "[PERF] ai(avg/frame): iter={:.0} steer_calls={:.1}",
            avg(counters.ai_iter),
            avg(counters.ai_steer_calls),
        );
    }

    // Flicker details
    if counters.flicker_iter > 0 {
        warn!(
            "[PERF] flicker(avg/frame): iter={:.0} writes={:.1}",
            avg(counters.flicker_iter),
            avg(counters.flicker_writes),
        );
    }

    counters.reset();
}
