//! Per-frame decoration spawning (static, animated, and directional billboards).

use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;

use crate::game::InGame;
use crate::game::entities::sprites;
use crate::game::optional::OptionalWrite;
use crate::game::sprite_material::{SpriteExtension, SpriteMaterial};

use super::lazy_spawn::{PendingSpawns, SpawnCtx};
use super::{DSFT_ANIMATED_LR_SCALE, DSFT_STATIC_LR_SCALE, decoration_point_light};

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
        let dec_pos = Vec3::from(openmm_data::odm::mm6_to_bevy(
            dec.position[0],
            dec.position[1],
            dec.position[2],
        ));

        if dec.is_directional {
            let Some(materials) = ctx.sprite_materials.as_deref_mut() else {
                continue;
            };
            let (dirs, dir_masks, px_w, px_h) = sprites::load_decoration_directions(
                &dec.sprite_name,
                ctx.game_assets.assets(),
                ctx.images,
                materials,
                &mut Some(&mut p.sprite_cache),
            );
            if px_w > 0.0 {
                for dir in &dirs {
                    if let Some(materials) = ctx.sprite_materials.as_deref_mut()
                        && let Some(m) = materials.get_mut(dir.id())
                    {
                        m.extension.tint = ctx.spawn_tint;
                    }
                }
                let dsft_scale = lod.dsft_scale_for_group(&dec.sprite_name);
                let (sw, sh) = (px_w * dsft_scale, px_h * dsft_scale);
                let pos = dec_pos + Vec3::new(0.0, sh / 2.0, 0.0);
                let child_id = commands
                    .spawn((
                        Name::new(format!("decoration:{}", key)),
                        Mesh3d(ctx.meshes.add(Rectangle::new(sw, sh))),
                        MeshMaterial3d(dirs[0].clone()),
                        Transform::from_translation(pos),
                        crate::game::entities::WorldEntity,
                        crate::game::entities::EntityKind::Decoration,
                        crate::game::entities::Billboard,
                        crate::game::entities::AnimationState::Idle,
                        sprites::SpriteSheet::new(vec![vec![dirs]], vec![(sw, sh)], vec![vec![dir_masks]]),
                        crate::game::entities::FacingYaw(dec.facing_yaw),
                    ))
                    .id();
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
                    let light_id = commands.spawn(decoration_point_light(dec.light_radius)).id();
                    commands
                        .entity(child_id)
                        .add_child(light_id)
                        .insert(crate::game::entities::SelfLit);
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
            let pos = dec_pos + Vec3::new(0.0, h / 2.0, 0.0);
            let mut frame_mats = vec![];
            let mut frame_masks = vec![];
            for sprite in &frame_sprites {
                let rgba = sprite.image.to_rgba8();
                let msk = std::sync::Arc::new(crate::game::entities::sprites::AlphaMask::from_image(&rgba));
                let tex = ctx
                    .images
                    .add(crate::assets::dynamic_to_bevy_image(image::DynamicImage::ImageRgba8(
                        rgba,
                    )));
                let Some(materials) = ctx.sprite_materials.as_deref_mut() else {
                    continue;
                };
                let mat = materials.add(SpriteMaterial {
                    base: StandardMaterial {
                        unlit: true,
                        base_color_texture: Some(tex),
                        alpha_mode: AlphaMode::Mask(0.5),
                        cull_mode: None,
                        double_sided: true,
                        perceptual_roughness: 1.0,
                        reflectance: 0.0,
                        ..default()
                    },
                    extension: SpriteExtension { tint: ctx.spawn_tint },
                });
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
                    crate::game::entities::WorldEntity,
                    crate::game::entities::EntityKind::Decoration,
                    crate::game::entities::Billboard,
                    crate::game::entities::AnimationState::Idle,
                    sheet,
                ))
                .id();
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
            let effective_lr = if dec.light_radius > 0 {
                dec.light_radius
            } else {
                let f = &frame_sprites[0];
                if f.d_sft_frame.is_luminous() && f.d_sft_frame.light_radius > 0 {
                    (f.d_sft_frame.light_radius as u16).saturating_mul(DSFT_ANIMATED_LR_SCALE)
                } else {
                    0
                }
            };
            if effective_lr > 0 {
                let light_id = commands.spawn(decoration_point_light(effective_lr)).id();
                commands
                    .entity(child_id)
                    .add_child(light_id)
                    .insert(crate::game::entities::SelfLit);
            }
            *spawned += 1;
        } else {
            let (mat, quad, w, h, mask) = if let Some((m, q, w, h, msk)) = p.dec_sprite_cache.get(key) {
                (m.clone(), q.clone(), *w, *h, msk.clone())
            } else {
                let sprite = match lod.billboard(key, dec.declist_id) {
                    Some(s) => s,
                    None => continue,
                };
                let (w, h) = sprite.dimensions();
                let rgba = sprite.image.to_rgba8();
                let msk = std::sync::Arc::new(crate::game::entities::sprites::AlphaMask::from_image(&rgba));
                let tex = ctx
                    .images
                    .add(crate::assets::dynamic_to_bevy_image(image::DynamicImage::ImageRgba8(
                        rgba,
                    )));
                let Some(materials) = ctx.sprite_materials.as_deref_mut() else {
                    continue;
                };
                let m = materials.add(SpriteMaterial {
                    base: StandardMaterial {
                        unlit: true,
                        base_color_texture: Some(tex),
                        alpha_mode: AlphaMode::Mask(0.5),
                        cull_mode: None,
                        double_sided: true,
                        perceptual_roughness: 1.0,
                        reflectance: 0.0,
                        ..default()
                    },
                    extension: SpriteExtension { tint: ctx.spawn_tint },
                });
                let q = ctx.meshes.add(Rectangle::new(w, h));
                p.dec_sprite_cache
                    .insert(key.clone(), (m.clone(), q.clone(), w, h, msk.clone()));
                (m, q, w, h, msk)
            };
            if let Some(materials) = ctx.sprite_materials.as_deref_mut()
                && let Some(m) = materials.get_mut(mat.id())
            {
                m.extension.tint = ctx.spawn_tint;
            }
            let pos = dec_pos + Vec3::new(0.0, h / 2.0, 0.0);
            let mut child = commands.spawn_empty();
            let child_id = child.id();
            child
                .insert(Name::new("decoration"))
                .insert(Mesh3d(quad))
                .insert(MeshMaterial3d(mat))
                .insert(Transform::from_translation(pos))
                .insert(crate::game::entities::WorldEntity)
                .insert(crate::game::entities::EntityKind::Decoration)
                .insert(crate::game::entities::Billboard)
                .insert(InGame);
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
                    .insert(crate::game::entities::DecorFlicker::new(dec.flicker_rate, phase));
            }
            let effective_lr = if dec.light_radius > 0 {
                dec.light_radius
            } else {
                lod.billboard_luminous_light_radius(dec.declist_id)
                    .saturating_mul(DSFT_STATIC_LR_SCALE)
            };
            if effective_lr > 0 {
                let light_id = commands.spawn(decoration_point_light(effective_lr)).id();
                commands
                    .entity(child_id)
                    .add_child(light_id)
                    .insert(crate::game::entities::SelfLit);
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
