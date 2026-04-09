use bevy::camera::Viewport;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::config::GameConfig;
use crate::game::hud::UiAssets;
use crate::game::player::PlayerCamera;

// MM6 reference dimensions — all HUD dimensions scale relative to these
pub(super) const REF_H: f32 = 480.0;
pub(super) const REF_W: f32 = 640.0;

// Footer layout constants (reference pixels at 640x480)
pub(super) const FOOTER_H: f32 = 24.0;
pub(super) const FOOTER_OVERLAP: f32 = 10.0;
pub(super) const FOOTER_LIFT: f32 = 5.0;
pub(super) const FOOTER_EXPOSED_H: f32 = FOOTER_H - FOOTER_OVERLAP + FOOTER_LIFT;

// Border markers for dynamic layout
#[derive(Component)]
pub(super) struct HudBorder1;
#[derive(Component)]
pub(super) struct HudBorder2;
#[derive(Component)]
pub(super) struct HudBorder3;
#[derive(Component)]
pub(super) struct HudBorder4;
#[derive(Component)]
pub(super) struct HudBorder5;
#[derive(Component)]
pub(super) struct HudBorder6;
#[derive(Component)]
pub(super) struct HudCorner;
#[derive(Component)]
pub(super) struct HudMapback;
#[derive(Component)]
pub(super) struct HudRoot;
#[derive(Component)]
pub(super) struct HudCamera;

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
    pub border4_h: f32,
    pub border5_w: f32,
    pub border5_h: f32,
    pub border6_w: f32,
    pub border6_h: f32,
    pub tap_w: f32,
    pub tap_h: f32,
    pub footer_w: f32,
    pub footer_h: f32,
    pub compass_w: f32,
    pub compass_h: f32,
    pub corner_h: f32,
}

impl HudDimensions {
    /// Scale a reference Y value (height or vertical position).
    pub fn scale_h(&self, ref_px: f32) -> f32 {
        ref_px * self.scale_y
    }

    /// Scale a reference X value (width or horizontal position).
    pub fn scale_w(&self, ref_px: f32) -> f32 {
        ref_px * self.scale_x
    }
}

/// Compute all HUD dimensions from letterboxed logical size and actual asset dimensions.
/// Widths scale by `scale_x` (width / 640), heights by `scale_y` (height / 480).
/// Asset sizes are read dynamically from `UiAssets` -- no hardcoded pixel values.
pub(super) fn hud_dimensions(width: f32, height: f32, ui: &UiAssets) -> HudDimensions {
    let scale_x = width / REF_W;
    let scale_y = height / REF_H;

    // Read true pixel dimensions from loaded assets (fallback to 0 if not loaded yet)
    let dim = |name: &str| -> (f32, f32) {
        ui.dimensions(name)
            .map(|(w, h)| (w as f32, h as f32))
            .unwrap_or((0.0, 0.0))
    };

    let border1 = dim("border1.pcx");
    let border2 = dim("border2.pcx");
    let border3 = dim("border3");
    let border4 = dim("border4");
    let border5 = dim("border5");
    let border6 = dim("border6");
    let tap1 = dim("tap1");
    let footer = dim("footer");
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
        border4_h: border4.1 * scale_y,
        border5_w: border5.0 * scale_x,
        border5_h: border5.1 * scale_y,
        border6_w: border6.0 * scale_x,
        border6_h: border6.1 * scale_y,
        tap_w: tap1.0 * scale_x,
        tap_h: tap1.1 * scale_y,
        footer_w: footer.0 * scale_x,
        footer_h: footer.1 * scale_y,
        compass_w: compass.0 * scale_x,
        compass_h: compass.1 * scale_y,
        corner_h: height - border1.1 * scale_y - tap1.1 * scale_y,
    }
}

/// Full HUD canvas reference dimensions: (width, height) in original image pixels.
///   width  = border1.w + border2.w - 1   (1-pixel overlap between right sidebar and bottom panel)
///   height = border1.h
pub fn hud_canvas_dims(ui: &UiAssets) -> (f32, f32) {
    let (b1w, b1h) = ui.dimensions("border1.pcx").unwrap_or((640, 480));
    let (b2w, _) = ui.dimensions("border2.pcx").unwrap_or((0, 0));
    ((b1w + b2w).saturating_sub(1) as f32, b1h as f32)
}

