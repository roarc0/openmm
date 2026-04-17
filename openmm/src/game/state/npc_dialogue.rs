use bevy::prelude::*;

use crate::assets::GameAssets;
use crate::game::state::{GENERATED_NPC_ID_BASE, MapEvents};

/// Resource holding an NPC portrait image to display at actual size.
#[derive(Resource)]
pub struct NpcPortrait {
    pub image: Handle<Image>,
    /// Natural pixel size of the portrait.
    pub size: Vec2,
}

/// Resource holding the NPC name and all profession data for display under the portrait.
#[derive(Resource, Default)]
pub struct NpcProfile {
    pub name: String,
    pub profession: Option<String>,
    /// Greeting line resolved from npcbtb based on the NPC's current greeting_id.
    pub greeting_text: Option<String>,
    /// Today's topic from proftext (e.g. "Magic Weapons").
    pub day_topic: Option<String>,
    /// Today's dialogue text from proftext.
    pub day_text: Option<String>,
    pub join_text: Option<String>,
    pub in_party_benefit: Option<String>,
    pub cost_per_week: Option<u32>,
    pub personality: Option<String>,
    pub action_text: Option<String>,
}

/// Resolve and load all data needed to display an NPC dialogue.
///
/// Returns `(NpcPortrait, NpcProfile)` ready to insert as resources, or `None` if the
/// portrait image could not be loaded. Handles both generated street NPCs and quest NPCs.
///
/// `day_of_week`: 0 = Sunday … 6 = Saturday (proftext index).
/// `npc_greetings`: from `world_state.game_vars.npc_greetings`.
pub fn prepare_npc_dialogue(
    npc_id: i32,
    map_events: &Option<bevy::ecs::system::Res<'_, MapEvents>>,
    game_assets: &GameAssets,
    images: &mut Assets<Image>,
    day_of_week: u32,
    npc_greetings: &std::collections::HashMap<i32, i32>,
) -> Option<(NpcPortrait, NpcProfile)> {
    let (portrait_name, display_name) = if npc_id >= GENERATED_NPC_ID_BASE {
        let entry = map_events.as_ref().and_then(|me| me.generated_npcs.get(&npc_id));
        if entry.is_none() {
            warn!(
                "SpeakNPC: generated_npcs miss for npc_id={} (expected GENERATED_NPC_ID_BASE+actor_idx)",
                npc_id
            );
        }
        let portrait = entry
            .map(|g| format!("NPC{:03}", g.portrait))
            .unwrap_or_else(|| format!("NPC{:03}", npc_id));
        let name = entry.map(|g| g.name.clone());
        (portrait, name)
    } else {
        let portrait = map_events
            .as_ref()
            .and_then(|me| me.npc_table.as_ref())
            .and_then(|t| t.portrait_name(npc_id))
            .unwrap_or_else(|| format!("NPC{:03}", npc_id));
        let name = map_events
            .as_ref()
            .and_then(|me| me.npc_table.as_ref())
            .and_then(|t| t.npc_name(npc_id).map(str::to_string));
        (portrait, name)
    };

    info!(
        "SpeakNPC: npc_id={} portrait='{}' name={:?}",
        npc_id, portrait_name, display_name
    );

    let profession_id = if npc_id >= GENERATED_NPC_ID_BASE {
        map_events
            .as_ref()
            .and_then(|me| me.generated_npcs.get(&npc_id))
            .filter(|g| g.profession_id > 0)
            .map(|g| g.profession_id as u16)
    } else {
        map_events
            .as_ref()
            .and_then(|me| me.npc_table.as_ref())
            .and_then(|t| t.get(npc_id))
            .filter(|e| e.profession_id > 0)
            .map(|e| e.profession_id as u16)
    };
    let prof_entry = profession_id.and_then(|id| game_assets.data().prof_table.as_ref()?.get(id));

    let portrait_img = game_assets
        .lod()
        .icon(&portrait_name)
        .or_else(|| game_assets.lod().icon("npc001"))?;

    let size = Vec2::new(portrait_img.width() as f32, portrait_img.height() as f32);
    let portrait = NpcPortrait {
        image: game_assets
            .load_icon(&portrait_name, images)
            .or_else(|| game_assets.load_icon("npc001", images))?,
        size,
    };

    // Greeting text from npcbtb: look up greeting_id, match personality code to NPC type column.
    let greeting_text = (|| -> Option<String> {
        let greeting_id = *npc_greetings.get(&npc_id)? as usize;
        if greeting_id == 0 {
            return None;
        }
        let personality = prof_entry.map(|p| p.personality.as_str()).unwrap_or("");
        let btb = game_assets.npcbtb()?;
        let npc_type_idx = if personality.is_empty() {
            0
        } else {
            btb.npc_types
                .iter()
                .position(|t| t.name.to_ascii_uppercase().contains(&personality.to_ascii_uppercase()))
                .unwrap_or(0)
        };
        let text = btb.message(npc_type_idx, greeting_id)?;
        if text.is_empty() { None } else { Some(text.to_string()) }
    })();

    // Day-of-week dialogue from proftext (0=Sunday … 6=Saturday).
    let (day_topic, day_text) = (|| -> Option<(String, String)> {
        let pid = profession_id?;
        let day = game_assets.proftext()?.get(pid)?.day(day_of_week as usize)?;
        let topic = if day.topic.is_empty() {
            None
        } else {
            Some(day.topic.clone())
        }?;
        let text = if day.text.is_empty() {
            None
        } else {
            Some(day.text.clone())
        }?;
        Some((topic, text))
    })()
    .map_or((None, None), |(t, d)| (Some(t), Some(d)));

    let first_name = display_name
        .as_deref()
        .and_then(|n| n.split_whitespace().next())
        .unwrap_or_default()
        .to_string();
    let profile = NpcProfile {
        name: first_name,
        profession: prof_entry.map(|p| p.name.clone()),
        greeting_text,
        day_topic,
        day_text,
        join_text: prof_entry.map(|p| p.join_text.clone()).filter(|s| !s.is_empty()),
        in_party_benefit: prof_entry.map(|p| p.in_party_benefit.clone()).filter(|s| !s.is_empty()),
        cost_per_week: prof_entry.map(|p| p.cost_per_week).filter(|&c| c > 0),
        personality: prof_entry.map(|p| p.personality.clone()).filter(|s| !s.is_empty()),
        action_text: prof_entry.map(|p| p.action_text.clone()).filter(|s| !s.is_empty()),
    };

    Some((portrait, profile))
}
