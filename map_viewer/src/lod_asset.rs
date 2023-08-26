use std::path::{Path, PathBuf};

use bevy::{
    asset::{AssetIo, AssetIoError, ChangeWatcher},
    prelude::{info, unwrap, App, AssetPlugin, AssetServer, Plugin},
    utils::{tracing::Metadata, BoxedFuture},
};

use lod::{get_lod_path, LodManager};

struct LodAssetIo {
    default_io: Box<dyn AssetIo>,
    lod_manager: LodManager,
}

impl AssetIo for LodAssetIo {
    fn load_path<'a>(&'a self, path: &'a Path) -> BoxedFuture<'a, Result<Vec<u8>, AssetIoError>> {
        //if let Some(data) = self.lod_manager.try_get_bytes(path) {}

        self.default_io.load_path(path)
    }

    fn read_directory(
        &self,
        path: &Path,
    ) -> Result<Box<dyn Iterator<Item = PathBuf>>, AssetIoError> {
        self.default_io.read_directory(path)
    }

    fn watch_path_for_changes(
        &self,
        to_watch: &Path,
        to_reload: Option<PathBuf>,
    ) -> Result<(), AssetIoError> {
        self.default_io.watch_path_for_changes(to_watch, to_reload)
    }

    fn watch_for_changes(&self, configuration: &ChangeWatcher) -> Result<(), AssetIoError> {
        self.default_io.watch_for_changes(configuration)
    }

    fn get_metadata(
        &self,
        path: &Path,
    ) -> std::result::Result<bevy::asset::Metadata, bevy::asset::AssetIoError> {
        self.default_io.get_metadata(path)
    }
}

struct LodAssetIoPlugin;

impl Plugin for LodAssetIoPlugin {
    fn build(&self, app: &mut App) {
        let default_io = AssetPlugin::default().create_platform_default_asset_io();

        let lod_path = get_lod_path();
        let lod_manager = LodManager::new(lod_path).unwrap();

        let asset_io = LodAssetIo {
            default_io,
            lod_manager,
        };
        app.insert_resource(AssetServer::new(asset_io));
    }
}
