use bevy::camera::{ImageRenderTarget, RenderTarget, Viewport};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureFormat, TextureUsages};
use bevy::window::PrimaryWindow;

use crate::system::config::GameConfig;
use crate::game::player::PlayerCamera;
use crate::screens::ui_assets::UiAssets;

/// Holds the render-to-texture state when `render_scale < 1.0`.
/// The 3D camera renders to `image` at reduced resolution; `display_node`
/// is a UI ImageNode that stretches the result to fill the viewport area.
#[derive(Resource)]
pub struct RenderScaleState {
    pub image: Handle<Image>,
    display_node: Entity,
    /// Cached (vp_w, vp_h, scale) to detect when we need to recreate.
    cached_key: (u32, u32, u32),
}

// MM6 reference dimensions — all HUD dimensions scale relative to these
pub(super) const REF_H: f32 = 480.0;
pub(super) const REF_W: f32 = 640.0;

// Footer layout constants (reference pixels at 640x480)
pub(super) const FOOTER_EXPOSED_H: f32 = 19.0; // FOOTER_H(24) - FOOTER_OVERLAP(10) + FOOTER_LIFT(5)

pub(super) struct HudDimensions {
    pub scale_x: f32,
    pub scale_y: f32,
    pub border1_w: f32,
    pub border1_h: f32,
    pub border2_w: f32,
    pub border2_h: f32,
    pub border3_w: f32,
    pub border3_h: f32,
    pub border4_w: f32,
    pub tap_w: f32,
    pub tap_h: f32,
    pub footer_w: f32,
    pub footer_h: f32,
    pub compass_w: f32,
    pub compass_h: f32,
}

impl HudDimensions {
    pub fn scale_h(&self, ref_px: f32) -> f32 {
        ref_px * self.scale_y
    }

    pub fn scale_w(&self, ref_px: f32) -> f32 {
        ref_px * self.scale_x
    }
}

/// Compute all HUD dimensions from letterboxed logical size and actual asset dimensions.
pub(super) fn hud_dimensions(width: f32, height: f32, ui: &UiAssets) -> HudDimensions {
    let scale_x = width / REF_W;
    let scale_y = height / REF_H;

    let dim_or = |name: &str, fw: f32, fh: f32| -> (f32, f32) {
        ui.dimensions(name)
            .map(|(w, h)| (w as f32, h as f32))
            .unwrap_or((fw, fh))
    };
    let dim = |name: &str| -> (f32, f32) {
        ui.dimensions(name)
            .map(|(w, h)| (w as f32, h as f32))
            .unwrap_or((0.0, 0.0))
    };

    let border1 = dim_or("border1.pcx", 172.0, 339.0);
    let border2 = dim_or("border2.pcx", 469.0, 109.0);
    let border3 = dim_or("border3", 468.0, 8.0);
    let border4 = dim_or("border4", 8.0, 344.0);
    let tap1 = dim_or("tap1", 172.0, 142.0);
    let footer = dim_or("footer", 483.0, 24.0);
    let compass = dim("compass");

    HudDimensions {
        scale_x,
        scale_y,
        border1_w: border1.0 * scale_x,
        border1_h: (border1.1 - 1.0) * scale_y,
        border2_w: border2.0 * scale_x,
        border2_h: border2.1 * scale_y,
        border3_w: border3.0 * scale_x,
        border3_h: border3.1 * scale_y,
        border4_w: border4.0 * scale_x,
        tap_w: tap1.0 * scale_x,
        tap_h: tap1.1 * scale_y,
        footer_w: footer.0 * scale_x,
        footer_h: footer.1 * scale_y,
        compass_w: compass.0 * scale_x,
        compass_h: compass.1 * scale_y,
    }
}

/// Parse aspect ratio string like "4:3" into a float (width/height).
pub fn parse_aspect_ratio(s: &str) -> Option<f32> {
    let (lhs, rhs) = s.split_once(':')?;
    let w: f32 = lhs.trim().parse().ok()?;
    let h: f32 = rhs.trim().parse().ok()?;
    if h > 0.0 { Some(w / h) } else { None }
}

