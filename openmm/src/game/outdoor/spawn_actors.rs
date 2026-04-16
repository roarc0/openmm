//! Per-frame spawning of NPC actors (DDM-placed) and ODM monster spawn-points.

use bevy::prelude::*;

use crate::game::spawn::SpawnCtx;
use crate::game::spawn::monster::{ActorKind, ActorSpawnParams, spawn_actor};
use crate::states::loading::PreparedWorld;

use super::lazy_spawn::PendingSpawns;

/// Spawn DDM-placed NPC actors. Some actors marked `is_monster()` are still
/// processed here (DDM monsters); ODM spawn-point monsters use the dedicated
/// [`spawn_odm_monsters`] path.
pub(super) fn spawn_npc_actors(
    commands: &mut Commands,
    ctx: &mut SpawnCtx,
    p: &mut PendingSpawns,
    prepared: &PreparedWorld,
    start: std::time::Instant,
    time_budget: f32,
    batch_max: usize,
    terrain_entity: Entity,
    actor_idx: &mut usize,
    spawned: &mut usize,
    map_events: &mut Option<ResMut<crate::game::world::MapEvents>>,
) {
    let actor_len = p.actor_order.len();
    while *actor_idx < actor_len && *spawned < batch_max && start.elapsed().as_secs_f32() * 1000.0 < time_budget {
        let i = p.actor_order[*actor_idx];
        *actor_idx += 1;
        let actor = match p.actors.as_ref().and_then(|a| a.get_actors().get(i)) {
            Some(a) => a,
            None => continue,
        };

        if actor.is_monster() {
            let ground_y = crate::game::collision::probe_ground_height(
                &prepared.map.height_map[..],
                None,
                actor.position[0] as f32,
                -(actor.position[1] as f32),
            )
            .max(actor.position[2] as f32);
            let ground_pos = Vec3::new(actor.position[0] as f32, ground_y, -(actor.position[1] as f32));

            let params = ActorSpawnParams {
                kind: ActorKind::Monster,
                name: &actor.name,
                standing_sprite: &actor.standing_sprite,
                walking_sprite: &actor.walking_sprite,
                attacking_sprite: &actor.attacking_sprite,
                dying_sprite: &actor.dying_sprite,
                variant: actor.variant,
                palette_id: actor.palette_id,
                ground_pos,
                hp: actor.hp,
                move_speed: actor.move_speed as f32,
                sound_ids: actor.sound_ids,
                tether_distance: actor.tether_distance as f32,
                attack_range: actor.radius as f32 * 2.0,
                aggro_range: actor.aggro_range,
                recovery_secs: actor.recovery_secs,
                can_fly: actor.can_fly,
                ai_type: &actor.ai_type,
                ddm_id: i as i32,
                group_id: actor.group,
                hostile: true,
            };
            if spawn_actor(commands, ctx, &params, Some(terrain_entity)).is_some() {
                *spawned += 1;
            }
            continue;
        }

        // --- NPC branch: peasant identity logic stays, entity spawn delegated ---

        let ground_y = crate::game::collision::probe_ground_height(
            &prepared.map.height_map[..],
            None,
            actor.position[0] as f32,
            -(actor.position[1] as f32),
        )
        .max(actor.position[2] as f32);
        let ground_pos = Vec3::new(actor.position[0] as f32, ground_y, -(actor.position[1] as f32));

        let (display_name, effective_npc_id) = if actor.is_peasant {
            let gid = crate::game::world::GENERATED_NPC_ID_BASE + i as i32;
            let name = map_events
                .as_deref()
                .and_then(|me| me.name_pool.as_ref())
                .map(|pool| pool.name_for(actor.is_female, i).to_string())
                .unwrap_or_else(|| actor.name.clone());
            let (portrait, prof) = map_events
                .as_deref()
                .and_then(|me| me.npc_table.as_ref())
                .and_then(|t| t.peasant_identity(actor.is_female, i))
                .unwrap_or((1, 52));
            if let Some(me) = map_events.as_mut() {
                me.generated_npcs.insert(
                    gid,
                    openmm_data::GeneratedNpc {
                        name: name.clone(),
                        portrait,
                        profession_id: prof,
                    },
                );
            }
            (name, gid)
        } else {
            (actor.name.clone(), actor.npc_id() as i32)
        };

        let hover_name = if actor.is_peasant {
            "Peasant".to_string()
        } else {
            display_name
                .split_whitespace()
                .next()
                .unwrap_or(&display_name)
                .to_string()
        };

        let params = ActorSpawnParams {
            kind: ActorKind::Npc {
                hover_name: &hover_name,
                npc_id: effective_npc_id as i16,
            },
            name: &actor.name,
            standing_sprite: &actor.standing_sprite,
            walking_sprite: &actor.walking_sprite,
            attacking_sprite: &actor.attacking_sprite,
            dying_sprite: "",
            variant: actor.variant,
            palette_id: actor.palette_id,
            ground_pos,
            hp: actor.hp,
            move_speed: actor.move_speed as f32,
            sound_ids: actor.sound_ids,
            tether_distance: actor.tether_distance as f32,
            attack_range: actor.radius as f32 * 2.0,
            aggro_range: actor.aggro_range,
            recovery_secs: actor.recovery_secs,
            can_fly: actor.can_fly,
            ai_type: &actor.ai_type,
            ddm_id: i as i32,
            group_id: actor.group,
            hostile: false,
        };
        if spawn_actor(commands, ctx, &params, Some(terrain_entity)).is_some() {
            *spawned += 1;
        }
    }
}

