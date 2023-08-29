use bevy::prelude::*;

use map_viewer::GamePlugin;

fn main() {
    App::new().add_plugins(GamePlugin).run();
}