/// Compute the letterboxed region within the physical window.
/// Returns (offset_x, offset_y, width, height) in physical pixels.
pub(super) fn letterbox_rect(window: &Window, cfg: &GameConfig) -> (u32, u32, u32, u32) {
    let pw = window.physical_width();
    let ph = window.physical_height();
    if let Some(target) = parse_aspect_ratio(&cfg.aspect_ratio) {
        let current = pw as f32 / ph as f32;
        if (current - target).abs() < 0.01 {
            (0, 0, pw, ph)
        } else if current > target {
            let w = (ph as f32 * target) as u32;
            let ox = (pw - w) / 2;
            (ox, 0, w, ph)
        } else {
            let h = (pw as f32 / target) as u32;
            let oy = (ph - h) / 2;
            (0, oy, pw, h)
        }
    } else {
        (0, 0, pw, ph)
    }
}

/// Shared preamble for viewport calculations: letterbox, scale, offsets, dimensions.
/// Returns (bar_x, bar_y, lw, lh, dims).
fn viewport_base(window: &Window, cfg: &GameConfig, ui: &UiAssets) -> (f32, f32, f32, f32, HudDimensions) {
    let sf = window.scale_factor();
    let (_, _, lpw, lph) = letterbox_rect(window, cfg);
    let lw = lpw as f32 / sf;
    let lh = lph as f32 / sf;
    let d = hud_dimensions(lw, lh, ui);
    let bar_x = (window.width() - lw) / 2.0;
    let bar_y = (window.height() - lh) / 2.0;
    (bar_x, bar_y, lw, lh, d)
}

/// Compute the 3D viewport rect in logical pixels: (left, top, width, height).
/// This is the playable area — letterboxed region minus HUD borders.
pub fn viewport_rect(window: &Window, cfg: &GameConfig, ui: &UiAssets) -> (f32, f32, f32, f32) {
    let (bar_x, bar_y, lw, lh, d) = viewport_base(window, cfg, ui);
    let footer_exposed = d.scale_h(FOOTER_EXPOSED_H);
    (
        bar_x,
        bar_y + d.border3_h,
        lw - d.border1_w,
        lh - d.border3_h - d.border2_h - footer_exposed,
    )
}

/// Compute the inner viewport rect (excluding left border4) in logical pixels.
/// Returns (left, top, width, height).
pub fn viewport_inner_rect(window: &Window, cfg: &GameConfig, ui: &UiAssets) -> (f32, f32, f32, f32) {
    let (bar_x, bar_y, lw, lh, d) = viewport_base(window, cfg, ui);
    let footer_exposed = d.scale_h(FOOTER_EXPOSED_H);
    (
        bar_x + d.border4_w,
        bar_y + d.border3_h,
        lw - d.border1_w - d.border4_w,
        lh - d.border3_h - d.border2_h - footer_exposed,
    )
}

