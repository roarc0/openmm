//! Sprite loading and resolution for world entities.
//!
//! Resolves DSFT group IDs to sprite roots, loads directional sprite frames,
//! and provides the animation update system. Reusable by any entity type
//! (NPCs, monsters, decorations with animations).

use std::collections::HashMap;
use std::sync::Arc;

use bevy::prelude::*;
use image::DynamicImage;

use crate::game::actors::Actor;
use crate::game::player::PlayerCamera;
use crate::game::sprites::material::{SpriteMaterial, unlit_billboard_material};
use crate::game::sprites::{AnimationState, FacingYaw, billboard_face_yaw, quantize_billboard_yaw};
use openmm_data::Assets as DataAssets;

/// Pad an RGBA image to target dimensions, centered horizontally and bottom-aligned.
fn pad_sprite_image(rgba: image::RgbaImage, target_w: u32, target_h: u32) -> image::RgbaImage {
    if rgba.width() == target_w && rgba.height() == target_h {
        return rgba;
    }
    let mut padded = image::RgbaImage::new(target_w, target_h);
    let x_off = (target_w - rgba.width()) / 2;
    let y_off = target_h - rgba.height();
    for py in 0..rgba.height() {
        for px in 0..rgba.width() {
            padded.put_pixel(px + x_off, py + y_off, *rgba.get_pixel(px, py));
        }
    }
    padded
}

/// CPU-side 1-bit alpha mask for a sprite image. Used for pixel-accurate ray hit testing.
/// Built from the padded RGBA image at load time and kept in memory alongside the material.
pub struct AlphaMask {
    pub width: u32,
    pub height: u32,
    /// Row-major, true = opaque (alpha > 127).
    pub(crate) data: Vec<bool>,
}

impl AlphaMask {
    /// Build a mask from a padded RGBA sprite image. Pixels with alpha > 127 are opaque.
    pub fn from_image(img: &image::RgbaImage) -> Self {
        let data = img.pixels().map(|p| p[3] > 127).collect();
        Self {
            width: img.width(),
            height: img.height(),
            data,
        }
    }

    /// Construct an `AlphaMask` directly from raw boolean data (used in tests).
    pub fn new(width: u32, height: u32, data: Vec<bool>) -> Self {
        Self { width, height, data }
    }

    /// Test whether a UV coordinate hits an opaque pixel. Both u and v in [0,1].
    /// UV is clamped to image bounds — never panics on out-of-range input.
    pub fn test(&self, u: f32, v: f32) -> bool {
        let x = (u * self.width as f32).clamp(0.0, (self.width - 1) as f32) as u32;
        let y = (v * self.height as f32).clamp(0.0, (self.height - 1) as f32) as u32;
        self.data[(y * self.width + x) as usize]
    }
}

/// Cache for loaded sprite materials to avoid duplicate texture loading.
#[derive(Resource, Default, Clone)]
pub struct SpriteCache {
    /// Maps "root_name + frame_letter + direction" to material handle
    materials: HashMap<String, Handle<SpriteMaterial>>,
    /// Maps cache key to (width, height)
    dimensions: HashMap<String, (f32, f32)>,
    /// Alpha masks keyed identically to `materials`.
    masks: HashMap<String, Arc<AlphaMask>>,
}

impl SpriteCache {
    /// Pre-decode a list of (sprite_root, variant) pairs into the cache.
    /// Call during loading screen to avoid decoding during gameplay.
    pub fn preload(
        &mut self,
        roots: &[(&str, u8, u16)],
        assets: &DataAssets,
        images: &mut Assets<Image>,
        materials: &mut Assets<SpriteMaterial>,
        selflit: bool,
    ) {
        for &(root, variant, palette_id) in roots {
            load_sprite_frames(
                root,
                assets,
                images,
                materials,
                &mut Some(self),
                variant,
                0,
                0,
                palette_id,
                selflit,
            );
        }
    }
}

