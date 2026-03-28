use bevy::prelude::*;

use crate::GameState;

pub struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(Color::srgb(0.45, 0.55, 0.8)))
            .add_systems(
                Update,
                (update_sky_color, sync_fog_to_sky)
                    .chain()
                    .run_if(in_state(GameState::Game)),
            );
    }
}

/// Update the clear color (sky background) based on time of day.
fn update_sky_color(
    mut clear_color: ResMut<ClearColor>,
    clock_query: Query<&super::sun::DayClock>,
) {
    let Ok(clock) = clock_query.single() else {
        return;
    };
    let tod = clock.time_of_day;

    // 0=midnight, 0.25=sunrise, 0.5=noon, 0.75=sunset
    let day_amount = 1.0 - (tod * 2.0 - 1.0).abs(); // 0 at midnight, 1 at noon
    let dawn_dusk: f32 = {
        let d1 = (tod - 0.25).abs();
        let d2 = (tod - 0.75).abs();
        (1.0 - (d1.min(d2) * 8.0).min(1.0)).max(0.0)
    };

    // Day: bright blue. Night: dark blue. Dawn/dusk: warm orange.
    let r: f32 = 0.05 + 0.40 * day_amount + 0.45 * dawn_dusk;
    let g: f32 = 0.05 + 0.50 * day_amount + 0.20 * dawn_dusk;
    let b: f32 = 0.15 + 0.65 * day_amount - 0.10 * dawn_dusk;

    clear_color.0 = Color::srgb(
        r.clamp(0.02, 0.95),
        g.clamp(0.02, 0.85),
        b.clamp(0.05, 0.90),
    );
}

/// Keep fog color in sync with the sky so distant objects fade into the horizon.
fn sync_fog_to_sky(
    clear_color: Res<ClearColor>,
    mut fog_query: Query<&mut DistanceFog>,
) {
    let sky = clear_color.0;
    for mut fog in fog_query.iter_mut() {
        fog.color = sky;
    }
}
