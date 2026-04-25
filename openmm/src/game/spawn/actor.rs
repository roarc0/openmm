//! Shared actor entity spawning (monsters and NPCs).

use bevy::prelude::*;

use crate::game::InGame;
use crate::game::actors::{Actor, ActorParams, MonsterAiMode, MonsterAiType, collision_radius_from_sprite_width};
use crate::game::interaction::{MonsterInteractable, NpcInteractable};
use crate::game::sprites::{
    AnimationState, Billboard, EntityKind, WorldEntity, apply_shadow_config,
    loading::{self as sprites, SpriteSheet},
};

use super::{SpawnCtx, WorldObstacle};

/// Whether this actor is a monster or an NPC — determines entity name,
/// `EntityKind` component, and which interactable marker gets attached.
pub enum ActorKind<'a> {
    Monster,
    Npc {
        /// Name shown on hover (e.g. "Peasant" or first-name-only).
        hover_name: &'a str,
        /// NPC ID for dialogue lookup. Generated street NPCs use
        /// `GENERATED_NPC_ID_BASE + spawn_index`.
        npc_id: i16,
    },
}

/// Everything a caller needs to specify to spawn an actor (monster or NPC).
/// `ground_pos` should be ground-level in Bevy coords — `spawn_actor` adds
/// sprite half-height so the billboard sits on the ground.
pub struct ActorSpawnParams<'a> {
    pub kind: ActorKind<'a>,
    pub name: &'a str,
    pub standing_sprite: &'a str,
    pub walking_sprite: &'a str,
    pub attacking_sprite: &'a str,
    pub dying_sprite: &'a str,
    pub variant: u8,
    pub palette_id: u16,
    pub ground_pos: Vec3,
    pub hp: i16,
    pub move_speed: f32,
    pub sound_ids: [u16; 4],
    pub tether_distance: f32,
    pub attack_range: f32,
    pub aggro_range: f32,
    pub recovery_secs: f32,
    pub can_fly: bool,
    pub ai_type: &'a str,
    pub ddm_id: i32,
    pub group_id: i32,
    pub hostile: bool,
}

/// Spawn an actor entity (monster or NPC) with full sprite loading.
///
/// Returns the entity ID, or `None` if sprites fail to load.
/// If `parent` is `Some`, the entity is added as a child (outdoor terrain root).
pub fn spawn_actor(
    commands: &mut Commands,
    ctx: &mut SpawnCtx,
    params: &ActorSpawnParams,
    parent: Option<Entity>,
) -> Option<Entity> {
    let (states, state_masks, raw_w, raw_h) = sprites::load_entity_sprites(
        params.standing_sprite,
        params.walking_sprite,
        params.attacking_sprite,
        params.dying_sprite,
        ctx.game_assets.assets(),
        ctx.images,
        ctx.sprite_materials,
        &mut Some(ctx.sprite_cache),
        params.variant,
        params.palette_id,
        false,
    );
    if states.is_empty() {
        error!(
            "Actor '{}' sprite '{}' failed to load — skipping",
            params.name, params.standing_sprite
        );
        return None;
    }

    let dsft_scale = ctx.game_assets.lod().dsft_scale_for_group(params.standing_sprite);
    let (sw, sh) = (raw_w * dsft_scale, raw_h * dsft_scale);
    let pos = params.ground_pos + Vec3::new(0.0, sh / 2.0, 0.0);
    let state_count = states.len();

    let (entity_name, entity_kind) = match &params.kind {
        ActorKind::Monster => (format!("monster:{}", params.name), EntityKind::Monster),
        ActorKind::Npc { .. } => (format!("npc:{}", params.name), EntityKind::Npc),
    };

    let ent = commands
        .spawn((
            Name::new(entity_name),
            Mesh3d(ctx.meshes.add(Rectangle::new(sw, sh))),
            MeshMaterial3d(states[0][0][0].clone()),
            Transform::from_translation(pos),
            WorldEntity,
            entity_kind,
            if params.hp <= 0 { AnimationState::Dead } else { AnimationState::Idle },
            Billboard,
            SpriteSheet::new(states, vec![(sw, sh); state_count], state_masks),
            MonsterAiMode::Wander,
            WorldObstacle {
                radius: collision_radius_from_sprite_width(sw),
            },
            Actor::new(ActorParams {
                name: params.name.to_string(),
                hp: params.hp,
                move_speed: params.move_speed,
                position: pos,
                hostile: params.hostile,
                variant: params.variant,
                sound_ids: params.sound_ids,
                tether_distance: params.tether_distance,
                attack_range: params.attack_range,
                ddm_id: params.ddm_id,
                group_id: params.group_id,
                aggro_range: params.aggro_range,
                recovery_secs: params.recovery_secs,
                sprite_half_height: sh / 2.0,
                collision_radius: collision_radius_from_sprite_width(sw),
                can_fly: params.can_fly,
                ai_type: MonsterAiType::from_str(params.ai_type),
            }),
            InGame,
        ))
        .id();

    // Attach kind-specific interactable component.
    match &params.kind {
        ActorKind::Monster => {
            commands.entity(ent).insert(MonsterInteractable {
                name: params.name.to_string(),
            });
        }
        ActorKind::Npc { hover_name, npc_id } => {
            commands.entity(ent).insert(NpcInteractable {
                name: hover_name.to_string(),
                npc_id: *npc_id,
            });
        }
    }

    apply_shadow_config(commands, ent, ctx.actor_shadows);

    if let Some(parent_entity) = parent {
        commands.entity(parent_entity).add_child(ent);
    }

    Some(ent)
}
