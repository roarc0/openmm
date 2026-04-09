//! Per-frame decoration spawning (static, animated, and directional billboards).

use bevy::ecs::message::MessageWriter;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use crate::game::InGame;
use crate::game::coords::mm6_position_to_bevy;
use crate::game::optional::OptionalWrite;
use crate::game::sprites::loading as sprites;
use crate::game::sprites::material::unlit_billboard_material;

use crate::game::lighting::{DecorationLight, decoration_point_light};

use super::lazy_spawn::{PendingSpawns, SpawnCtx};

pub(super) fn spawn_decorations(
    commands: &mut Commands,
    ctx: &mut SpawnCtx,
    p: &mut PendingSpawns,
    bb_idx: &mut usize,
    spawned: &mut usize,
    sound_events: &mut Option<MessageWriter<crate::game::sound::effects::PlaySoundEvent>>,
) {
    let lod = ctx.game_assets.lod();
    let bb_len = p.billboard_order.len();
    while *bb_idx < bb_len && *spawned < ctx.batch_max && ctx.start.elapsed().as_secs_f32() * 1000.0 < ctx.time_budget {
        let dec_idx = p.billboard_order[*bb_idx];
        *bb_idx += 1;
        let dec = &p.decorations.entries()[dec_idx];
        let key = &dec.sprite_name;
        let dec_pos = Vec3::from(mm6_position_to_bevy(dec.position[0], dec.position[1], dec.position[2]));

        if dec.is_directional {
            // Directional decorations with a light_radius (ships' lanterns, etc.)
            // become SelfLit — pick the selflit shader variant so they don't
            // get dimmed at night. Non-lit directionals use the regular one.
            let is_selflit = dec.light_radius > 0;
            let Some(materials) = ctx.sprite_materials.as_deref_mut() else {
                continue;
            };
            let (dirs, dir_masks, px_w, px_h) = sprites::load_decoration_directions(
                &dec.sprite_name,
                ctx.game_assets.assets(),
                ctx.images,
                materials,
                &mut Some(&mut p.sprite_cache),
                is_selflit,
            );
            if px_w > 0.0 {
                let dsft_scale = lod.dsft_scale_for_group(&dec.sprite_name);
                let (sw, sh) = (px_w * dsft_scale, px_h * dsft_scale);
                let pos = dec_pos + Vec3::new(0.0, sh / 2.0, 0.0);
                let child_id = commands
                    .spawn((
                        Name::new(format!("decoration:{}", key)),
                        Mesh3d(ctx.meshes.add(Rectangle::new(sw, sh))),
                        MeshMaterial3d(dirs[0].clone()),
                        Transform::from_translation(pos),
                        crate::game::sprites::WorldEntity,
                        crate::game::sprites::EntityKind::Decoration,
                        crate::game::sprites::Billboard,
                        crate::game::sprites::AnimationState::Idle,
                        sprites::SpriteSheet::new(vec![vec![dirs]], vec![(sw, sh)], vec![vec![dir_masks]]),
                        crate::game::sprites::FacingYaw(dec.facing_yaw),
                    ))
                    .id();
                if !ctx.billboard_shadows {
                    commands.entity(child_id).insert(NotShadowCaster);
                }
                commands.entity(ctx.terrain_entity).add_child(child_id);
                if dec.event_id > 0 {
                    commands
                        .entity(child_id)
                        .insert(crate::game::interaction::DecorationInfo {
                            event_id: dec.event_id as u16,
                            position: pos,
                            billboard_index: dec.billboard_index,
                            declist_id: dec.declist_id,
                            ground_y: dec_pos.y,
                            half_w: 0.0,
                            half_h: 0.0,
                            mask: None,
                        });
                }
                if dec.trigger_radius > 0 && dec.event_id > 0 {
                    commands
                        .entity(child_id)
                        .insert(crate::game::interaction::DecorationTrigger::new(
                            dec.event_id as u16,
                            dec.trigger_radius as f32,
                        ));
                }
                if dec.light_radius > 0 {
                    let light_id = commands
                        .spawn(decoration_point_light(
                            DecorationLight::Ddeclist(dec.light_radius),
                            ctx.shadows,
                        ))
                        .id();
                    commands
                        .entity(child_id)
                        .add_child(light_id)
                        .insert(crate::game::sprites::SelfLit);
                }
                *spawned += 1;
            } else {
                continue;
            }
        } else if dec.num_frames > 1 {
            if ctx.sprite_materials.is_none() {
                continue;
            }
            let frame_sprites = lod.billboard_animation_frames(key, dec.declist_id);
            if frame_sprites.is_empty() {
                continue;
            }
            let (w, h) = frame_sprites[0].dimensions();
            if w == 0.0 || h == 0.0 {
                continue;
            }
            // Animated decorations become SelfLit if they have a ddeclist light
            // or a luminous DSFT frame (campfires, braziers). Decide up front so
            // we pick the matching tint buffer at material creation time.
            let first = &frame_sprites[0];
            let is_selflit =
                dec.light_radius > 0 || (first.d_sft_frame.is_luminous() && first.d_sft_frame.light_radius > 0);
            let pos = dec_pos + Vec3::new(0.0, h / 2.0, 0.0);
            let mut frame_mats = vec![];
            let mut frame_masks = vec![];
            for sprite in &frame_sprites {
                let rgba = sprite.image.to_rgba8();
                let msk = std::sync::Arc::new(crate::game::sprites::loading::AlphaMask::from_image(&rgba));
                let tex = ctx.images.add(crate::assets::rgba8_to_bevy_image(rgba));
                let Some(materials) = ctx.sprite_materials.as_deref_mut() else {
                    continue;
                };
                let mat = materials.add(unlit_billboard_material(tex, is_selflit));
                frame_mats.push(std::array::from_fn(|_| mat.clone()));
                frame_masks.push(std::array::from_fn(|_| msk.clone()));
            }
            let mut sheet = sprites::SpriteSheet::new(vec![frame_mats], vec![(w, h)], vec![frame_masks]);
            sheet.frame_duration = dec.frame_duration;
            let child_id = commands
                .spawn((
                    Name::new(format!("decoration:{}", key)),
                    Mesh3d(ctx.meshes.add(Rectangle::new(w, h))),
                    MeshMaterial3d(sheet.states[0][0][0].clone()),
                    Transform::from_translation(pos),
                    crate::game::sprites::WorldEntity,
                    crate::game::sprites::EntityKind::Decoration,
                    crate::game::sprites::Billboard,
                    crate::game::sprites::AnimationState::Idle,
                    sheet,
                ))
                .id();
            if !ctx.billboard_shadows {
                commands.entity(child_id).insert(NotShadowCaster);
            }
            commands.entity(ctx.terrain_entity).add_child(child_id);
            if dec.event_id > 0 {
                commands
                    .entity(child_id)
                    .insert(crate::game::interaction::DecorationInfo {
                        event_id: dec.event_id as u16,
                        position: pos,
                        billboard_index: dec.billboard_index,
                        declist_id: dec.declist_id,
                        ground_y: dec_pos.y,
                        half_w: 0.0,
                        half_h: 0.0,
                        mask: None,
                    });
            }
            if dec.trigger_radius > 0 && dec.event_id > 0 {
                commands
                    .entity(child_id)
                    .insert(crate::game::interaction::DecorationTrigger::new(
                        dec.event_id as u16,
                        dec.trigger_radius as f32,
                    ));
            }
            let light_source = if dec.light_radius > 0 {
                Some(DecorationLight::Ddeclist(dec.light_radius))
            } else {
                let f = &frame_sprites[0];
                if f.d_sft_frame.is_luminous() && f.d_sft_frame.light_radius > 0 {
                    Some(DecorationLight::AnimatedDsft(f.d_sft_frame.light_radius as u16))
                } else {
                    None
                }
            };
            if let Some(source) = light_source {
                let light_id = commands.spawn(decoration_point_light(source, ctx.shadows)).id();
                commands
                    .entity(child_id)
                    .add_child(light_id)
                    .insert(crate::game::sprites::SelfLit);
            }
            *spawned += 1;
        } else {
            // Decide selflit up front so the cached material references the right shader variant.
            let is_selflit = dec.light_radius > 0 || lod.billboard_luminous_light_radius(dec.declist_id) > 0;
            let (mat, quad, w, h, mask) = if let Some((m, q, w, h, msk)) = p.dec_sprite_cache.get(key) {
                (m.clone(), q.clone(), *w, *h, msk.clone())
            } else {
                let sprite = match lod.billboard(key, dec.declist_id) {
                    Some(s) => s,
                    None => continue,
                };
                let (w, h) = sprite.dimensions();
                let rgba = sprite.image.to_rgba8();
                let msk = std::sync::Arc::new(crate::game::sprites::loading::AlphaMask::from_image(&rgba));
                let tex = ctx.images.add(crate::assets::rgba8_to_bevy_image(rgba));
                let Some(materials) = ctx.sprite_materials.as_deref_mut() else {
                    continue;
                };
                let m = materials.add(unlit_billboard_material(tex, is_selflit));
                let q = ctx.meshes.add(Rectangle::new(w, h));
                p.dec_sprite_cache
                    .insert(key.clone(), (m.clone(), q.clone(), w, h, msk.clone()));
                (m, q, w, h, msk)
            };
            let pos = dec_pos + Vec3::new(0.0, h / 2.0, 0.0);
            let mut child = commands.spawn_empty();
            let child_id = child.id();
            child
                .insert(Name::new("decoration"))
                .insert(Mesh3d(quad))
                .insert(MeshMaterial3d(mat))
                .insert(Transform::from_translation(pos))
                .insert(crate::game::sprites::WorldEntity)
                .insert(crate::game::sprites::EntityKind::Decoration)
                .insert(crate::game::sprites::Billboard)
                .insert(InGame);
            if !ctx.billboard_shadows {
                commands.entity(child_id).insert(NotShadowCaster);
            }
            commands.entity(ctx.terrain_entity).add_child(child_id);
            if dec.event_id > 0 {
                commands
                    .entity(child_id)
                    .insert(crate::game::interaction::DecorationInfo {
                        event_id: dec.event_id as u16,
                        position: pos,
                        billboard_index: dec.billboard_index,
                        declist_id: dec.declist_id,
                        ground_y: dec_pos.y,
                        half_w: w / 2.0,
                        half_h: h / 2.0,
                        mask: Some(mask),
                    });
            }
            if dec.trigger_radius > 0 && dec.event_id > 0 {
                commands
                    .entity(child_id)
                    .insert(crate::game::interaction::DecorationTrigger::new(
                        dec.event_id as u16,
                        dec.trigger_radius as f32,
                    ));
            }
            if dec.flicker_rate > 0.0 {
                let phase = (pos.x * 0.137 + pos.z * 0.031).abs().fract();
                commands
                    .entity(child_id)
                    .insert(crate::game::sprites::DecorFlicker::new(dec.flicker_rate, phase));
            }
            let light_source = if dec.light_radius > 0 {
                Some(DecorationLight::Ddeclist(dec.light_radius))
            } else {
                let lr = lod.billboard_luminous_light_radius(dec.declist_id);
                if lr > 0 {
                    Some(DecorationLight::StaticDsft(lr))
                } else {
                    None
                }
            };
            if let Some(source) = light_source {
                let light_id = commands.spawn(decoration_point_light(source, ctx.shadows)).id();
                commands
                    .entity(child_id)
                    .add_child(light_id)
                    .insert(crate::game::sprites::SelfLit);
            }
            *spawned += 1;
        }
        if dec.sound_id > 0 {
            sound_events.try_write(crate::game::sound::effects::PlaySoundEvent {
                sound_id: dec.sound_id as u32,
                position: dec_pos,
            });
        }
    }
}