/// Build a cache key for a sprite root with optional variant, minimum size, and palette.
/// Format: "root", "root@v2", "root@v2p223", "root@64x128", "root@64x128@v2", "root@64x128@v2p223"
/// palette_id is included only when variant > 1 && palette_id > 0 (DSFT direct path).
fn cache_key(root: &str, variant: u8, min_w: u32, min_h: u32, palette_id: u16) -> String {
    let has_size = min_w > 0 || min_h > 0;
    let has_variant = variant > 1;
    let has_palette = has_variant && palette_id > 0;
    match (has_size, has_variant, has_palette) {
        (false, false, _) => root.to_string(),
        (false, true, false) => format!("{}@v{}", root, variant),
        (false, true, true) => format!("{}@v{}p{}", root, variant, palette_id),
        (true, false, _) => format!("{}@{}x{}", root, min_w, min_h),
        (true, true, false) => format!("{}@{}x{}@v{}", root, min_w, min_h, variant),
        (true, true, true) => format!("{}@{}x{}@v{}p{}", root, min_w, min_h, variant, palette_id),
    }
}

/// Preloaded sprite frames for an entity.
/// `states[state_idx][frame_idx]` = array of 5 material handles (directions 0-4).
#[derive(Component)]
pub struct SpriteSheet {
    /// states[0]=standing, states[1]=walking (if available)
    pub states: Vec<Vec<[Handle<SpriteMaterial>; 5]>>,
    /// Width and height per state (sprites can differ between standing/walking).
    pub state_dimensions: Vec<(f32, f32)>,
    /// Per-state, per-frame, per-direction alpha masks (parallel to `states`).
    pub state_masks: Vec<Vec<[Arc<AlphaMask>; 5]>>,
    /// The alpha mask for the currently displayed (state, frame, direction). Updated by update_sprite_sheets.
    pub current_mask: Option<Arc<AlphaMask>>,
    pub current_frame: usize,
    pub current_state: usize,
    pub frame_timer: f32,
    pub frame_duration: f32,
    /// Last applied (state, frame, direction) -- skip material swap when unchanged.
    last_applied: (usize, usize, usize),
}

impl SpriteSheet {
    pub fn new(
        states: Vec<Vec<[Handle<SpriteMaterial>; 5]>>,
        state_dimensions: Vec<(f32, f32)>,
        state_masks: Vec<Vec<[Arc<AlphaMask>; 5]>>,
    ) -> Self {
        Self {
            states,
            state_dimensions,
            state_masks,
            current_mask: None,
            current_frame: 0,
            current_state: 0,
            frame_timer: 0.0,
            frame_duration: 0.15,
            last_applied: (usize::MAX, usize::MAX, usize::MAX),
        }
    }
}

/// Load a complete entity's sprite set (standing + walking + attacking + dying) using the cache.
/// Returns (states, state_masks, quad_width, quad_height) where the quad uses the max
/// dimensions across all states so none gets stretched.
/// State indices: 0=standing, 1=walking, 2=attacking, 3=dying.
/// `palette_id` is the DSFT palette for this variant —
/// when non-zero and variant > 1, used directly for palette swap instead of
/// the offset-from-sprite-header approach (which uses a different numbering).
pub fn load_entity_sprites(
    standing_root: &str,
    walking_root: &str,
    attacking_root: &str,
    dying_root: &str,
    assets: &DataAssets,
    images: &mut Assets<Image>,
    materials: &mut Assets<SpriteMaterial>,
    cache: &mut Option<&mut SpriteCache>,
    variant: u8,
    palette_id: u16,
    selflit: bool,
) -> (
    Vec<Vec<[Handle<SpriteMaterial>; 5]>>,
    Vec<Vec<[Arc<AlphaMask>; 5]>>,
    f32,
    f32,
) {
    // Load walking first (usually wider) to get target dimensions
    let (walking, walking_masks, ww, wh) = load_sprite_frames(
        walking_root,
        assets,
        images,
        materials,
        cache,
        variant,
        0,
        0,
        palette_id,
        selflit,
    );

    // Load standing, padded to at least walking dimensions
    let (standing, standing_masks, sw, sh) = load_sprite_frames(
        standing_root,
        assets,
        images,
        materials,
        cache,
        variant,
        ww as u32,
        wh as u32,
        palette_id,
        selflit,
    );
    if standing.is_empty() {
        return (Vec::new(), Vec::new(), 0.0, 0.0);
    }

    let qw = sw.max(ww);
    let qh = sh.max(wh);

    // If standing is larger than walking, reload walking padded to match
    let (walking, walking_masks) = if !walking.is_empty() && (sw > ww || sh > wh) {
        let (padded, padded_masks, _, _) = load_sprite_frames(
            walking_root,
            assets,
            images,
            materials,
            cache,
            variant,
            qw as u32,
            qh as u32,
            palette_id,
            selflit,
        );
        (padded, padded_masks)
    } else {
        (walking, walking_masks)
    };

    // Load attacking animation (state 2), padded to the unified quad size.
    let (attacking, attacking_masks, _, _) = load_sprite_frames(
        attacking_root,
        assets,
        images,
        materials,
        cache,
        variant,
        qw as u32,
        qh as u32,
        palette_id,
        selflit,
    );

    // Load dying animation (state 3), padded to the unified quad size.
    let (dying, dying_masks, _, _) = load_sprite_frames(
        dying_root, assets, images, materials, cache, variant, qw as u32, qh as u32, palette_id, selflit,
    );

    let mut states = vec![standing];
    let mut state_masks = vec![standing_masks];
    if !walking.is_empty() {
        states.push(walking);
        state_masks.push(walking_masks);
    }
    if !attacking.is_empty() {
        states.push(attacking);
        state_masks.push(attacking_masks);
    }
    if !dying.is_empty() {
        states.push(dying);
        state_masks.push(dying_masks);
    }

    (states, state_masks, qw, qh)
}

