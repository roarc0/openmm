//! Map event loading — shared between outdoor (ODM) and indoor (BLV) maps.

use bevy::prelude::*;

use crate::assets::GameAssets;

/// Parsed event data for the current map.
#[derive(Resource, Default)]
pub struct MapEvents {
    pub evt: Option<lod::evt::EvtFile>,
    pub houses: Option<lod::twodevents::TwoDEvents>,
}

/// Load event data for a map and insert the MapEvents resource.
/// `map_base` is the map filename stem without extension, e.g. "oute3" or "d01".
pub fn load_map_events(commands: &mut Commands, game_assets: &GameAssets, map_base: &str) {
    let evt = match lod::evt::EvtFile::parse(game_assets.lod_manager(), map_base) {
        Ok(e) => {
            info!("Loaded {}.evt: {} events", map_base, e.events.len());
            Some(e)
        }
        Err(e) => {
            warn!("Failed to load {}.evt: {}", map_base, e);
            None
        }
    };
    let houses = match lod::twodevents::TwoDEvents::parse(game_assets.lod_manager()) {
        Ok(h) => {
            info!("Loaded 2devents.txt: {} houses", h.houses.len());
            Some(h)
        }
        Err(e) => {
            warn!("Failed to load 2devents.txt: {}", e);
            None
        }
    };
    if let Some(ref e) = evt {
        let mut ids: Vec<_> = e.events.keys().collect();
        ids.sort();
        info!("EVT event_ids: {:?}", ids);
    }
    commands.insert_resource(MapEvents { evt, houses });
}
