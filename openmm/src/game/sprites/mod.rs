use bevy::prelude::*;

use crate::GameState;
use crate::game::spatial_index::SpatialIndexSet;
use crate::game::state::ui_state::{UiMode, UiState};

pub mod loading;
pub mod material;
pub mod tint_buffer;

// Future modules:
// pub mod loot;

// --- Shared components for all world entities ---

/// All world entities that are spawned from map data.
#[derive(Component)]
pub struct WorldEntity;

/// What kind of world entity this is.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub enum EntityKind {
    /// Static decoration: trees, rocks, fountains, etc. Single sprite, no behavior.
    Decoration,
    /// Interactive NPC: has dialogue, directional sprites, idle animations.
    Npc,
    /// Monster: directional sprites, multiple animation states, can be killed and looted.
    Monster,
    /// Item on the ground: can be picked up.
    Loot,
}

/// Billboard rendering: entity always faces the camera.
#[derive(Component)]
pub struct Billboard;

/// Fixed facing direction in world space (radians, Y-axis rotation).
/// Used by directional decorations (e.g. ships) whose displayed sprite depends
/// on the camera angle relative to this facing. Actor entities use Actor.facing_yaw instead.
#[derive(Component)]
pub struct FacingYaw(pub f32);

/// Animation state for entities that have multiple frames.
/// Not implemented yet — placeholder for the animation system.
#[derive(Component, Debug, Clone, PartialEq, Eq, Default)]
pub enum AnimationState {
    #[default]
    Idle,
    Walking,
    Attacking,
    GettingHit,
    Dying,
    Dead,
}

/// Loot container: when a monster dies, this component is added so it can be looted.
/// Not implemented yet.
#[derive(Component)]
pub struct Lootable;

/// Marks a billboard sprite that is itself a light source (torch, campfire, brazier, etc.).
/// The lighting tint system skips these entities so the fire/flame texture stays at full
/// brightness regardless of time of day or dungeon ambient.
#[derive(Component)]
pub struct SelfLit;

/// Visibility flicker for torches, candles, and similar decorations.
/// Toggles Visibility at a fixed rate; runs after distance_culling so out-of-range
/// entities stay hidden even when "lit".
///
/// Stateless: lit/unlit is computed from `Time::elapsed_secs()` + a per-entity
/// phase, so `flicker_system` never writes the component. Only `Visibility` is
/// mutated, and only when a torch is currently unlit.
#[derive(Component)]
pub struct DecorFlicker {
    /// Toggles per second.
    pub rate: f32,
    /// Phase offset (in toggle units, not seconds) so nearby torches don't
    /// flicker in sync. Kept in 0..1.
    phase: f32,
}

impl DecorFlicker {
    pub fn new(rate: f32, phase_offset: f32) -> Self {
        Self {
            rate,
            phase: phase_offset.rem_euclid(1.0),
        }
    }

    /// Deterministic lit state at a given elapsed time.
    fn lit(&self, elapsed: f32) -> bool {
        if self.rate <= 0.0 {
            return true;
        }
        // Toggle count since t=0, offset by the per-entity phase. Even → lit.
        let toggles = (elapsed * self.rate + self.phase).floor() as i64;
        toggles & 1 == 0
    }
}

// --- Helpers ---

/// Apply standard billboard shadow settings: never receive shadows,
/// optionally cast shadows based on config flag. Replaces 12+ identical blocks.
pub fn apply_shadow_config(commands: &mut Commands, entity: Entity, cast_shadows: bool) {
    commands.entity(entity).insert(bevy::light::NotShadowReceiver);
    if !cast_shadows {
        commands.entity(entity).insert(bevy::light::NotShadowCaster);
    }
}

// --- Plugin ---

pub struct SpritesPlugin;

impl Plugin for SpritesPlugin {
    fn build(&self, app: &mut App) {
        // Distance culling now lives in `spatial_index::rebuild_and_cull` so
        // the grid build and the per-entity visibility write share a single
        // iteration. Everything here runs after that set.
        app.add_systems(
            Update,
            (flicker_system, loading::update_sprite_sheets, billboard_face_camera)
                .chain()
                .after(SpatialIndexSet)
                .run_if(in_state(GameState::Game))
                .run_if(|ui: Res<UiState>| ui.mode == UiMode::World),
        );
    }
}

/// Toggle visibility for flickering decorations (torches, candles, etc.).
/// Runs after `SpatialIndexSet` (which performs distance culling): when
/// unlit it forces Hidden; when lit it leaves whatever the cull set, so
/// out-of-range entities stay hidden.
fn flicker_system(
    time: Res<Time>,
    mut query: Query<(&DecorFlicker, &mut Visibility)>,
    #[cfg(feature = "perf_log")] mut perf: ResMut<crate::screens::debug::perf_log::PerfCounters>,
) {
    #[cfg(feature = "perf_log")]
    let _start = crate::screens::debug::perf_log::perf_start();

    let elapsed = time.elapsed_secs();
    for (flicker, mut vis) in query.iter_mut() {
        #[cfg(feature = "perf_log")]
        {
            perf.flicker_iter += 1;
        }
        if !flicker.lit(elapsed) {
            if *vis != Visibility::Hidden {
                #[cfg(feature = "perf_log")]
                {
                    perf.flicker_writes += 1;
                }
            }
            vis.set_if_neq(Visibility::Hidden);
        }
    }

    #[cfg(feature = "perf_log")]
    {
        perf.time_flicker_us += crate::screens::debug::perf_log::perf_elapsed_us(_start);
    }
}

