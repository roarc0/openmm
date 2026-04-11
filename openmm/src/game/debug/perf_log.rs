//! Lightweight resource stability monitor and per-system timing.
//!
//! Enabled via `--features perf_log` or `make profile`.
//! Logs entity counts and resource stats every 2 seconds.
//! Detects entity leaks by comparing current counts against a baseline
//! captured after lazy spawn completes. Warns on unexpected growth.

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

/// Snapshot of entity counts for leak detection.
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
        app.init_resource::<PerfState>().add_systems(
            Update,
            perf_report
                .run_if(in_state(GameState::Game))
                .run_if(resource_equals(HudView::World)),
        );
        info!("[PERF] perf_log enabled — reporting every {REPORT_INTERVAL_SECS}s, leak detection after {BASELINE_AFTER_REPORTS} reports");
    }
}

fn perf_report(
    time: Res<Time>,
    mut state: ResMut<PerfState>,
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

    warn!(
        "[PERF] entities={} audio={} lights={} world={} sprites={}/{} occ_faces={occ} click_faces={click}",
        snap.total, snap.audio, snap.lights, snap.world, visible_sprites, snap.sprites
    );
}
