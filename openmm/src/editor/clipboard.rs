use super::canvas::{EditorScreen, resolve_elem_size};
use crate::screens::ui_assets::UiAssets;
use crate::screens::{REF_H, REF_W, ScreenElement};
use bevy::prelude::*;

/// Simple internal clipboard for copying and pasting editor elements.
#[derive(Resource, Default)]
pub struct Clipboard(pub Option<ScreenElement>);

impl Clipboard {
    pub fn copy(&mut self, elem: &ScreenElement) {
        self.0 = Some(elem.clone());
        info!("editor: copied element '{}'", elem.id());
    }

    /// Paste the current clipboard element into the screen at the given mouse position.
    /// Returns the newly pasted element index if successful.
    pub fn paste(&self, editor: &mut EditorScreen, mouse_ref: Vec2, ui_assets: &UiAssets) -> Option<usize> {
        let base_elem = self.0.as_ref()?;
        let mut new_elem = base_elem.clone();

        let (w, h) = resolve_elem_size(&new_elem, ui_assets);

        // Center on mouse position and clamp to screen boundaries.
        let x = (mouse_ref.x - w / 2.0).round().clamp(0.0, (REF_W - w).max(0.0));
        let y = (mouse_ref.y - h / 2.0).round().clamp(0.0, (REF_H - h).max(0.0));
        new_elem.set_position((x, y));

        // Ensure unique ID by appending _copy and incrementing index.
        let base_id = new_elem.id().to_string();
        let prefix = if base_id.contains("_copy") {
            base_id.clone()
        } else {
            format!("{}_copy", base_id)
        };

        let mut attempt = prefix.clone();
        let mut counter = 1;
        while editor.screen.elements.iter().any(|e| e.id() == attempt) {
            attempt = format!("{}_{}", prefix, counter);
            counter += 1;
        }

        match &mut new_elem {
            ScreenElement::Image(e) => e.id = attempt,
            ScreenElement::Video(e) => e.id = attempt,
            ScreenElement::Text(e) => e.id = attempt,
        }

        editor.screen.elements.push(new_elem);
        editor.dirty = true;

        let new_idx = editor.screen.elements.len() - 1;
        info!("editor: pasted element '{}'", editor.screen.elements[new_idx].id());

        Some(new_idx)
    }
}
