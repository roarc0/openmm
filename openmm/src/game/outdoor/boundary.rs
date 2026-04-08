//! Detect when the player crosses the play-area boundary and queue an adjacent map load.

use bevy::prelude::*;

use crate::GameState;

/// Half-size of the playable area in world units.
const PLAY_BOUNDARY: f32 = openmm_data::odm::ODM_TILE_SCALE * openmm_data::odm::ODM_PLAY_SIZE as f32 / 2.0;
/// Full playable area width (used to translate player position to new map).
pub const PLAY_WIDTH: f32 = openmm_data::odm::ODM_TILE_SCALE * openmm_data::odm::ODM_PLAY_SIZE as f32;

pub(super) fn check_map_boundary(
    mut commands: Commands,
    mut world_state: ResMut<crate::game::world_state::WorldState>,
    mut save_data: ResMut<crate::save::GameSave>,
    mut game_state: ResMut<NextState<GameState>>,
    player_query: Query<&Transform, With<crate::game::player::Player>>,
    load_request: Option<Res<crate::states::loading::LoadRequest>>,
) {
    // Don't trigger boundary crossing if a map transition is already queued
    if load_request.is_some() {
        debug!("check_map_boundary: skipped (LoadRequest exists)");
        return;
    }
    let Ok(transform) = player_query.single() else { return };
    let openmm_data::utils::MapName::Outdoor(ref odm) = world_state.map.name else {
        debug!(
            "check_map_boundary: skipped (not outdoor map: {:?})",
            world_state.map.name
        );
        return;
    };
    let pos = transform.translation;
    let (yaw, _, _) = transform.rotation.to_euler(EulerRot::YXZ);

    // Check which boundary was crossed (Bevy: +X=east, -X=west, -Z=north, +Z=south)
    let (new_odm, new_x, new_z) = if pos.x > PLAY_BOUNDARY {
        // East edge → load eastern map, player appears at western edge
        (odm.go_east(), pos.x - PLAY_WIDTH, pos.z)
    } else if pos.x < -PLAY_BOUNDARY {
        // West edge → load western map, player appears at eastern edge
        (odm.go_west(), pos.x + PLAY_WIDTH, pos.z)
    } else if pos.z < -PLAY_BOUNDARY {
        // North edge (Bevy -Z = MM6 +Y = north)
        (odm.go_north(), pos.x, pos.z + PLAY_WIDTH)
    } else if pos.z > PLAY_BOUNDARY {
        // South edge (Bevy +Z = MM6 -Y = south)
        (odm.go_south(), pos.x, pos.z - PLAY_WIDTH)
    } else {
        return; // Still inside playable area
    };

    let Some(new_odm) = new_odm else {
        return; // No adjacent map (edge of the world grid)
    };

    info!("Map transition: {} → {}", world_state.map.name, new_odm);

    // Update world state and save data for the new map
    world_state.map.name = openmm_data::utils::MapName::Outdoor(new_odm.clone());
    world_state.map.map_x = new_odm.x;
    world_state.map.map_y = new_odm.y;
    world_state.player.position = Vec3::new(new_x, pos.y, new_z);
    world_state.player.yaw = yaw;
    world_state.write_to_save(&mut save_data);

    commands.insert_resource(crate::states::loading::LoadRequest {
        map_name: world_state.map.name.clone(),
        spawn_position: None,
        spawn_yaw: None,
    });
    game_state.set(GameState::Loading);
}
