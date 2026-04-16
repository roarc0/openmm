//! Shared decoration entity spawning (directional, animated, static).

use std::collections::HashMap;
use std::sync::Arc;

use bevy::prelude::*;

use openmm_data::provider::decorations::DecorationEntry;

use crate::game::InGame;
use crate::game::interaction::{DecorationInfo, DecorationTrigger};
use crate::game::rendering::lighting::{DecorationLight, decoration_point_light};
use crate::game::sprites::{
    AnimationState, Billboard, DecorFlicker, EntityKind, FacingYaw, SelfLit, WorldEntity,
    apply_shadow_config,
    loading::{self as sprites, AlphaMask, SpriteSheet},
};
use crate::game::sprites::material::SpriteMaterial;

use super::SpawnCtx;

/// Cached static decoration materials: sprite name → (material, mesh, w, h, mask).
pub type DecSpriteCache = HashMap<
    String,
    (
        Handle<SpriteMaterial>,
        Handle<Mesh>,
        f32,
        f32,
        Arc<AlphaMask>,
    ),
>;

/// Spawn a decoration entity (directional, animated, or static).
///
/// `dec_pos` is the ground-level position in Bevy coords (from `mm6_position_to_bevy`).
/// Returns the entity ID, or `None` if sprites fail to load.
pub fn spawn_decoration(
    commands: &mut Commands,
    ctx: &mut SpawnCtx,
    dec: &DecorationEntry,
    dec_pos: Vec3,
    parent: Option<Entity>,
    dec_sprite_cache: &mut DecSpriteCache,
) -> Option<Entity> {
    if dec.is_directional {
        spawn_directional(commands, ctx, dec, dec_pos, parent)
    } else if dec.num_frames > 1 {
        spawn_animated(commands, ctx, dec, dec_pos, parent)
    } else {
        spawn_static(commands, ctx, dec, dec_pos, parent, dec_sprite_cache)
    }
}

/// Directional decoration (ships' lanterns, oriented signs, etc.).
fn spawn_directional(
    commands: &mut Commands,
    ctx: &mut SpawnCtx,
    dec: &DecorationEntry,
    dec_pos: Vec3,
    parent: Option<Entity>,
) -> Option<Entity> {
    let key = &dec.sprite_name;
    let is_selflit = dec.light_radius > 0;

    let (dirs, dir_masks, px_w, px_h) = sprites::load_decoration_directions(
        key,
        ctx.game_assets.assets(),
        ctx.images,
        ctx.sprite_materials,
        &mut Some(ctx.sprite_cache),
        is_selflit,
    );
    if px_w == 0.0 {
        return None;
    }

    let lod = ctx.game_assets.lod();
    let dsft_scale = lod.dsft_scale_for_group(key);
    let (sw, sh) = (px_w * dsft_scale, px_h * dsft_scale);
    let pos = dec_pos + Vec3::new(0.0, sh / 2.0, 0.0);

    let ent = commands
        .spawn((
            Name::new(format!("decoration:{}", key)),
            Mesh3d(ctx.meshes.add(Rectangle::new(sw, sh))),
            MeshMaterial3d(dirs[0].clone()),
            Transform::from_translation(pos),
            WorldEntity,
            EntityKind::Decoration,
            Billboard,
            AnimationState::Idle,
            SpriteSheet::new(vec![vec![dirs]], vec![(sw, sh)], vec![vec![dir_masks]]),
            FacingYaw(dec.facing_yaw),
            InGame,
        ))
        .id();

    apply_shadow_config(commands, ent, ctx.billboard_shadows);
    attach_common(commands, ent, dec, pos, dec_pos.y, 0.0, 0.0, None);
    attach_light_as_child(commands, ent, dec, ctx.shadows);

    if let Some(p) = parent {
        commands.entity(p).add_child(ent);
    }

    Some(ent)
}

/// Animated decoration (campfires, braziers, flame torches).
fn spawn_animated(
    commands: &mut Commands,
    ctx: &mut SpawnCtx,
    dec: &DecorationEntry,
    dec_pos: Vec3,
    parent: Option<Entity>,
) -> Option<Entity> {
    let key = &dec.sprite_name;
    let lod = ctx.game_assets.lod();

    let frame_sprites = lod.billboard_animation_frames(key, dec.declist_id);
    if frame_sprites.is_empty() {
        return None;
    }
    let (w, h) = frame_sprites[0].dimensions();
    if w == 0.0 || h == 0.0 {
        return None;
    }

    // Animated decorations are selflit if they have a ddeclist light or a luminous DSFT frame.
    let first = &frame_sprites[0];
    let is_selflit =
        dec.light_radius > 0 || (first.d_sft_frame.is_luminous() && first.d_sft_frame.light_radius > 0);

    let pos = dec_pos + Vec3::new(0.0, h / 2.0, 0.0);
    let mut frame_mats = vec![];
    let mut frame_masks = vec![];
    for sprite in &frame_sprites {
        let rgba = sprite.image.to_rgba8();
        let (mat, msk) = sprites::sprite_to_material_with_mask(rgba, ctx.images, ctx.sprite_materials, is_selflit);
        frame_mats.push(std::array::from_fn(|_| mat.clone()));
        frame_masks.push(std::array::from_fn(|_| msk.clone()));
    }

    let mut sheet = SpriteSheet::new(vec![frame_mats], vec![(w, h)], vec![frame_masks]);
    sheet.frame_duration = dec.frame_duration;

    let ent = commands
        .spawn((
            Name::new(format!("decoration:{}", key)),
            Mesh3d(ctx.meshes.add(Rectangle::new(w, h))),
            MeshMaterial3d(sheet.states[0][0][0].clone()),
            Transform::from_translation(pos),
            WorldEntity,
            EntityKind::Decoration,
            Billboard,
            AnimationState::Idle,
            sheet,
            InGame,
        ))
        .id();

    apply_shadow_config(commands, ent, ctx.billboard_shadows);
    attach_common(commands, ent, dec, pos, dec_pos.y, 0.0, 0.0, None);
    // Animated flames don't get DecorFlicker — frame cycling IS the visual effect.

    // ddeclist light handled by attach_light_as_child.
    attach_light_as_child(commands, ent, dec, ctx.shadows);

    // DSFT-luminous animated decorations (campfires) carry light radius in the DSFT frame.
    if dec.light_radius == 0 {
        let dsft_lr = lod.billboard_luminous_light_radius(dec.declist_id);
        if dsft_lr > 0 {
            let light_id = commands
                .spawn(decoration_point_light(
                    DecorationLight::AnimatedDsft(dsft_lr),
                    ctx.shadows,
                ))
                .id();
            commands.entity(ent).add_child(light_id).insert(SelfLit);
        }
    }

    if let Some(p) = parent {
        commands.entity(p).add_child(ent);
    }

    Some(ent)
}

