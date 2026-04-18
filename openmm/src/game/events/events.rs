//! Map event loading — shared between outdoor (ODM) and indoor (BLV) maps.

use bevy::prelude::{Assets as BevyAssets, *};

use crate::assets::GameAssets;

/// Base value added to the actor spawn index to form a synthetic npc_id for generated street NPCs.
/// npcdata.txt has ~400 entries, so any value above that avoids collisions with quest NPC ids.
/// This is an internal convention — MM6 has no equivalent; MM7 uses Game.StreetNPC + 5000.
pub const GENERATED_NPC_ID_BASE: i32 = 5000;

/// Parsed event data for the current map.
#[derive(Resource, Default)]
pub struct MapEvents {
    pub evt: Option<openmm_data::EvtFile>,
    pub houses: Option<openmm_data::TwoDEvents>,
    /// Global NPC metadata table (id → name, portrait, profession).
    /// Loaded from `npcdata.txt` in icons.lod; same for every map.
    pub npc_table: Option<openmm_data::StreetNpcs>,
    /// Name pool for generating street NPC names (from `npcnames.txt`).
    pub name_pool: Option<openmm_data::NpcNamePools>,
    /// Dynamically generated NPCs for peasant actors (npc_id ≥ GENERATED_NPC_ID_BASE).
    /// Populated at actor spawn time; keyed by the assigned npc_id.
    pub generated_npcs: std::collections::HashMap<i32, openmm_data::GeneratedNpc>,
}

/// Map building type string → screen .ron name for the overlay system.
pub fn building_screen_for_type(building_type: &str) -> &'static str {
    let lower = building_type.to_lowercase();
    if lower.contains("weapon") {
        return "weapon_shop";
    }
    if lower.contains("armor") {
        return "armor_shop";
    }
    if lower.contains("magic") || lower.contains("alchemy") {
        return "magic_shop";
    }
    if lower.contains("general") || lower.contains("store") {
        return "general_store";
    }
    if lower.contains("tavern") {
        return "tavern";
    }
    if lower.contains("temple") && !lower.contains("ent") {
        return "temple";
    }
    if lower.contains("training") {
        return "training";
    }
    if lower.contains("guild") {
        return "guild";
    }
    if lower.contains("bank") {
        return "bank";
    }
    if lower.contains("stables") || lower.contains("boats") || lower.contains("wagon") {
        return "travel";
    }
    if lower.contains("town hall") || lower.contains("city council") {
        return "town_hall";
    }
    if lower.contains("jail") {
        return "jail";
    }
    if lower.contains("library") || lower.contains("oracle") || lower.contains("seer") {
        return "library";
    }
    if lower.contains("circus") || lower.contains("tent") {
        return "circus";
    }
    if lower.contains("throne") || lower.contains("castle") && !lower.contains("ent") {
        return "throne";
    }
    if lower.contains("house") || lower.contains("hosue") || lower.contains("hermit") {
        return "house";
    }
    // Dungeon/castle/pyramid/hive entrances — transition, not a shop UI.
    if lower.contains("ent") {
        return "building";
    }
    "building"
}

/// Map building type string → fallback background image name.
fn building_background(building_type: &str) -> &'static str {
    let lower = building_type.to_lowercase();
    if lower.contains("weapon") {
        return "wepntabl";
    }
    if lower.contains("armor") {
        return "armory";
    }
    if lower.contains("magic") || lower.contains("guild") || lower.contains("alchemy") {
        return "magshelf";
    }
    if lower.contains("general") || lower.contains("store") {
        return "genshelf";
    }
    "evt02"
}

/// Resolve the background image handle for a building interaction (SpeakInHouse).
/// Tries the house's picture_id first, falls back to building_type, then "evt02".
pub fn resolve_building_image(
    house_id: u32,
    map_events: &MapEvents,
    game_assets: &GameAssets,
    images: &mut BevyAssets<Image>,
) -> Option<bevy::asset::Handle<Image>> {
    if let Some(houses) = map_events.houses.as_ref()
        && let Some(entry) = houses.houses.get(&house_id)
    {
        let pic_name = format!("evt{:02}", entry.picture_id);
        if let Some(handle) = game_assets.load_icon(&pic_name, images) {
            return Some(handle);
        }
        return game_assets.load_icon(building_background(&entry.building_type), images);
    }
    game_assets.load_icon("evt02", images)
}

/// Load event data for a map and insert the MapEvents resource.
/// `map_base` is the map filename stem without extension, e.g. "oute3" or "d01".
/// `indoor` controls whether to skip loading 2devents.txt (only relevant for outdoor maps).
pub fn load_map_events(commands: &mut Commands, game_assets: &GameAssets, map_base: &str, indoor: bool) {
    let mut evt = match openmm_data::EvtFile::parse(game_assets.assets(), map_base) {
        Ok(e) => {
            info!("Loaded {}.evt: {} events", map_base, e.events.len());
            Some(e)
        }
        Err(e) => {
            warn!("Failed to load {}.evt: {}", map_base, e);
            None
        }
    };

    // Merge global.evt events (map-independent global events)
    match openmm_data::EvtFile::parse(game_assets.assets(), "global") {
        Ok(global) => {
            info!("Loaded global.evt: {} events", global.events.len());
            if let Some(ref mut map_evt) = evt {
                for (id, actions) in global.events {
                    map_evt.events.entry(id).or_default().extend(actions);
                }
            } else {
                evt = Some(global);
            }
        }
        Err(e) => {
            debug!("No global.evt: {}", e);
        }
    }
    let houses = if indoor {
        None
    } else {
        match openmm_data::TwoDEvents::parse(game_assets.assets()) {
            Ok(h) => {
                info!("Loaded 2devents.txt: {} houses", h.houses.len());
                Some(h)
            }
            Err(e) => {
                warn!("Failed to load 2devents.txt: {}", e);
                None
            }
        }
    };
    if let Some(ref e) = evt {
        let mut ids: Vec<_> = e.events.keys().collect();
        ids.sort();
        debug!("EVT event_ids: {:?}", ids);
        for &id in &ids {
            if let Some(actions) = e.events.get(id) {
                debug!("  event[{}]: {:?}", id, actions);
            }
        }
    }
    let npc_table = game_assets.data().street_npcs.clone();
    let name_pool = game_assets.data().name_pool.clone();

    commands.insert_resource(MapEvents {
        evt,
        houses,
        npc_table,
        name_pool,
        generated_npcs: Default::default(),
    });
}
