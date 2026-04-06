use bevy::prelude::*;

use crate::GameState;
use crate::game::hud::HudView;

pub mod actor;
pub mod sprites;

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
#[derive(Component)]
pub struct DecorFlicker {
    /// Toggles per second.
    pub rate: f32,
    timer: f32,
    lit: bool,
}

impl DecorFlicker {
    pub fn new(rate: f32, phase_offset: f32) -> Self {
        // Start with a random phase so nearby torches don't flicker in sync.
        let period = if rate > 0.0 { 1.0 / rate } else { 1.0 };
        Self {
            rate,
            timer: phase_offset * period,
            lit: true,
        }
    }
}

// --- Plugin ---

pub struct EntitiesPlugin;

impl Plugin for EntitiesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                distance_culling,
                flicker_system,
                sprites::update_sprite_sheets,
                billboard_face_camera,
            )
                .chain()
                .run_if(in_state(GameState::Game))
                .run_if(resource_equals(HudView::World)),
        );
    }
}

/// Hide entities that are far from the player, show ones that are close.
fn distance_culling(
    cfg: Res<crate::config::GameConfig>,
    player_query: Query<&GlobalTransform, With<crate::game::player::Player>>,
    mut entity_query: Query<(&GlobalTransform, &mut Visibility), With<WorldEntity>>,
) {
    let Ok(player_gt) = player_query.single() else {
        return;
    };
    let player_pos = player_gt.translation();
    let draw_dist_sq = cfg.draw_distance * cfg.draw_distance;

    for (entity_gt, mut vis) in entity_query.iter_mut() {
        let dist_sq = player_pos.distance_squared(entity_gt.translation());
        let new_vis = if dist_sq < draw_dist_sq {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != new_vis {
            *vis = new_vis;
        }
    }
}

/// Toggle visibility for flickering decorations (torches, candles, etc.).
/// Runs after distance_culling: when unlit it forces Hidden; when lit it leaves
/// whatever distance_culling set, so out-of-range entities stay hidden.
fn flicker_system(time: Res<Time>, mut query: Query<(&mut DecorFlicker, &mut Visibility)>) {
    let dt = time.delta_secs();
    for (mut flicker, mut vis) in query.iter_mut() {
        flicker.timer += dt;
        let period = 1.0 / flicker.rate;
        while flicker.timer >= period {
            flicker.timer -= period;
            flicker.lit = !flicker.lit;
        }
        if !flicker.lit {
            *vis = Visibility::Hidden;
        }
        // When lit, distance_culling result stands — no change needed.
    }
}

/// Rotate visible billboard entities to face the camera (Y-axis only, stays upright).
/// Only processes visible entities — distance_culling already hides far ones.
/// Skips entities with SpriteSheet (those are handled by update_sprite_sheets).
fn billboard_face_camera(
    camera_query: Query<&GlobalTransform, With<crate::game::player::PlayerCamera>>,
    mut billboard_query: Query<
        (&mut Transform, &GlobalTransform, &Visibility),
        (With<Billboard>, Without<sprites::SpriteSheet>),
    >,
) {
    let Ok(camera_gt) = camera_query.single() else {
        return;
    };
    let cam_pos = camera_gt.translation();

    for (mut transform, global_transform, vis) in billboard_query.iter_mut() {
        if *vis == Visibility::Hidden {
            continue;
        }
        let dir = cam_pos - global_transform.translation();
        if dir.x.abs() > 0.01 || dir.z.abs() > 0.01 {
            let new_rot = Quat::from_rotation_y(dir.x.atan2(dir.z));
            // Only write if rotation actually changed — writing Transform marks it changed,
            // triggering GlobalTransform propagation for every billboard every frame.
            if transform.rotation != new_rot {
                transform.rotation = new_rot;
            }
        }
    }
}