/// Load sprite frames for a single animation (e.g. standing or walking).
///
/// `variant` controls tinting: 0/1 = none, 2 = blue, 3 = red.
/// `min_w`/`min_h` enforce minimum padding dimensions (used to pad standing
/// sprites to match walking sprite size). Pass 0 for no minimum.
/// `palette_id` is the DSFT palette — when non-zero and variant > 1, used directly.
pub fn load_sprite_frames(
    root: &str,
    assets: &DataAssets,
    images: &mut Assets<Image>,
    materials: &mut Assets<SpriteMaterial>,
    cache: &mut Option<&mut SpriteCache>,
    variant: u8,
    min_w: u32,
    min_h: u32,
    palette_id: u16,
    selflit: bool,
) -> (Vec<[Handle<SpriteMaterial>; 5]>, Vec<[Arc<AlphaMask>; 5]>, f32, f32) {
    let root = root.trim_end_matches(|c: char| c.is_ascii_digit());
    let key = cache_key(root, variant, min_w, min_h, palette_id);

    if let Some(c) = cache.as_ref()
        && let Some(&(w, h)) = c.dimensions.get(&key)
    {
        let (frames, masks) = rebuild_from_cache(&key, c);
        if !frames.is_empty() {
            return (frames, masks, w, h);
        }
    }

    // Try progressively shorter root names (e.g. "gobla" -> "gobl" -> "gob")
    let mut try_root = root;
    while try_root.len() >= 3 {
        let (frames, frame_masks, w, h) = decode_sprite_frames(
            try_root, assets, images, materials, variant, min_w, min_h, palette_id, selflit,
        );
        if !frames.is_empty() {
            store_in_cache(&key, &frames, &frame_masks, w, h, cache);
            return (frames, frame_masks, w, h);
        }
        try_root = &try_root[..try_root.len() - 1];
    }
    (Vec::new(), Vec::new(), 0.0, 0.0)
}

/// Convert an RGBA sprite image into a billboard material and alpha mask.
/// Centralises the rgba→mask→texture→material pipeline used by every sprite spawn site.
pub fn sprite_to_material_with_mask(
    rgba: image::RgbaImage,
    images: &mut bevy::prelude::Assets<Image>,
    materials: &mut bevy::prelude::Assets<SpriteMaterial>,
    selflit: bool,
) -> (bevy::prelude::Handle<SpriteMaterial>, Arc<AlphaMask>) {
    let mask = Arc::new(AlphaMask::from_image(&rgba));
    let tex = images.add(crate::assets::rgba8_to_bevy_image(rgba));
    let mat = materials.add(unlit_billboard_material(tex, selflit));
    (mat, mask)
}