/// Update the 3D camera viewport to the letterboxed area minus HUD borders.
/// When `render_scale < 1.0`, renders to a lower-res texture and displays
/// it stretched via a UI node — HUD stays at native resolution.
pub(crate) fn update_viewport(
    mut commands: Commands,
    windows: Query<&Window, With<PrimaryWindow>>,
    cfg: Res<GameConfig>,
    ui_assets: Res<UiAssets>,
    mut images: ResMut<Assets<Image>>,
    mut player_cameras: Query<(Entity, &mut Camera), With<PlayerCamera>>,
    scale_state: Option<ResMut<RenderScaleState>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };

    let (lx, ly, lw, lh) = letterbox_rect(window, &cfg);
    let sf = window.scale_factor();
    let lw_logical = lw as f32 / sf;
    let lh_logical = lh as f32 / sf;
    let d = hud_dimensions(lw_logical, lh_logical, &ui_assets);

    let sidebar_w = (d.border1_w * sf) as u32;
    let top_h = (d.border3_h * sf) as u32;
    let footer_exposed = (d.scale_h(FOOTER_EXPOSED_H) * sf) as u32;
    let bottom_h = (d.border2_h * sf) as u32 + footer_exposed;

    let vp_x = lx;
    let vp_y = ly + top_h;
    let vp_w = lw.saturating_sub(sidebar_w).max(1);
    let vp_h = lh.saturating_sub(top_h + bottom_h).max(1);

    let scale = cfg.render_scale.clamp(0.1, 1.0);

    if scale >= 1.0 {
        // Native resolution: render directly to window viewport.
        // Tear down render-to-texture state if it exists.
        if let Some(state) = scale_state {
            commands.entity(state.display_node).despawn();
            // Restore camera to render to the primary window.
            for (cam_entity, _) in player_cameras.iter() {
                commands.entity(cam_entity).insert(RenderTarget::default());
            }
            commands.remove_resource::<RenderScaleState>();
        }

        let pos = UVec2::new(vp_x, vp_y);
        let size = UVec2::new(vp_w, vp_h);
        for (_, mut camera) in player_cameras.iter_mut() {
            let changed = camera
                .viewport
                .as_ref()
                .is_none_or(|v| v.physical_position != pos || v.physical_size != size);
            if changed {
                camera.viewport = Some(Viewport {
                    physical_position: pos,
                    physical_size: size,
                    ..default()
                });
            }
        }
        return;
    }

    // Scaled rendering: render 3D to a lower-res texture, display via UI node.
    let scaled_w = ((vp_w as f32 * scale) as u32).max(1);
    let scaled_h = ((vp_h as f32 * scale) as u32).max(1);
    // Quantize scale to integer permille for cheap change detection.
    let scale_key = (scale * 1000.0) as u32;
    let key = (vp_w, vp_h, scale_key);

    // Logical position/size for the UI display node.
    let logical_left = vp_x as f32 / sf;
    let logical_top = vp_y as f32 / sf;
    let logical_w = vp_w as f32 / sf;
    let logical_h = vp_h as f32 / sf;

    let image_target = |handle: Handle<Image>| -> RenderTarget {
        RenderTarget::Image(ImageRenderTarget {
            handle,
            scale_factor: 1.0,
        })
    };

    if let Some(mut state) = scale_state {
        if state.cached_key != key {
            // Resolution or scale changed — resize the render target.
            if let Some(img) = images.get_mut(&state.image) {
                img.resize(Extent3d {
                    width: scaled_w,
                    height: scaled_h,
                    depth_or_array_layers: 1,
                });
            }
            state.cached_key = key;
            // Update display node size/position.
            if let Ok(mut node) = commands.get_entity(state.display_node) {
                node.insert(Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(logical_left),
                    top: Val::Px(logical_top),
                    width: Val::Px(logical_w),
                    height: Val::Px(logical_h),
                    ..default()
                });
            }
        }
        // Camera targets the existing render texture (no viewport — fills the image).
        let handle = state.image.clone();
        for (cam_entity, mut camera) in player_cameras.iter_mut() {
            commands.entity(cam_entity).insert(image_target(handle.clone()));
            camera.viewport = None;
        }
    } else {
        // First time at this scale — create render target and display node.
        let size = Extent3d {
            width: scaled_w,
            height: scaled_h,
            depth_or_array_layers: 1,
        };
        let mut image = Image::default();
        image.texture_descriptor.size = size;
        image.texture_descriptor.format = TextureFormat::Bgra8UnormSrgb;
        image.texture_descriptor.usage =
            TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
        image.resize(size);
        let image_handle = images.add(image);

        // UI node that stretches the render texture to fill the viewport area.
        // Rendered behind the HUD (default UI ordering).
        let display_node = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(logical_left),
                    top: Val::Px(logical_top),
                    width: Val::Px(logical_w),
                    height: Val::Px(logical_h),
                    ..default()
                },
                ImageNode::new(image_handle.clone()),
                // Low z-index so HUD renders on top.
                ZIndex(-100),
                crate::game::InGame,
            ))
            .id();

        for (cam_entity, mut camera) in player_cameras.iter_mut() {
            commands.entity(cam_entity).insert(image_target(image_handle.clone()));
            camera.viewport = None;
        }

        commands.insert_resource(RenderScaleState {
            image: image_handle,
            display_node,
            cached_key: key,
        });
    }
}