/// Spawn ODM monster spawn-point entries. Each entry is one group member with
/// position fanned out around the spawn-point centre.
pub(super) fn spawn_odm_monsters(
    commands: &mut Commands,
    ctx: &mut SpawnCtx,
    p: &mut PendingSpawns,
    prepared: &PreparedWorld,
    start: std::time::Instant,
    time_budget: f32,
    batch_max: usize,
    terrain_entity: Entity,
    monster_idx: &mut usize,
    spawned: &mut usize,
) {
    let monster_len = p.monster_order.len();
    while *monster_idx < monster_len && *spawned < batch_max && start.elapsed().as_secs_f32() * 1000.0 < time_budget {
        let i = p.monster_order[*monster_idx];
        *monster_idx += 1;
        let mon = match p.monsters.as_ref().and_then(|m| m.entries().get(i)) {
            Some(m) => m,
            None => continue,
        };

        let angle = mon.group_index as f32 * 2.399_f32;
        let (wx, wz) = (
            mon.spawn_position[0] as f32 + mon.spawn_radius as f32 * angle.cos(),
            -(mon.spawn_position[1] as f32 + mon.spawn_radius as f32 * angle.sin()),
        );
        let ground_y = crate::game::collision::probe_ground_height(&prepared.map.height_map[..], None, wx, wz);
        let ground_pos = Vec3::new(wx, ground_y, wz);

        let params = ActorSpawnParams {
            kind: ActorKind::Monster,
            name: &mon.name,
            standing_sprite: &mon.standing_sprite,
            walking_sprite: &mon.walking_sprite,
            attacking_sprite: &mon.attacking_sprite,
            dying_sprite: &mon.dying_sprite,
            variant: mon.variant,
            palette_id: mon.palette_id,
            ground_pos,
            hp: mon.hp,
            move_speed: mon.move_speed as f32,
            sound_ids: mon.sound_ids,
            tether_distance: mon.radius as f32 * 2.0,
            attack_range: mon.body_radius as f32 * 2.0,
            aggro_range: mon.aggro_range,
            recovery_secs: mon.recovery_secs,
            can_fly: mon.can_fly,
            ai_type: &mon.ai_type,
            ddm_id: -1,
            group_id: 0,
            hostile: true,
        };
        if spawn_actor(commands, ctx, &params, Some(terrain_entity)).is_some() {
            *spawned += 1;
        }
    }
}
