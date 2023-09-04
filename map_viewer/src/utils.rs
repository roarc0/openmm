use bevy::prelude::Color;
use random_color::{Luminosity, RandomColor};

pub(super) fn random_color() -> Color {
    let color = RandomColor::new()
        .luminosity(Luminosity::Dark)
        .to_rgb_array();

    Color::rgba(
        color[0] as f32 / 255.,
        color[1] as f32 / 255.,
        color[2] as f32 / 255.,
        1.0,
    )
}
