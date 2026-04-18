//! Editor guide lines — persistent horizontal/vertical reference lines
//! that appear on all screens to show UI layout boundaries.

use bevy::prelude::*;
use bevy_inspector_egui::bevy_egui::egui;
use serde::{Deserialize, Serialize};

use crate::screens::{REF_H, REF_W};

/// Guide line orientation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum GuideAxis {
    Horizontal,
    Vertical,
}

/// A single guide line at a fixed position in reference coordinates (640x480).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuideLine {
    pub axis: GuideAxis,
    /// Position in reference pixels (x for vertical, y for horizontal).
    pub position: f32,
    #[serde(default = "default_label")]
    pub label: String,
}

fn default_label() -> String {
    String::new()
}

/// All editor guide lines — persisted in `openmm-editor.toml`.
#[derive(Resource, Default)]
pub struct Guides {
    pub lines: Vec<GuideLine>,
    /// Whether guides are visible on canvas.
    pub visible: bool,
}

impl Guides {
    /// Build from an already-loaded config (no disk I/O).
    pub fn from_config(cfg: &super::io::EditorConfig) -> Self {
        Guides {
            lines: cfg.guides.clone(),
            visible: true,
        }
    }

    /// Sync guide lines back to the config resource.
    pub fn sync_to_config(&self, cfg: &mut super::io::EditorConfig) {
        cfg.guides = self.lines.clone();
        cfg.mark_dirty();
    }
}

/// Guide line color — red dashed, easy to spot against dark backgrounds.
const GUIDE_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(220, 50, 50, 180);
const GUIDE_LABEL_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(220, 50, 50, 140);
const DASH_LEN: f32 = 6.0;
const GAP_LEN: f32 = 4.0;

/// Draw all guide lines on the egui painter. Called from `draw_overlays`.
pub fn draw_guides(painter: &egui::Painter, guides: &Guides, win_w: f32, win_h: f32) {
    if !guides.visible {
        return;
    }
    let stroke = egui::Stroke::new(1.0, GUIDE_COLOR);

    for guide in &guides.lines {
        match guide.axis {
            GuideAxis::Horizontal => {
                let y = guide.position / REF_H * win_h;
                draw_dashed_line(painter, egui::pos2(0.0, y), egui::pos2(win_w, y), stroke);
                if !guide.label.is_empty() {
                    painter.text(
                        egui::pos2(4.0, y - 12.0),
                        egui::Align2::LEFT_BOTTOM,
                        &guide.label,
                        egui::FontId::proportional(10.0),
                        GUIDE_LABEL_COLOR,
                    );
                }
            }
            GuideAxis::Vertical => {
                let x = guide.position / REF_W * win_w;
                draw_dashed_line(painter, egui::pos2(x, 0.0), egui::pos2(x, win_h), stroke);
                if !guide.label.is_empty() {
                    painter.text(
                        egui::pos2(x + 3.0, 4.0),
                        egui::Align2::LEFT_TOP,
                        &guide.label,
                        egui::FontId::proportional(10.0),
                        GUIDE_LABEL_COLOR,
                    );
                }
            }
        }
    }
}

/// Draw a dashed line between two points.
fn draw_dashed_line(painter: &egui::Painter, from: egui::Pos2, to: egui::Pos2, stroke: egui::Stroke) {
    let delta = to - from;
    let len = delta.length();
    if len < 1.0 {
        return;
    }
    let dir = delta / len;
    let segment = DASH_LEN + GAP_LEN;
    let mut t = 0.0;
    while t < len {
        let dash_end = (t + DASH_LEN).min(len);
        let p0 = from + dir * t;
        let p1 = from + dir * dash_end;
        painter.line_segment([p0, p1], stroke);
        t += segment;
    }
}

/// Draw guides section inline inside a parent `egui::Ui` (collapsible).
pub fn guides_section(ui: &mut egui::Ui, guides: &mut Guides, cfg: &mut super::io::EditorConfig) {
    ui.checkbox(&mut guides.visible, "Show guides");

    let mut to_remove = None;
    let mut changed = false;
    for (i, guide) in guides.lines.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            let axis_label = match guide.axis {
                GuideAxis::Horizontal => "H",
                GuideAxis::Vertical => "V",
            };
            ui.label(axis_label);
            if ui
                .add(egui::DragValue::new(&mut guide.position).speed(1.0).suffix("px"))
                .changed()
            {
                changed = true;
            }
            if ui
                .add(egui::TextEdit::singleline(&mut guide.label).desired_width(60.0))
                .changed()
            {
                changed = true;
            }
            if ui.small_button("x").clicked() {
                to_remove = Some(i);
            }
        });
    }
    if let Some(i) = to_remove {
        guides.lines.remove(i);
        changed = true;
    }

    ui.horizontal(|ui| {
        if ui.button("+ H").clicked() {
            guides.lines.push(GuideLine {
                axis: GuideAxis::Horizontal,
                position: REF_H / 2.0,
                label: String::new(),
            });
            changed = true;
        }
        if ui.button("+ V").clicked() {
            guides.lines.push(GuideLine {
                axis: GuideAxis::Vertical,
                position: REF_W / 2.0,
                label: String::new(),
            });
            changed = true;
        }
    });

    if changed {
        guides.sync_to_config(cfg);
    }
}