/// Convert a MM6 UI reference coordinate to HudRoot-local logical pixels (left, top).
/// `(ref_x, ref_y)` are pixel positions in the full HUD canvas (see `hud_canvas_dims`).
/// `(lw, lh)` are the letterboxed logical dimensions.
pub fn hud_ref_to_local(ref_x: f32, ref_y: f32, lw: f32, lh: f32, ref_w: f32, ref_h: f32) -> (f32, f32) {
    (ref_x * lw / ref_w, ref_y * lh / ref_h)
}

/// Parse aspect ratio string like "4:3" into a float (width/height).
pub fn parse_aspect_ratio(s: &str) -> Option<f32> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() == 2 {
        let w: f32 = parts[0].trim().parse().ok()?;
        let h: f32 = parts[1].trim().parse().ok()?;
        if h > 0.0 { Some(w / h) } else { None }
    } else {
        None
    }
}

/// Compute the letterboxed region within the physical window.
/// Returns (offset_x, offset_y, width, height) in physical pixels.
pub(super) fn letterbox_rect(window: &Window, cfg: &GameConfig) -> (u32, u32, u32, u32) {
    let pw = window.physical_width();
    let ph = window.physical_height();
    if let Some(target) = parse_aspect_ratio(&cfg.aspect_ratio) {
        let current = pw as f32 / ph as f32;
        if (current - target).abs() < 0.01 {
            // Close enough -- no bars needed
            (0, 0, pw, ph)
        } else if current > target {
            // Too wide -- pillarbox
            let w = (ph as f32 * target) as u32;
            let ox = (pw - w) / 2;
            (ox, 0, w, ph)
        } else {
            // Too tall -- letterbox
            let h = (pw as f32 / target) as u32;
            let oy = (ph - h) / 2;
            (0, oy, pw, h)
        }
    } else {
        (0, 0, pw, ph)
    }
}

/// Compute the logical letterboxed size (what the HUD camera sees).
pub(super) fn logical_size(window: &Window, cfg: &GameConfig) -> (f32, f32) {
    let sf = window.scale_factor();
    let (_, _, pw, ph) = letterbox_rect(window, cfg);
    (pw as f32 / sf, ph as f32 / sf)
}

/// Compute the 3D viewport rect in logical (CSS) pixels: (left, top, width, height).
/// This is the playable area -- letterboxed region minus HUD borders (matches camera viewport).
pub fn viewport_rect(window: &Window, cfg: &GameConfig, ui: &UiAssets) -> (f32, f32, f32, f32) {
    let sf = window.scale_factor();
    let (_, _, lpw, lph) = letterbox_rect(window, cfg);
    let lw = lpw as f32 / sf;
    let lh = lph as f32 / sf;
    let d = hud_dimensions(lw, lh, ui);
    let bar_x = (window.width() - lw) / 2.0;
    let bar_y = (window.height() - lh) / 2.0;
    let footer_exposed = d.scale_h(FOOTER_EXPOSED_H);
    let vp_left = bar_x;
    let vp_top = bar_y + d.border3_h;
    let vp_w = lw - d.border1_w;
    let vp_h = lh - d.border3_h - d.border2_h - footer_exposed;
    (vp_left, vp_top, vp_w, vp_h)
}

/// Update the 3D camera viewport to the letterboxed area minus HUD borders.
/// The HUD camera has no viewport (full window) and clears to black for letterbox bars.
pub(super) fn update_viewport(
    windows: Query<&Window, With<PrimaryWindow>>,
    cfg: Res<GameConfig>,
    ui_assets: Res<UiAssets>,
    mut player_cameras: Query<&mut Camera, With<PlayerCamera>>,
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

    for mut camera in player_cameras.iter_mut() {
        camera.viewport = Some(Viewport {
            physical_position: UVec2::new(vp_x, vp_y),
            physical_size: UVec2::new(vp_w, vp_h),
            ..default()
        });
    }
}
