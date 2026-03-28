//! Indoor (BLV) map spawner — the indoor equivalent of odm.rs.
//!
//! When the loading pipeline produces a `PreparedIndoorWorld`, this plugin
//! spawns the face-based geometry and ambient lighting.

use bevy::prelude::*;

use crate::game::InGame;
use crate::states::loading::PreparedIndoorWorld;
use crate::GameState;

pub struct BlvPlugin;

impl Plugin for BlvPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Game), spawn_indoor_world);
    }
}

fn spawn_indoor_world(
    prepared: Option<Res<PreparedIndoorWorld>>,
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(prepared) = prepared else { return };

    // Spawn all face meshes (grouped by texture)
    for model in &prepared.models {
        for sub in &model.sub_meshes {
            let mut mat = sub.material.clone();
            if let Some(ref tex) = sub.texture {
                let tex_handle = images.add(tex.clone());
                mat.base_color_texture = Some(tex_handle);
            }
            commands.spawn((
                Mesh3d(meshes.add(sub.mesh.clone())),
                MeshMaterial3d(materials.add(mat)),
                InGame,
            ));
        }
    }

    // Indoor ambient lighting: central point light for fill
    commands.spawn((
        PointLight {
            intensity: 1_000_000.0,
            range: 50_000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(0.0, 2000.0, 0.0),
        InGame,
    ));
}
