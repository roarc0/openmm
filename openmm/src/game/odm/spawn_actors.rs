//! Per-frame spawning of NPC actors (DDM-placed) and ODM monster spawn-points.

use bevy::prelude::*;

use crate::game::InGame;
use crate::game::entities::{actor, sprites};
use crate::states::loading::PreparedWorld;

use super::lazy_spawn::{PendingSpawns, SpawnCtx};

/// Spawn DDM-placed NPC actors. Some actors marked `is_monster()` are still
/// processed here (DDM monsters); ODM spawn-point monsters use the dedicated
/// [`spawn_odm_monsters`] path.
pub(super) fn spawn_npc_actors(
    commands: &mut Commands,
    ctx: &mut SpawnCtx,
    p: &mut PendingSpawns,
    prepared: &PreparedWorld,
    actor_idx: &mut usize,
    spawned: &mut usize,
    map_events: &mut Option<ResMut<crate::game::events::MapEvents>>,
) {
    let lod = ctx.game_assets.lod();
    let actor_len = p.actor_order.len();
    while *actor_idx < actor_len
        && *spawned < ctx.batch_max
        && ctx.start.elapsed().as_secs_f32() * 1000.0 < ctx.time_budget
    {
        let i = p.actor_order[*actor_idx];
        *actor_idx += 1;
        let actor = match p.actors.as_ref().and_then(|a| a.get_actors().get(i)) {
            Some(a) => a,
            None => continue,
        };

        if actor.is_monster() {
            let (states, state_masks, raw_w, raw_h) = if let Some(materials) = ctx.sprite_materials.as_deref_mut() {
                sprites::load_entity_sprites(
                    &actor.standing_sprite,
                    &actor.walking_sprite,
                    &actor.attacking_sprite,
                    &actor.dying_sprite,
                    ctx.game_assets.assets(),
                    ctx.images,
                    materials,
                    &mut Some(&mut p.sprite_cache),
                    actor.variant,
                    actor.palette_id,
                )
            } else {
                (Vec::new(), Vec::new(), 0.0, 0.0)
            };
            if states.is_empty() {
                continue;
            }
            if let Some(materials) = ctx.sprite_materials.as_deref_mut() {
                for state in &states {
                    for frame in state {
                        for handle in frame {
                            if let Some(m) = materials.get_mut(handle.id()) {
                                m.extension.tint = ctx.spawn_tint;
                            }
                        }
                    }
                }
            }
            let dsft_scale = lod.dsft_scale_for_group(&actor.standing_sprite);
            let (sw, sh) = (raw_w * dsft_scale, raw_h * dsft_scale);
            let pos = Vec3::new(
                actor.position[0] as f32,
                crate::game::collision::probe_ground_height(
                    &prepared.map.height_map[..],
                    None,
                    actor.position[0] as f32,
                    -(actor.position[1] as f32),
                )
                .max(actor.position[2] as f32)
                    + sh / 2.0,
                -(actor.position[1] as f32),
            );
            let state_count = states.len();
            let mut child = commands.spawn_empty();
            let child_id = child.id();
            child
                .insert(Name::new(format!("monster:{}", actor.name)))
                .insert(Mesh3d(ctx.meshes.add(Rectangle::new(sw, sh))))
                .insert(MeshMaterial3d(states[0][0][0].clone()))
                .insert(Transform::from_translation(pos))
                .insert(crate::game::entities::WorldEntity)
                .insert(crate::game::entities::EntityKind::Monster)
                .insert(crate::game::entities::AnimationState::Idle)
                .insert(sprites::SpriteSheet::new(
                    states,
                    vec![(sw, sh); state_count],
                    state_masks,
                ))
                .insert(crate::game::interaction::MonsterInteractable {
                    name: actor.name.clone(),
                })
                .insert(crate::game::monster_ai::MonsterAiMode::Wander)
                .insert(actor::Actor {
                    name: actor.name.clone(),
                    hp: actor.hp,
                    max_hp: actor.hp,
                    move_speed: actor.move_speed as f32,
                    initial_position: pos,
                    guarding_position: pos,
                    tether_distance: actor.tether_distance as f32,
                    wander_timer: (pos.x * 0.011 + pos.z * 0.017).abs().fract() * 4.0,
                    wander_target: pos,
                    facing_yaw: 0.0,
                    hostile: true,
                    variant: actor.variant,
                    sound_ids: actor.sound_ids,
                    fidget_timer: (pos.x * 0.013 + pos.z * 0.019).abs().fract() * 15.0 + 5.0,
                    attack_range: actor.radius as f32 * 2.0,
                    attack_timer: (pos.x * 0.007 + pos.z * 0.023).abs().fract() * 3.0 + 1.0,
                    attack_anim_remaining: 0.0,
                    ddm_id: i as i32,
                    group_id: actor.group,
                    aggro_range: actor.aggro_range,
                    recovery_secs: actor.recovery_secs,
                    sprite_half_height: sh / 2.0,
                    can_fly: actor.can_fly,
                    vertical_velocity: 0.0,
                    ai_type: actor.ai_type.clone(),
                })
                .insert(InGame);
            commands.entity(ctx.terrain_entity).add_child(child_id);
            *spawned += 1;
            continue;
        }

        let (s2, m2, w2, h2) = if let Some(materials) = ctx.sprite_materials.as_deref_mut() {
            sprites::load_entity_sprites(
                &actor.standing_sprite,
                &actor.walking_sprite,
                &actor.attacking_sprite,
                "",
                ctx.game_assets.assets(),
                ctx.images,
                materials,
                &mut Some(&mut p.sprite_cache),
                actor.variant,
                actor.palette_id,
            )
        } else {
            (Vec::new(), Vec::new(), 0.0, 0.0)
        };
        if s2.is_empty() {
            continue;
        }
        if let Some(materials) = ctx.sprite_materials.as_deref_mut() {
            for state in &s2 {
                for frame in state {
                    for handle in frame {
                        if let Some(m) = materials.get_mut(handle.id()) {
                            m.extension.tint = ctx.spawn_tint;
                        }
                    }
                }
            }
        }
        let dsft_scale = lod.dsft_scale_for_group(&actor.standing_sprite);
        let (sw, sh) = (w2 * dsft_scale, h2 * dsft_scale);
        let pos = Vec3::new(
            actor.position[0] as f32,
            crate::game::collision::probe_ground_height(
                &prepared.map.height_map[..],
                None,
                actor.position[0] as f32,
                -(actor.position[1] as f32),
            )
            .max(actor.position[2] as f32)
                + sh / 2.0,
            -(actor.position[1] as f32),
        );
        let (display_name, effective_npc_id) = if actor.is_peasant {
            let gid = crate::game::events::GENERATED_NPC_ID_BASE + i as i32;
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
        let state_count = s2.len();
        let mut child = commands.spawn_empty();
        let child_id = child.id();
        child
            .insert(Name::new(format!("npc:{}", actor.name)))
            .insert(Mesh3d(ctx.meshes.add(Rectangle::new(sw, sh))))
            .insert(MeshMaterial3d(s2[0][0][0].clone()))
            .insert(Transform::from_translation(pos))
            .insert(crate::game::entities::WorldEntity)
            .insert(crate::game::entities::EntityKind::Npc)
            .insert(crate::game::entities::AnimationState::Idle)
            .insert(sprites::SpriteSheet::new(s2, vec![(sw, sh); state_count], m2))
            .insert(crate::game::interaction::NpcInteractable {
                name: hover_name,
                npc_id: effective_npc_id as i16,
            })
            .insert(crate::game::monster_ai::MonsterAiMode::Wander)
            .insert(actor::Actor {
                name: actor.name.clone(),
                hp: actor.hp,
                max_hp: actor.hp,
                move_speed: actor.move_speed as f32,
                initial_position: pos,
                guarding_position: pos,
                tether_distance: actor.tether_distance as f32,
                wander_timer: (pos.x * 0.011 + pos.z * 0.017).abs().fract() * 4.0,
                wander_target: pos,
                facing_yaw: 0.0,
                hostile: false,
                variant: actor.variant,
                sound_ids: actor.sound_ids,
                fidget_timer: (pos.x * 0.013 + pos.z * 0.019).abs().fract() * 15.0 + 5.0,
                attack_range: actor.radius as f32 * 2.0,
                attack_timer: (pos.x * 0.007 + pos.z * 0.023).abs().fract() * 3.0 + 1.0,
                attack_anim_remaining: 0.0,
                ddm_id: i as i32,
                group_id: actor.group,
                aggro_range: actor.aggro_range,
                recovery_secs: actor.recovery_secs,
                sprite_half_height: sh / 2.0,
                can_fly: actor.can_fly,
                vertical_velocity: 0.0,
                ai_type: actor.ai_type.clone(),
            })
            .insert(InGame);
        commands.entity(ctx.terrain_entity).add_child(child_id);
        *spawned += 1;
    }
}

/// Spawn ODM monster spawn-point entries. Each entry is one group member with
/// position fanned out around the spawn-point centre.
pub(super) fn spawn_odm_monsters(
    commands: &mut Commands,
    ctx: &mut SpawnCtx,
    p: &mut PendingSpawns,
    prepared: &PreparedWorld,
    monster_idx: &mut usize,
    spawned: &mut usize,
) {
    let monster_len = p.monster_order.len();
    while *monster_idx < monster_len
        && *spawned < ctx.batch_max
        && ctx.start.elapsed().as_secs_f32() * 1000.0 < ctx.time_budget
    {
        let i = p.monster_order[*monster_idx];
        *monster_idx += 1;
        let mon = match p.monsters.as_ref().and_then(|m| m.entries().get(i)) {
            Some(m) => m,
            None => continue,
        };

        let (states, state_masks, raw_w, raw_h) = if let Some(materials) = ctx.sprite_materials.as_deref_mut() {
            sprites::load_entity_sprites(
                &mon.standing_sprite,
                &mon.walking_sprite,
                &mon.attacking_sprite,
                &mon.dying_sprite,
                ctx.game_assets.assets(),
                ctx.images,
                materials,
                &mut Some(&mut p.sprite_cache),
                mon.variant,
                mon.palette_id,
            )
        } else {
            (Vec::new(), Vec::new(), 0.0, 0.0)
        };
        if states.is_empty() {
            continue;
        }
        if let Some(materials) = ctx.sprite_materials.as_deref_mut() {
            for state in &states {
                for frame in state {
                    for handle in frame {
                        if let Some(m) = materials.get_mut(handle.id()) {
                            m.extension.tint = ctx.spawn_tint;
                        }
                    }
                }
            }
        }
        let dsft_scale = ctx.game_assets.lod().dsft_scale_for_group(&mon.standing_sprite);
        let (sw, sh) = (raw_w * dsft_scale, raw_h * dsft_scale);
        let angle = mon.group_index as f32 * 2.399_f32;
        let (wx, wz) = (
            mon.spawn_position[0] as f32 + mon.spawn_radius as f32 * angle.cos(),
            -(mon.spawn_position[1] as f32 + mon.spawn_radius as f32 * angle.sin()),
        );
        let pos = Vec3::new(
            wx,
            crate::game::collision::probe_ground_height(&prepared.map.height_map[..], None, wx, wz) + sh / 2.0,
            wz,
        );
        let state_count = states.len();
        let mut child = commands.spawn_empty();
        let child_id = child.id();
        child
            .insert(Name::new(format!("monster:{}", mon.name)))
            .insert(Mesh3d(ctx.meshes.add(Rectangle::new(sw, sh))))
            .insert(MeshMaterial3d(states[0][0][0].clone()))
            .insert(Transform::from_translation(pos))
            .insert(crate::game::entities::WorldEntity)
            .insert(crate::game::entities::EntityKind::Monster)
            .insert(crate::game::entities::AnimationState::Idle)
            .insert(sprites::SpriteSheet::new(
                states,
                vec![(sw, sh); state_count],
                state_masks,
            ))
            .insert(crate::game::interaction::MonsterInteractable { name: mon.name.clone() })
            .insert(crate::game::monster_ai::MonsterAiMode::Wander)
            .insert(actor::Actor {
                name: mon.name.clone(),
                hp: mon.hp,
                max_hp: mon.hp,
                move_speed: mon.move_speed as f32,
                initial_position: pos,
                guarding_position: pos,
                tether_distance: mon.radius as f32 * 2.0,
                wander_timer: (pos.x * 0.011 + pos.z * 0.017).abs().fract() * 4.0,
                wander_target: pos,
                facing_yaw: 0.0,
                hostile: true,
                variant: mon.variant,
                sound_ids: mon.sound_ids,
                fidget_timer: (pos.x * 0.013 + pos.z * 0.019).abs().fract() * 15.0 + 5.0,
                attack_range: mon.body_radius as f32 * 2.0,
                attack_timer: (pos.x * 0.007 + pos.z * 0.023).abs().fract() * 3.0 + 1.0,
                attack_anim_remaining: 0.0,
                ddm_id: -1,
                group_id: 0,
                aggro_range: mon.aggro_range,
                recovery_secs: mon.recovery_secs,
                sprite_half_height: sh / 2.0,
                can_fly: mon.can_fly,
                vertical_velocity: 0.0,
                ai_type: mon.ai_type.clone(),
            })
            .insert(InGame);
        commands.entity(ctx.terrain_entity).add_child(child_id);
        *spawned += 1;
    }
}