fn store_in_cache(
    key: &str,
    frames: &[[Handle<SpriteMaterial>; 5]],
    frame_masks: &[[Arc<AlphaMask>; 5]],
    w: f32,
    h: f32,
    cache: &mut Option<&mut SpriteCache>,
) {
    if let Some(cache) = cache.as_mut() {
        cache.dimensions.insert(key.to_string(), (w, h));
        for (fi, (dirs, masks)) in frames.iter().zip(frame_masks.iter()).enumerate() {
            let frame_letter = (b'a' + fi as u8) as char;
            for di in 0..5 {
                let mat_key = format!("{}{}{}", key, frame_letter, di);
                cache.materials.insert(mat_key.clone(), dirs[di].clone());
                cache.masks.insert(mat_key, masks[di].clone());
            }
        }
    }
}

fn rebuild_from_cache(key: &str, cache: &SpriteCache) -> (Vec<[Handle<SpriteMaterial>; 5]>, Vec<[Arc<AlphaMask>; 5]>) {
    let mut frames = Vec::new();
    let mut mask_frames = Vec::new();
    let fallback_mask = Arc::new(AlphaMask {
        width: 1,
        height: 1,
        data: vec![true],
    });
    for fi in 0..6 {
        let frame_letter = (b'a' + fi) as char;
        let key0 = format!("{}{}0", key, frame_letter);
        if let Some(mat0) = cache.materials.get(&key0) {
            let mask0 = cache.masks.get(&key0).cloned().unwrap_or_else(|| fallback_mask.clone());
            let mut dirs: [Handle<SpriteMaterial>; 5] = Default::default();
            let mut masks: [Arc<AlphaMask>; 5] = std::array::from_fn(|_| fallback_mask.clone());
            for di in 0..5 {
                let mat_key = format!("{}{}{}", key, frame_letter, di);
                dirs[di] = cache.materials.get(&mat_key).cloned().unwrap_or_else(|| mat0.clone());
                masks[di] = cache.masks.get(&mat_key).cloned().unwrap_or_else(|| mask0.clone());
            }
            frames.push(dirs);
            mask_frames.push(masks);
        } else {
            break;
        }
    }
    (frames, mask_frames)
}

/// Decode sprite frames from the LOD, apply variant tinting, and pad to uniform size.
/// When `palette_id > 0` and `variant > 1`, uses the DSFT palette directly
/// (sprite file header palettes use a different numbering system).
fn decode_sprite_frames(
    root: &str,
    assets: &DataAssets,
    images: &mut Assets<Image>,
    materials: &mut Assets<SpriteMaterial>,
    variant: u8,
    min_w: u32,
    min_h: u32,
    palette_id: u16,
    selflit: bool,
) -> (Vec<[Handle<SpriteMaterial>; 5]>, Vec<[Arc<AlphaMask>; 5]>, f32, f32) {
    // First pass: collect all raw sprites and find max dimensions.
    let mut raw_sprites: Vec<Vec<Option<DynamicImage>>> = Vec::new();
    let mut max_w = min_w;
    let mut max_h = min_h;

    // Some dying sprites are stored as a single image with no frame/direction
    // suffix (e.g. "arc1diq" — the DSFT sprite_name IS the file name). Detect
    // this by attempting to load the root itself before the frame-letter loop.
    let root_lower = root.to_lowercase();
    let root_img = assets.lod().sprite(root);
    let single_frame_root = root_img.is_some()
        && !assets.lod_contains("sprites", &format!("{}a0", root_lower))
        && !assets.lod_contains("sprites", &format!("{}a", root_lower));

    if single_frame_root {
        // Load the single image for all 5 directional slots.
        let img = root_img;
        if let Some(ref i) = img {
            max_w = max_w.max(i.width());
            max_h = max_h.max(i.height());
        }
        raw_sprites.push(vec![img, None, None, None, None]);
    } else {
        for frame_char in b'a'..=b'f' {
            let frame_letter = frame_char as char;
            let test0 = format!("{}{}0", root, frame_letter);
            let test_nodir = format!("{}{}", root, frame_letter);

            let has_frame = assets.lod_contains("sprites", &test0) || assets.lod_contains("sprites", &test_nodir);
            if !has_frame {
                break;
            }

            let mut dir_imgs: Vec<Option<DynamicImage>> = Vec::with_capacity(5);
            for dir in 0..5u8 {
                let name = format!("{}{}{}", root, frame_letter, dir);
                let img = if variant > 1 && palette_id > 0 {
                    // Use DSFT palette directly (sprite header palettes differ in numbering)
                    let sprite_name = if assets.lod_contains("sprites", &name.to_lowercase()) {
                        &name
                    } else {
                        &test_nodir
                    };
                    assets
                        .lod()
                        .sprite_with_palette(sprite_name, palette_id)
                        .or_else(|| assets.lod().sprite(sprite_name))
                } else if variant > 1 {
                    let pal_offset = (variant - 1) as u16;
                    load_sprite_with_palette_offset(assets, &name, &test_nodir, pal_offset)
                } else {
                    assets.lod().sprite(&name).or_else(|| assets.lod().sprite(&test_nodir))
                };
                if let Some(ref i) = img {
                    max_w = max_w.max(i.width());
                    max_h = max_h.max(i.height());
                }
                dir_imgs.push(img);
            }
            raw_sprites.push(dir_imgs);
        }
    } // end else (multi-frame sprites)

    if raw_sprites.is_empty() || max_w == 0 {
        return (Vec::new(), Vec::new(), 0.0, 0.0);
    }

    // Second pass: pad to uniform size, build alpha masks, and create materials.
    let mut frames = Vec::new();
    let mut frame_masks: Vec<[Arc<AlphaMask>; 5]> = Vec::new();
    let fallback_mask = Arc::new(AlphaMask {
        width: 1,
        height: 1,
        data: vec![true],
    });
    for dir_imgs in raw_sprites {
        let mut dir_materials: [Handle<SpriteMaterial>; 5] = Default::default();
        let mut dir_masks: [Arc<AlphaMask>; 5] = std::array::from_fn(|_| fallback_mask.clone());
        for (dir, img_opt) in dir_imgs.into_iter().enumerate() {
            if let Some(img) = img_opt {
                let rgba = pad_sprite_image(img.into_rgba8(), max_w, max_h);
                let (mat, mask) = sprite_to_material_with_mask(rgba, images, materials, selflit);
                dir_materials[dir] = mat;
                dir_masks[dir] = mask;
            } else if dir > 0 {
                dir_materials[dir] = dir_materials[0].clone();
                dir_masks[dir] = dir_masks[0].clone();
            }
        }
        frames.push(dir_materials);
        frame_masks.push(dir_masks);
    }

    (frames, frame_masks, max_w as f32, max_h as f32)
}

