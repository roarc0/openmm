use bevy::prelude::*;
use std::collections::HashMap;

use crate::assets::GameAssets;

/// Cached UI texture handles loaded from LOD archives.
#[derive(Resource, Default)]
pub struct UiAssets {
    textures: HashMap<String, Handle<Image>>,
}

impl UiAssets {
    /// Load a UI texture by name from the LOD icons archive.
    /// Handles both PCX and custom bitmap formats.
    /// Caches the result — subsequent calls return the cached handle.
    pub fn get_or_load(
        &mut self,
        name: &str,
        game_assets: &GameAssets,
        images: &mut Assets<Image>,
    ) -> Option<Handle<Image>> {
        if let Some(handle) = self.textures.get(name) {
            return Some(handle.clone());
        }
        let img = game_assets.lod_manager().icon(name)?;
        let bevy_img = crate::assets::dynamic_to_bevy_image(img);
        let handle = images.add(bevy_img);
        self.textures.insert(name.to_string(), handle.clone());
        Some(handle)
    }
}