/// Rotate visible billboard entities to face the camera (Y-axis only, stays upright).
/// Only processes visible entities — distance_culling already hides far ones.
/// Skips entities with SpriteSheet (those are handled by update_sprite_sheets).
fn billboard_face_camera(
    camera_query: Query<&GlobalTransform, With<crate::game::player::PlayerCamera>>,
    mut billboard_query: Query<
        (&mut Transform, &GlobalTransform, &Visibility),
        (With<Billboard>, Without<loading::SpriteSheet>),
    >,
    #[cfg(feature = "perf_log")] mut perf: ResMut<crate::screens::debug::perf_log::PerfCounters>,
) {
    #[cfg(feature = "perf_log")]
    let _start = crate::screens::debug::perf_log::perf_start();

    let Ok(camera_gt) = camera_query.single() else {
        return;
    };
    let cam_pos = camera_gt.translation();

    for (mut transform, global_transform, vis) in billboard_query.iter_mut() {
        #[cfg(feature = "perf_log")]
        {
            perf.billboard_iter += 1;
        }
        if *vis == Visibility::Hidden {
            continue;
        }
        let dir = cam_pos - global_transform.translation();
        if dir.x.abs() > 0.01 || dir.z.abs() > 0.01 {
            // Quantize to ~1.4° steps (128 bins per full turn) so tiny camera
            // movements don't dirty every billboard's Transform every frame.
            // Sprites face the camera — sub-degree precision is invisible.
            let raw = dir.x.atan2(dir.z);
            let quantized = (raw * 128.0 / std::f32::consts::TAU).round() * std::f32::consts::TAU / 128.0;
            let new_rot = Quat::from_rotation_y(quantized);
            if transform.rotation != new_rot {
                transform.rotation = new_rot;
                #[cfg(feature = "perf_log")]
                {
                    perf.billboard_rot_writes += 1;
                }
            }
        }
    }

    #[cfg(feature = "perf_log")]
    {
        perf.time_billboard_face_us += crate::screens::debug::perf_log::perf_elapsed_us(_start);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flicker_starts_lit_with_zero_phase() {
        let f = DecorFlicker::new(2.0, 0.0);
        assert!(f.lit(0.0));
    }

    #[test]
    fn flicker_toggles_at_rate() {
        // rate=2 ⇒ toggle every 0.5s. Starting lit at t=0.
        let f = DecorFlicker::new(2.0, 0.0);
        assert!(f.lit(0.0));
        assert!(f.lit(0.25));
        assert!(!f.lit(0.5));
        assert!(!f.lit(0.75));
        assert!(f.lit(1.0));
        assert!(f.lit(1.25));
        assert!(!f.lit(1.5));
    }

    #[test]
    fn flicker_phase_offsets_toggle_timing() {
        // rate=2, phase=0.5 ⇒ first toggle happens 0.25s earlier than with
        // phase=0 (since 0.25*2 + 0.5 == 1.0). Ensures neighbouring torches
        // with different phases don't flicker in lock-step.
        let zero = DecorFlicker::new(2.0, 0.0);
        let half = DecorFlicker::new(2.0, 0.5);
        assert!(zero.lit(0.0));
        assert!(half.lit(0.0));
        // t=0.25 — zero is still lit, half has just toggled off.
        assert!(zero.lit(0.25));
        assert!(!half.lit(0.25));
        // t=0.5 — zero just toggled off, half still off (2nd half of its dark period).
        assert!(!zero.lit(0.5));
        assert!(!half.lit(0.5));
        // t=0.75 — zero still off, half toggled back on.
        assert!(!zero.lit(0.75));
        assert!(half.lit(0.75));
    }

    #[test]
    fn flicker_phase_is_wrapped_to_unit_interval() {
        // phase_offset > 1.0 should wrap.
        let a = DecorFlicker::new(2.0, 0.25);
        let b = DecorFlicker::new(2.0, 1.25);
        let c = DecorFlicker::new(2.0, -0.75);
        for t in [0.0, 0.1, 0.3, 1.7] {
            assert_eq!(a.lit(t), b.lit(t));
            assert_eq!(a.lit(t), c.lit(t));
        }
    }

    #[test]
    fn flicker_rate_zero_is_always_lit() {
        let f = DecorFlicker::new(0.0, 0.123);
        assert!(f.lit(0.0));
        assert!(f.lit(1.0));
        assert!(f.lit(1e6));
    }

    #[test]
    fn flicker_is_stateless_pure() {
        // Same inputs must always return the same output — no timer drift.
        let f = DecorFlicker::new(3.5, 0.2);
        for _ in 0..100 {
            assert_eq!(f.lit(0.7), f.lit(0.7));
            assert_eq!(f.lit(42.123), f.lit(42.123));
        }
    }
}
