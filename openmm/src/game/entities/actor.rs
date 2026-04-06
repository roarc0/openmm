//! Actor entity: NPCs and monsters.

use bevy::prelude::*;

/// Unified NPC/monster actor component.
#[derive(Component)]
pub struct Actor {
    pub name: String,
    pub hp: i16,
    pub max_hp: i16,
    pub move_speed: f32,
    pub initial_position: Vec3,
    pub guarding_position: Vec3,
    pub tether_distance: f32,
    pub wander_timer: f32,
    pub wander_target: Vec3,
    pub facing_yaw: f32,
    pub hostile: bool,
    /// A/B/C difficulty variant: 1=A (base), 2=B, 3=C.
    pub variant: u8,
    /// Sound IDs: [attack, die, got_hit, fidget]. Zero = no sound for that slot.
    pub sound_ids: [u16; 4],
    /// Seconds until next fidget sound attempt.
    pub fidget_timer: f32,
    /// Melee attack reach in Bevy world units. 0 = no melee attack.
    pub attack_range: f32,
    /// Seconds until this actor's next attack attempt.
    pub attack_timer: f32,
    /// Seconds remaining in the current attack animation. 0 = not attacking.
    pub attack_anim_remaining: f32,
    /// Index in the DDM actors array (0-based). -1 for non-DDM actors (ODM spawn groups).
    /// Used by ToggleActorFlag to target a specific actor.
    pub ddm_id: i32,
    /// Faction group ID from DDM (0 = none). Used by ToggleActorGroupFlag / ChangeGroup.
    pub group_id: i32,
    /// Aggro detection radius in world units. 0 = passive (won't aggro).
    pub aggro_range: f32,
    /// Attack recovery in seconds. Minimum time between attacks.
    pub recovery_secs: f32,
    /// Half the sprite quad height in world units. Used to snap Y to terrain surface.
    pub sprite_half_height: f32,
    /// Whether this monster can fly. Flying actors are not terrain-snapped during movement.
    pub can_fly: bool,
    /// Vertical velocity in world units/sec. Applied by the gravity system when airborne.
    pub vertical_velocity: f32,
    /// AI behaviour type from monsters.txt: "Normal", "Aggress", "Wimp", "Suicidal".
    /// Controls aggro probability and flee behaviour.
    pub ai_type: String,
}
