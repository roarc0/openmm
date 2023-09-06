use bevy::prelude::*;

use lod::{
    odm::{ODM_HEIGHT_SCALE, ODM_PLAY_SIZE, ODM_TILE_SCALE},
    LodManager,
};

use crate::{
    despawn_all,
    odm::{OdmName, OdmPlugin},
    player::{MovementSettings, PlayerPlugin},
    GameState,
};

use self::{sky::SkyPlugin, sun::SunPlugin};

pub(crate) mod sky;
pub(crate) mod sun;

#[derive(Component)]
pub(super) struct InWorld;

#[derive(Resource)]
pub(super) struct WorldSettings {
    pub lod_manager: LodManager,
    pub current_odm: OdmName,
    pub odm_changed: bool,
}

impl Default for WorldSettings {
    fn default() -> Self {
        Self {
            lod_manager: LodManager::new(lod::get_lod_path()).expect("unable to load lod files"),
            current_odm: OdmName::default(),
            odm_changed: true,
        }
    }
}

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldSettings>()
            .insert_resource(MovementSettings {
                max_xz: ODM_TILE_SCALE * ODM_PLAY_SIZE as f32 / 2.0,
                max_y: ODM_TILE_SCALE * ODM_HEIGHT_SCALE / 2.0,
                ..Default::default()
            })
            .add_plugins((PlayerPlugin, SunPlugin, SkyPlugin, OdmPlugin))
            .add_systems(OnExit(GameState::Game), despawn_all::<InWorld>);
    }
}
