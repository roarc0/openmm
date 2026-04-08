//! `spawn_world` orchestrates the one-time outdoor map spawn at `OnEnter(Game)`.
//!
//! Builds the terrain entity, parents BSP buildings under it, computes the
//! distance-sorted spawn order for decorations / actors / monsters, then hands
//! off to [`super::lazy_spawn`] for time-budgeted per-frame spawning.

use bevy::prelude::*;

use crate::game::optional::OptionalWrite;
use crate::game::sprites::material::SpriteMaterial;
use crate::states::loading::PreparedWorld;

use super::bsp;
use super::spawn_terrain;

use super::lazy_spawn::{PendingSpawns, SpawnProgress, sort_by_distance_mm6};

pub(super) fn spawn_world(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    _sprite_materials: Option<ResMut<Assets<SpriteMaterial>>>,
    mut terrain_materials: Option<ResMut<Assets<spawn_terrain::TerrainMaterial>>>,
    mut prepared: Option<ResMut<PreparedWorld>>,
    save_data: Res<crate::save::GameSave>,
    cfg: Res<crate::config::GameConfig>,
    mut music_events: Option<bevy::ecs::message::MessageWriter<crate::game::sound::music::PlayMusicEvent>>,
) {
    let Some(prepared) = prepared.as_mut() else {
        // No outdoor PreparedWorld — this is an indoor map, skip outdoor spawning
        return;
    };

    let (terrain_tex_handle, water_tex_handle, water_mask_handle) =
        spawn_terrain::prepare_terrain_textures(prepared, &mut images, &cfg);

    let terrain_entity_id = spawn_terrain::spawn_terrain(
        &mut commands,
        &mut meshes,
        &mut materials,
        terrain_materials.as_deref_mut(),
        prepared.terrain_mesh.clone(),
        terrain_tex_handle,
        water_tex_handle,
        water_mask_handle,
    );

    bsp::spawn_bsp_models(
        &mut commands,
        terrain_entity_id,
        prepared,
        &mut meshes,
        &mut materials,
        &mut images,
        &cfg,
    );

    let player_spawn = Vec3::new(
        save_data.player.position[0],
        save_data.player.position[1],
        save_data.player.position[2],
    );
    let orders = compute_all_sprites_spawns(prepared, player_spawn);

    music_events.try_write(crate::game::sound::music::PlayMusicEvent {
        track: prepared.music_track,
        volume: cfg.music_volume,
    });

    let total = orders.billboard.len() + orders.actor.len() + orders.monster.len();
    commands.insert_resource(SpawnProgress { total, done: 0 });
    commands.insert_resource(PendingSpawns {
        billboard_order: orders.billboard,
        actor_order: orders.actor,
        idx: 0,
        frames_elapsed: 0,
        sprite_cache: prepared.sprite_cache.clone(),
        dec_sprite_cache: prepared.dec_sprite_cache.clone(),
        decorations: prepared.decorations.clone(),
        actors: prepared.resolved_actors.take(),
        monsters: prepared.resolved_monsters.take(),
        monster_order: orders.monster,
        terrain_entity: terrain_entity_id,
    });

    bsp::spawn_bsp_clickable_faces(&mut commands, prepared);
}

/// Distance-sorted spawn orders for the three entity kinds.
struct SpawnSprites {
    billboard: Vec<usize>,
    actor: Vec<usize>,
    monster: Vec<usize>,
}

fn compute_all_sprites_spawns(prepared: &PreparedWorld, player_spawn: Vec3) -> SpawnSprites {
    let billboard = sort_by_distance_mm6(
        prepared.decorations.entries(),
        player_spawn,
        |d| d.position[0] as f32,
        |d| d.position[1] as f32,
    );

    let actor = prepared
        .resolved_actors
        .as_ref()
        .map(|a| {
            sort_by_distance_mm6(
                a.get_actors(),
                player_spawn,
                |actor| actor.position[0] as f32,
                |actor| actor.position[1] as f32,
            )
        })
        .unwrap_or_default();

    let monster = prepared
        .resolved_monsters
        .as_ref()
        .map(|m| {
            sort_by_distance_mm6(
                m.entries(),
                player_spawn,
                |mon| mon.spawn_position[0] as f32,
                |mon| mon.spawn_position[1] as f32,
            )
        })
        .unwrap_or_default();

    SpawnSprites {
        billboard,
        actor,
        monster,
    }
}
