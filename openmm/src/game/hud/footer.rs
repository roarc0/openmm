use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use super::UiAssets;
use crate::config::GameConfig;
use crate::fonts::GameFonts;

use super::borders::viewport_rect;

#[derive(Component)]
pub(super) struct HudFooter;
#[derive(Component)]
pub(super) struct HudFooterText;

/// Text displayed in the footer bar. Write to this resource from any system
/// to update the footer message (e.g. building names, hints, status text).
///
/// Set `text` to change the message. The HUD will re-render automatically.
/// Set `text` to an empty string to clear the footer.
///
/// # Example
/// ```ignore
/// fn my_system(mut footer: ResMut<FooterText>) {
///     footer.set("The Knife Shoppe");
/// }
/// ```
#[derive(Resource)]
pub struct FooterText {
    text: String,
    color: [u8; 4],
    font: String,
    /// Generation counter -- bumped on every change so the HUD knows to re-render.
    pub(super) generation: u64,
    /// If set, this text is "locked" until the timer expires.
    /// Hover hints won't overwrite locked text.
    lock_until: Option<f64>,
}

impl Default for FooterText {
    fn default() -> Self {
        Self {
            text: String::new(),
            color: crate::fonts::WHITE,
            font: "smallnum".into(),
            generation: 0,
            lock_until: None,
        }
    }
}

impl FooterText {
    /// Set footer text with the default font and color.
    /// This is a "soft" set — won't overwrite locked (status) text.
    pub fn set(&mut self, text: &str) {
        if self.lock_until.is_some() {
            return; // locked by status text, ignore hover hints
        }
        if self.text != text {
            self.text = text.to_string();
            self.generation += 1;
        }
    }

    /// Set footer text that persists for `duration` seconds.
    /// Cannot be overwritten by hover hints until it expires.
    pub fn set_status(&mut self, text: &str, duration: f64, now: f64) {
        self.text = text.to_string();
        self.lock_until = Some(now + duration);
        self.generation += 1;
    }

    /// Call every frame to expire locked text.
    pub fn tick(&mut self, now: f64) {
        if let Some(until) = self.lock_until
            && now >= until
        {
            self.lock_until = None;
            self.text.clear();
            self.generation += 1;
        }
    }

    /// Set footer text with a specific color.
    pub fn set_colored(&mut self, text: &str, color: [u8; 4]) {
        let changed = self.text != text || self.color != color;
        if changed {
            self.text = text.to_string();
            self.color = color;
            self.generation += 1;
        }
    }

    /// Set footer text with a specific font and color.
    pub fn set_styled(&mut self, text: &str, font: &str, color: [u8; 4]) {
        let changed = self.text != text || self.color != color || self.font != font;
        if changed {
            self.text = text.to_string();
            self.font = font.to_string();
            self.color = color;
            self.generation += 1;
        }
    }

    /// Clear the footer text.
    pub fn clear(&mut self) {
        self.set("");
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}

/// Re-render the footer text image whenever `FooterText` changes.
pub(super) fn update_footer_text(
    footer: Res<FooterText>,
    mut last_gen: Local<u64>,
    game_fonts: Res<GameFonts>,
    ui_assets: Res<UiAssets>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cfg: Res<GameConfig>,
    mut images: ResMut<Assets<Image>>,
    mut query: Query<(&mut ImageNode, &mut Visibility, &mut Node), With<HudFooterText>>,
) {
    if footer.generation == *last_gen {
        return;
    }
    *last_gen = footer.generation;

    // Compute viewport width for centering
    let vp_w = windows.single().ok().map(|w| {
        let (_, _, vp_w, _) = viewport_rect(w, &cfg, &ui_assets);
        vp_w
    });

    for (mut img_node, mut vis, mut node) in query.iter_mut() {
        if footer.text.is_empty() {
            *vis = Visibility::Hidden;
        } else if let Some(handle) = game_fonts.render(&footer.text, &footer.font, footer.color, &mut images) {
            // Center the text: account for scaling (height constrains the image,
            // width scales proportionally)
            let text_px_w = game_fonts.measure(&footer.text, &footer.font) as f32;
            if let Some(vp_w) = vp_w
                && let Some(font) = game_fonts.get(&footer.font)
            {
                let native_h = font.height as f32;
                if let Val::Px(display_h) = node.height {
                    let scale = display_h / native_h;
                    let display_w = text_px_w * scale;
                    node.left = Val::Px((vp_w - display_w) / 2.0);
                }
            }
            img_node.image = handle;
            *vis = Visibility::Inherited;
        }
    }
}
