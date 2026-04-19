//! Actor entity: NPCs and monsters.

use bevy::prelude::*;

/// Minimum horizontal collision radius used for actor movement against world geometry.
pub const MIN_ACTOR_COLLISION_RADIUS: f32 = 20.0;

/// Unified NPC/monster actor component.
#[derive(Component)]
/// AI behaviour type from monsters.txt: "Normal", "Aggress", "Wimp", "Suicidal".
/// Controls aggro probability and flee behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MonsterAiType {
    #[default]
    Normal,
    Aggress,
    Wimp,
    Suicidal,
}

impl MonsterAiType {
    pub fn from_str(s: &str) -> Self {
        match s {
            "Aggress" => Self::Aggress,
            "Wimp" => Self::Wimp,
            "Suicidal" => Self::Suicidal,
            _ => Self::Normal,
        }
    }
}

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
    /// Sound IDs: [attack, die, got_hit, fidget]. Use `ActorSoundSlot` for indexing.
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
    /// Horizontal collision radius in world units.
    ///
    /// This forms a cylinder footprint in XZ against world collision so wide
    /// sprites don't clip through BSP models with only their center line.
    pub collision_radius: f32,
    /// Whether this monster can fly. Flying actors are not terrain-snapped during movement.
    pub can_fly: bool,
    /// Vertical velocity in world units/sec. Applied by the gravity system when airborne.
    pub vertical_velocity: f32,
    /// AI behaviour type from monsters.txt.
    pub ai_type: MonsterAiType,
    /// Cached steering detour angle (radians, relative to base heading) from the last
    /// blocked frame. `None` means no cache — try the direct path fresh. When set, the
    /// steering probe tries this offset first so a monster hugging a wall doesn't need
    /// to re-probe every frame while rounding the same obstacle.
    pub cached_steer_offset: Option<f32>,
}

/// Parameters for constructing an [`Actor`] via [`Actor::new`].
/// Caller supplies only the varying fields; shared fields (timer seeds, sentinels) are
/// computed by the constructor.
pub struct ActorParams {
    pub name: String,
    pub hp: i16,
    pub move_speed: f32,
    pub position: Vec3,
    pub hostile: bool,
    pub variant: u8,
    pub sound_ids: [u16; 4],
    pub tether_distance: f32,
    pub attack_range: f32,
    pub ddm_id: i32,
    pub group_id: i32,
    pub aggro_range: f32,
    pub recovery_secs: f32,
    pub sprite_half_height: f32,
    pub collision_radius: f32,
    pub can_fly: bool,
    pub ai_type: MonsterAiType,
}

impl Actor {
    /// Construct an [`Actor`] from [`ActorParams`].
    /// Timer fields (wander_timer, fidget_timer, attack_timer) are deterministically
    /// seeded from the spawn position so nearby actors don't synchronise their behaviour.
    pub fn new(p: ActorParams) -> Self {
        let pos = p.position;
        Self {
            name: p.name,
            hp: p.hp,
            max_hp: p.hp,
            move_speed: p.move_speed,
            initial_position: pos,
            guarding_position: pos,
            tether_distance: p.tether_distance,
            wander_timer: (pos.x * 0.011 + pos.z * 0.017).abs().fract() * 4.0,
            wander_target: pos,
            facing_yaw: 0.0,
            hostile: p.hostile,
            variant: p.variant,
            sound_ids: p.sound_ids,
            fidget_timer: (pos.x * 0.013 + pos.z * 0.019).abs().fract() * 15.0 + 5.0,
            attack_range: p.attack_range,
            attack_timer: (pos.x * 0.007 + pos.z * 0.023).abs().fract() * 3.0 + 1.0,
            attack_anim_remaining: 0.0,
            ddm_id: p.ddm_id,
            group_id: p.group_id,
            aggro_range: p.aggro_range,
            recovery_secs: p.recovery_secs,
            sprite_half_height: p.sprite_half_height,
            collision_radius: p.collision_radius.max(MIN_ACTOR_COLLISION_RADIUS),
            can_fly: p.can_fly,
            vertical_velocity: 0.0,
            ai_type: p.ai_type,
            cached_steer_offset: None,
        }
    }
}
