use bevy::prelude::*;

use self::{sky::SkyPlugin, sun::SunPlugin};

pub(crate) mod sky;
pub(crate) mod sun;

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((SunPlugin, SkyPlugin));
    }
}
