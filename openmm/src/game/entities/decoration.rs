use bevy::prelude::*;

use crate::game::entities::{Billboard, EntityKind, WorldEntity};
use crate::states::loading::PreparedWorld;

/// Spawn static decoration billboards (trees, rocks, fountains, etc.)
/// from the prepared billboard data.
pub fn spawn_decorations(
    parent: &mut ChildSpawnerCommands,
    prepared: &PreparedWorld,
    images: &mut ResMut<Assets<Image>>,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    for bb in &prepared.billboards {
        let tex_handle = images.add(bb.image.clone());
        let bb_mat = materials.add(StandardMaterial {
            base_color_texture: Some(tex_handle),
            alpha_mode: AlphaMode::Mask(0.5),
            cull_mode: None,
            double_sided: true,
            unlit: true,
            ..default()
        });
        let quad = meshes.add(Rectangle::new(bb.width, bb.height));
        // Position with bottom at the given world position
        let pos = bb.position + Vec3::new(0.0, bb.height / 2.0, 0.0);

        parent.spawn((
            Name::new("decoration"),
            Mesh3d(quad),
            MeshMaterial3d(bb_mat),
            Transform::from_translation(pos),
            WorldEntity,
            EntityKind::Decoration,
            Billboard,
        ));
    }
}