/// Static single-frame decoration (trees, rocks, signs, sconces).
fn spawn_static(
    commands: &mut Commands,
    ctx: &mut SpawnCtx,
    dec: &DecorationEntry,
    dec_pos: Vec3,
    parent: Option<Entity>,
    cache: &mut DecSpriteCache,
) -> Option<Entity> {
    let key = &dec.sprite_name;
    let lod = ctx.game_assets.lod();
    let dsft_lr = lod.billboard_luminous_light_radius(dec.declist_id);
    let is_selflit = dec.light_radius > 0 || dsft_lr > 0;

    let (mat, quad, w, h, mask) = if let Some((m, q, w, h, msk)) = cache.get(key) {
        (m.clone(), q.clone(), *w, *h, msk.clone())
    } else {
        let sprite = match lod.billboard(key, dec.declist_id) {
            Some(s) => s,
            None => return None,
        };
        let (w, h) = sprite.dimensions();
        if w == 0.0 || h == 0.0 {
            return None;
        }
        let rgba = sprite.image.to_rgba8();
        let (m, msk) = sprites::sprite_to_material_with_mask(rgba, ctx.images, ctx.sprite_materials, is_selflit);
        let q = ctx.meshes.add(Rectangle::new(w, h));
        cache.insert(key.clone(), (m.clone(), q.clone(), w, h, msk.clone()));
        (m, q, w, h, msk)
    };

    let pos = dec_pos + Vec3::new(0.0, h / 2.0, 0.0);
    let ent = commands
        .spawn((
            Name::new(format!("decoration:{}", key)),
            Mesh3d(quad),
            MeshMaterial3d(mat),
            Transform::from_translation(pos),
            WorldEntity,
            EntityKind::Decoration,
            Billboard,
            InGame,
        ))
        .id();

    apply_shadow_config(commands, ent, ctx.billboard_shadows);
    attach_common(commands, ent, dec, pos, dec_pos.y, w / 2.0, h / 2.0, Some(mask));

    // DecorFlicker for statics with a flicker rate.
    if dec.flicker_rate > 0.0 {
        let phase = (pos.x * 0.137 + pos.z * 0.031).abs().fract();
        commands.entity(ent).insert(DecorFlicker::new(dec.flicker_rate, phase));
    }

    // Light: ddeclist first, then DSFT luminous.
    attach_light_as_child(commands, ent, dec, ctx.shadows);
    if dec.light_radius == 0 && dsft_lr > 0 {
        let light_id = commands
            .spawn(decoration_point_light(
                DecorationLight::StaticDsft(dsft_lr),
                ctx.shadows,
            ))
            .id();
        commands.entity(ent).add_child(light_id).insert(SelfLit);
    }

    if let Some(p) = parent {
        commands.entity(p).add_child(ent);
    }

    Some(ent)
}

/// Attach components common to all decoration types.
fn attach_common(
    commands: &mut Commands,
    ent: Entity,
    dec: &DecorationEntry,
    pos: Vec3,
    ground_y: f32,
    half_w: f32,
    half_h: f32,
    mask: Option<Arc<AlphaMask>>,
) {
    commands
        .entity(ent)
        .insert(DecorationInfo::from_entry(dec, pos, ground_y, half_w, half_h, mask));

    if dec.trigger_radius > 0 && dec.event_id > 0 {
        commands
            .entity(ent)
            .insert(DecorationTrigger::new(dec.event_id as u16, dec.trigger_radius as f32));
    }
}

/// Attach a ddeclist point light as a child entity + SelfLit marker.
fn attach_light_as_child(
    commands: &mut Commands,
    ent: Entity,
    dec: &DecorationEntry,
    shadows: bool,
) {
    if dec.light_radius > 0 {
        let light_id = commands
            .spawn(decoration_point_light(
                DecorationLight::Ddeclist(dec.light_radius),
                shadows,
            ))
            .id();
        commands.entity(ent).add_child(light_id).insert(SelfLit);
    }
}
