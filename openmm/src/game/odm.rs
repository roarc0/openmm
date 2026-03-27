use bevy::prelude::*;
use bevy::render::render_resource::Face;

use crate::GameState;
use crate::game::InGame;
use crate::game::player::PlayerCamera;
use crate::states::loading::PreparedWorld;

/// Marker for billboard entities that should face the camera.
#[derive(Component)]
struct BillboardMarker;

/// Grid coordinate for outdoor maps. Columns a-e, rows 1-3.
#[derive(Clone)]
pub struct OdmName {
    pub x: char,
    pub y: char,
}

impl Default for OdmName {
    fn default() -> Self {
        Self { x: 'e', y: '3' }
    }
}

use std::fmt::Display;

impl Display for OdmName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(Self::map_name(self.x, self.y).as_str())
    }
}

impl TryFrom<&str> for OdmName {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let x = value.as_bytes().get(3).copied().ok_or("invalid map name")? as char;
        let y = value.as_bytes().get(4).copied().ok_or("invalid map name")? as char;

        let x = Self::validate_x(x).ok_or("invalid map x coordinate")?;
        let y = Self::validate_y(y).ok_or("invalid map y coordinate")?;

        Ok(Self { x, y })
    }
}

impl OdmName {
    pub fn go_north(&self) -> Option<OdmName> {
        let y = Self::validate_y((self.y as u8 - 1) as char)?;
        Some(Self { x: self.x, y })
    }

    pub fn go_west(&self) -> Option<OdmName> {
        let x = Self::validate_x((self.x as u8 - 1) as char)?;
        Some(Self { x, y: self.y })
    }

    pub fn go_south(&self) -> Option<OdmName> {
        let y = Self::validate_y((self.y as u8 + 1) as char)?;
        Some(Self { x: self.x, y })
    }

    pub fn go_east(&self) -> Option<OdmName> {
        let x = Self::validate_x((self.x as u8 + 1) as char)?;
        Some(Self { x, y: self.y })
    }

    fn map_name(x: char, y: char) -> String {
        format!("out{}{}.odm", x, y)
    }

    fn validate_x(c: char) -> Option<char> {
        match c {
            'a'..='e' => Some(c),
            _ => None,
        }
    }
    fn validate_y(c: char) -> Option<char> {
        match c {
            '1'..='3' => Some(c),
            _ => None,
        }
    }
}

pub struct OdmPlugin;

impl Plugin for OdmPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Game), spawn_world)
            .add_systems(
                Update,
                billboard_face_camera.run_if(in_state(GameState::Game)),
            );
    }
}

/// Rotate all billboards to face the camera (Y-axis only).
fn billboard_face_camera(
    camera_query: Query<&GlobalTransform, With<PlayerCamera>>,
    mut billboard_query: Query<(&mut Transform, &GlobalTransform), With<BillboardMarker>>,
) {
    let Ok(camera_gt) = camera_query.single() else {
        return;
    };
    let cam_pos = camera_gt.translation();

    for (mut transform, global_transform) in billboard_query.iter_mut() {
        let bb_pos = global_transform.translation();
        let dir = cam_pos - bb_pos;
        // Only rotate around Y axis (billboard stays upright)
        if dir.x.abs() > 0.01 || dir.z.abs() > 0.01 {
            let angle = dir.x.atan2(dir.z);
            transform.rotation = Quat::from_rotation_y(angle);
        }
    }
}

fn spawn_world(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    prepared: Option<Res<PreparedWorld>>,
) {
    let Some(prepared) = prepared else {
        error!("No PreparedWorld available when entering Game state");
        return;
    };

    let terrain_texture = prepared.terrain_texture.clone();
    let terrain_mesh = prepared.terrain_mesh.clone();

    let image_handle = images.add(terrain_texture);
    let material = StandardMaterial {
        base_color: Color::srgb(0.85, 0.85, 0.85),
        base_color_texture: Some(image_handle),
        unlit: false,
        alpha_mode: AlphaMode::Opaque,
        perceptual_roughness: 1.0,
        reflectance: 0.0,
        metallic: 0.0,
        cull_mode: Some(Face::Back),
        double_sided: false,
        ..default()
    };

    commands
        .spawn((
            Name::new("odm"),
            Mesh3d(meshes.add(terrain_mesh)),
            MeshMaterial3d(materials.add(material)),
            InGame,
        ))
        .with_children(|parent| {
            for model in &prepared.models {
                for sub in &model.sub_meshes {
                    let mut mat = sub.material.clone();
                    if let Some(ref tex) = sub.texture {
                        let tex_handle = images.add(tex.clone());
                        mat.base_color_texture = Some(tex_handle);
                    }
                    parent.spawn((
                        Name::new("model"),
                        Mesh3d(meshes.add(sub.mesh.clone())),
                        MeshMaterial3d(materials.add(mat)),
                    ));
                }
            }

            // Spawn billboards as camera-facing quads
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
                // Position billboard with bottom at the given position
                let pos = bb.position + Vec3::new(0.0, bb.height / 2.0, 0.0);
                parent.spawn((
                    Name::new("billboard"),
                    Mesh3d(quad),
                    MeshMaterial3d(bb_mat),
                    Transform::from_translation(pos),
                    BillboardMarker,
                ));
            }
        });
}