/// Load a sprite with a palette offset applied (for monster variant palette swaps).
/// Reads the base sprite's palette_id from its header, adds the offset, and decodes
/// with the variant palette. Falls back to normal sprite() if palette not found.
fn load_sprite_with_palette_offset(
    assets: &DataAssets,
    name: &str,
    fallback: &str,
    palette_offset: u16,
) -> Option<DynamicImage> {
    // Try the primary name first, then the fallback (no-direction variant).
    // Use O(1) lod_contains to pick the name, then fetch bytes exactly once.
    let sprite_name = if assets.lod_contains("sprites", &name.to_lowercase()) {
        name
    } else if assets.lod_contains("sprites", &fallback.to_lowercase()) {
        fallback
    } else {
        return None;
    };

    // Read the base palette_id from the sprite header via openmm-data parser
    let sprite_data = assets
        .get_bytes(format!("sprites/{}", sprite_name.to_lowercase()))
        .ok()?;
    let base_palette_id = openmm_data::assets::image::sprite_palette_id(&sprite_data)?;
    let variant_palette_id = base_palette_id + palette_offset;

    // Try with variant palette, fall back to normal decode
    assets
        .lod()
        .sprite_with_palette(sprite_name, variant_palette_id)
        .or_else(|| assets.lod().sprite(sprite_name))
}

