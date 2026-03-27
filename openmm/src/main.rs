use bevy::prelude::*;

use openmm::GamePlugin;

fn main() {
    App::new().add_plugins(GamePlugin).run();
}