/// Update sprite sheets based on camera angle, entity facing, and animation state.
/// Works with Actor entities (NPCs/monsters) and directional decorations (FacingYaw).
/// Only processes visible entities within draw distance. Skips material swap when
/// the displayed (state, frame, direction) hasn't changed.
pub fn update_sprite_sheets(
    time: Res<Time>,
    cfg: Res<crate::system::config::GameConfig>,
    camera_query: Query<&GlobalTransform, With<PlayerCamera>>,
    mut query: Query<(
        &mut SpriteSheet,
        &mut MeshMaterial3d<SpriteMaterial>,
        &mut Transform,
        &GlobalTransform,
        &AnimationState,
        Option<&Actor>,
        Option<&FacingYaw>,
        &Visibility,
    )>,
    #[cfg(feature = "perf_log")] mut perf: ResMut<crate::screens::debug::perf_log::PerfCounters>,
) {
    #[cfg(feature = "perf_log")]
    let _start = crate::screens::debug::perf_log::perf_start();

    let Ok(camera_gt) = camera_query.single() else {
        return;
    };
    let cam_pos = camera_gt.translation();
    let cam_forward = camera_gt.forward().as_vec3();
    let dt = time.delta_secs();

    for (mut sprites, mut mat_handle, mut transform, global_transform, anim_state, actor, facing_yaw, vis) in
        query.iter_mut()
    {
        #[cfg(feature = "perf_log")]
        {
            perf.sprite_iter += 1;
        }
        if *vis == Visibility::Hidden {
            continue;
        }
        let actor_pos = global_transform.translation();

        // Skip animation for entities far from camera
        if cam_pos.distance_squared(actor_pos) > cfg.draw_distance * cfg.draw_distance {
            continue;
        }
        #[cfg(feature = "perf_log")]
        {
            perf.sprite_visible += 1;
        }

        let state_idx = match anim_state {
            AnimationState::Attacking if sprites.states.len() > 2 && !sprites.states[2].is_empty() => 2,
            AnimationState::Dying | AnimationState::Dead
                if sprites.states.len() > 3 && !sprites.states[3].is_empty() =>
            {
                3
            }
            AnimationState::Walking if sprites.states.len() > 1 && !sprites.states[1].is_empty() => 1,
            _ => 0,
        };
        // Dead corpses don't animate — skip all the update logic but keep current frame.
        let is_dead = *anim_state == AnimationState::Dead;
        let frame_count = sprites.states[state_idx].len();
        if frame_count == 0 {
            continue;
        }

        // Reset frame when state changes
        if state_idx != sprites.current_state {
            sprites.current_state = state_idx;
            // Dead corpses jump to last frame of dying animation (the corpse-on-ground frame).
            sprites.current_frame = if is_dead && frame_count > 0 { frame_count - 1 } else { 0 };
            sprites.frame_timer = 0.0;
        }

        // Dead corpses: skip animation advancement, but still allow rotation updates.
        if !is_dead {
            // Advance animation timer; dying animation clamps on the last frame instead of looping.
            let is_dying = *anim_state == AnimationState::Dying;
            sprites.frame_timer += dt;
            if sprites.frame_timer >= sprites.frame_duration {
                sprites.frame_timer -= sprites.frame_duration;
                let next = sprites.current_frame + 1;
                sprites.current_frame = if is_dying && next >= frame_count {
                    frame_count - 1 // hold last frame
                } else {
                    next % frame_count
                };
            }
        }

        // Pick directional frame based on camera angle relative to entity facing.
        // Actors use Actor.facing_yaw (updated by wander), decorations use FacingYaw (fixed from map).
        let entity_yaw = actor
            .map(|a| a.facing_yaw)
            .or_else(|| facing_yaw.map(|f| f.0))
            .unwrap_or(0.0);
        let raw_angle = billboard_face_yaw(cam_pos, cam_forward, actor_pos);
        // Use full-precision angle for direction selection (octant boundaries
        // matter) but quantized angle for the Transform rotation. 128 bins
        // per full turn ≈ 2.8° — imperceptible on a billboard, but prevents
        // tiny camera movements from dirtying every sprite's Transform.
        let camera_angle = quantize_billboard_yaw(raw_angle);
        let (direction, mirrored) = direction_for_angle(entity_yaw, raw_angle);

        // Only swap material when the displayed frame actually changed
        let current_key = (state_idx, sprites.current_frame, direction);
        if current_key != sprites.last_applied {
            sprites.last_applied = current_key;
            let new_mat = sprites.states[state_idx][sprites.current_frame][direction].clone();
            *mat_handle = MeshMaterial3d(new_mat);
            // Keep current_mask in sync with the displayed frame
            if state_idx < sprites.state_masks.len() && sprites.current_frame < sprites.state_masks[state_idx].len() {
                sprites.current_mask = Some(sprites.state_masks[state_idx][sprites.current_frame][direction].clone());
            }
            #[cfg(feature = "perf_log")]
            {
                perf.sprite_mat_swaps += 1;
            }
        }

        // Only write when value actually changed — writing Transform marks it dirty,
        // causing GlobalTransform propagation and render-world extraction every frame.
        let new_rot = Quat::from_rotation_y(camera_angle);
        if transform.rotation != new_rot {
            transform.rotation = new_rot;
            #[cfg(feature = "perf_log")]
            {
                perf.sprite_rot_writes += 1;
            }
        }
        // Scale only changes when mirrored octant flips (infrequent) — guard the write.
        let new_x_scale = if mirrored { -1.0 } else { 1.0 };
        if transform.scale.x != new_x_scale {
            transform.scale.x = new_x_scale;
            #[cfg(feature = "perf_log")]
            {
                perf.sprite_scale_writes += 1;
            }
        }
    }

    #[cfg(feature = "perf_log")]
    {
        perf.time_sprite_update_us += crate::screens::debug::perf_log::perf_elapsed_us(_start);
    }
}

/// Pick the sprite direction index (0-4) and mirror flag from an entity's facing
/// yaw and the camera angle. MM6 sprites have 5 pre-rendered views (0=front,
/// 1-4=rotations). Octants 5-7 mirror views 3-1 via negative X scale.
pub fn direction_for_angle(facing_yaw: f32, camera_angle: f32) -> (usize, bool) {
    let relative = (facing_yaw - camera_angle).rem_euclid(std::f32::consts::TAU);
    let octant = ((relative + std::f32::consts::FRAC_PI_8) / std::f32::consts::FRAC_PI_4) as usize % 8;
    match octant {
        0 => (0, false),
        1 => (1, false),
        2 => (2, false),
        3 => (3, false),
        4 => (4, false),
        5 => (3, true),
        6 => (2, true),
        7 => (1, true),
        _ => (0, false),
    }
}

/// Load decoration directional sprites (e.g. "shp0"-"shp4").
/// Unlike NPC sprites, decoration directions use `{root}{direction}` naming
/// with no frame letters or animation — just 5 pre-rendered views.
/// Returns (materials_for_5_directions, width, height).
pub fn load_decoration_directions(
    root: &str,
    assets: &DataAssets,
    images: &mut Assets<Image>,
    materials: &mut Assets<SpriteMaterial>,
    cache: &mut Option<&mut SpriteCache>,
    selflit: bool,
) -> ([Handle<SpriteMaterial>; 5], [Arc<AlphaMask>; 5], f32, f32) {
    let key = format!("dec:{}", root);
    if let Some(c) = cache.as_ref()
        && let Some(&(w, h)) = c.dimensions.get(&key)
    {
        let mut dirs: [Handle<SpriteMaterial>; 5] = Default::default();
        let fallback = Arc::new(AlphaMask {
            width: 1,
            height: 1,
            data: vec![true],
        });
        let mut masks: [Arc<AlphaMask>; 5] = std::array::from_fn(|_| fallback.clone());
        let mut found = true;
        for di in 0..5 {
            let mat_key = format!("{}a{}", key, di);
            if let Some(mat) = c.materials.get(&mat_key) {
                dirs[di] = mat.clone();
                masks[di] = c.masks.get(&mat_key).cloned().unwrap_or_else(|| fallback.clone());
            } else {
                found = false;
                break;
            }
        }
        if found {
            return (dirs, masks, w, h);
        }
    }

    // Load all direction sprites and find max dimensions
    let mut raw: Vec<Option<DynamicImage>> = Vec::with_capacity(5);
    let mut max_w = 0u32;
    let mut max_h = 0u32;
    for dir in 0..5u8 {
        let name = format!("{}{}", root, dir);
        let img = assets.lod().sprite(&name);
        if let Some(ref i) = img {
            max_w = max_w.max(i.width());
            max_h = max_h.max(i.height());
        }
        raw.push(img);
    }

    if max_w == 0 || raw[0].is_none() {
        return (
            Default::default(),
            std::array::from_fn(|_| {
                Arc::new(AlphaMask {
                    width: 1,
                    height: 1,
                    data: vec![true],
                })
            }),
            0.0,
            0.0,
        );
    }

    let fallback_mask = Arc::new(AlphaMask {
        width: 1,
        height: 1,
        data: vec![true],
    });
    let mut dirs: [Handle<SpriteMaterial>; 5] = Default::default();
    let mut dir_masks: [Arc<AlphaMask>; 5] = std::array::from_fn(|_| fallback_mask.clone());
    for (dir, img_opt) in raw.into_iter().enumerate() {
        if let Some(img) = img_opt {
            let rgba = pad_sprite_image(img.into_rgba8(), max_w, max_h);
            let (mat, mask) = sprite_to_material_with_mask(rgba, images, materials, selflit);
            dirs[dir] = mat;
            dir_masks[dir] = mask;
        } else if dir > 0 {
            dirs[dir] = dirs[0].clone();
            dir_masks[dir] = dir_masks[0].clone();
        }
    }

    // Store in cache
    if let Some(c) = cache.as_mut() {
        c.dimensions.insert(key.clone(), (max_w as f32, max_h as f32));
        for di in 0..5 {
            let mat_key = format!("{}a{}", key, di);
            c.materials.insert(mat_key.clone(), dirs[di].clone());
            c.masks.insert(mat_key, dir_masks[di].clone());
        }
    }

    (dirs, dir_masks, max_w as f32, max_h as f32)
}

/// Load a static (single-frame, non-directional) decoration sprite by name and return the
/// material, mesh, and world-space dimensions — ready to swap onto an existing entity.
///
/// Applies the DSFT group scale factor so the size matches what the spawn code produces.
/// Returns `None` if the sprite is not found in the LOD.
pub fn load_static_decoration_sprite(
    sprite_name: &str,
    assets: &DataAssets,
    images: &mut Assets<Image>,
    materials: &mut Assets<SpriteMaterial>,
    meshes: &mut Assets<Mesh>,
    selflit: bool,
) -> Option<(Handle<SpriteMaterial>, Handle<Mesh>, f32, f32)> {
    let name_lower = sprite_name.to_lowercase();
    let img = assets.lod().sprite(&name_lower)?;
    let dsft_scale = assets.lod().dsft_scale_for_group(&name_lower);
    let w = img.width() as f32 * dsft_scale;
    let h = img.height() as f32 * dsft_scale;
    let bevy_img = crate::assets::dynamic_to_bevy_image(img);
    let tex = images.add(bevy_img);
    let mat = materials.add(unlit_billboard_material(tex, selflit));
    let mesh = meshes.add(Rectangle::new(w, h));
    Some((mat, mesh, w, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_mask(width: u32, height: u32, opaque: &[(u32, u32)]) -> AlphaMask {
        let mut data = vec![false; (width * height) as usize];
        for &(x, y) in opaque {
            data[(y * width + x) as usize] = true;
        }
        AlphaMask { width, height, data }
    }

    #[test]
    fn alpha_mask_opaque_pixel() {
        let mask = make_mask(4, 4, &[(1, 1), (2, 2)]);
        assert!(mask.test(1.5 / 4.0, 1.5 / 4.0));
    }

    #[test]
    fn alpha_mask_transparent_pixel() {
        let mask = make_mask(4, 4, &[(1, 1)]);
        assert!(!mask.test(0.5 / 4.0, 0.5 / 4.0)); // pixel (0,0) is transparent
    }

    #[test]
    fn alpha_mask_clamped_edges() {
        let mask = make_mask(2, 2, &[(0, 0), (1, 0), (0, 1), (1, 1)]);
        assert!(mask.test(-0.5, -0.5)); // clamps to (0,0)
        assert!(mask.test(1.5, 1.5)); // clamps to (1,1)
    }

    #[test]
    fn alpha_mask_from_image() {
        let mut img = image::RgbaImage::new(2, 2);
        img.put_pixel(0, 0, image::Rgba([255, 0, 0, 255])); // opaque
        img.put_pixel(1, 0, image::Rgba([0, 0, 0, 0])); // transparent
        img.put_pixel(0, 1, image::Rgba([0, 0, 0, 0])); // transparent
        img.put_pixel(1, 1, image::Rgba([0, 255, 0, 128])); // semi → opaque (>127)
        let mask = AlphaMask::from_image(&img);
        assert!(mask.test(0.25, 0.25)); // pixel (0,0) opaque
        assert!(!mask.test(0.75, 0.25)); // pixel (1,0) transparent
        assert!(mask.test(0.75, 0.75)); // pixel (1,1) semi-opaque → opaque
    }
}
