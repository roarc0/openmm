//! NPC dialogue overlay — shows portrait, name, profession, and greeting text.
//!
//! Built as Bevy UI nodes rather than the screen .ron system because the
//! npc_speak.ron layout isn't finished. When the .ron is ready, migrate
//! this to the screen-layer approach.

use bevy::prelude::*;

use crate::game::actors::npc_dialogue::{NpcPortrait, NpcProfile};
use crate::game::rendering::viewport;
use crate::screens::ui_assets::UiAssets;
use crate::system::config::GameConfig;

use super::{UiMode, UiState};

/// Marker for all NPC dialogue overlay entities — despawned on exit.
#[derive(Component)]
struct NpcDialogueUi;

/// Tracks whether the overlay is currently spawned.
#[derive(Resource, Default)]
struct NpcDialogueActive(bool);

pub struct NpcDialoguePlugin;

impl Plugin for NpcDialoguePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NpcDialogueActive>()
            .add_systems(Update, (spawn_dialogue, despawn_dialogue));
    }
}

/// Spawn the dialogue overlay when entering NpcDialogue mode.
fn spawn_dialogue(
    ui: Res<UiState>,
    mut active: ResMut<NpcDialogueActive>,
    portrait_res: Option<Res<NpcPortrait>>,
    profile_res: Option<Res<NpcProfile>>,
    mut commands: Commands,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    cfg: Res<GameConfig>,
    ui_assets: Res<UiAssets>,
) {
    if ui.mode != UiMode::NpcDialogue || active.0 {
        return;
    }
    active.0 = true;

    let Some(portrait) = portrait_res else {
        return;
    };
    let default_profile = NpcProfile::default();
    let profile: &NpcProfile = match profile_res {
        Some(ref r) => r,
        None => &default_profile,
    };

    let Ok(window) = windows.single() else {
        return;
    };
    let (vp_x, vp_y, vp_w, vp_h) = viewport::viewport_inner_rect(window, &cfg, &ui_assets);

    // Panel sits on the right side of the viewport, same area as MM6 dialogue.
    // In original MM6: right panel is ~152px wide at 640x480.
    let scale = vp_h / 345.0; // reference inner height
    let panel_w = 152.0 * scale;
    let panel_x = vp_x + vp_w - panel_w;
    let panel_y = vp_y;
    let panel_h = vp_h;

    // Portrait size scaled to fit panel width with margin.
    let portrait_w = (panel_w * 0.7).min(portrait.size.x * scale);
    let portrait_h = portrait_w * (portrait.size.y / portrait.size.x);
    let portrait_left = (panel_w - portrait_w) / 2.0;

    // Root panel — dark semi-transparent background on the right.
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(panel_x),
                top: Val::Px(panel_y),
                width: Val::Px(panel_w),
                height: Val::Px(panel_h),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(4.0 * scale)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.85)),
            GlobalZIndex(10),
            NpcDialogueUi,
        ))
        .with_children(|parent| {
            // Portrait image.
            parent.spawn((
                ImageNode::new(portrait.image.clone()),
                Node {
                    width: Val::Px(portrait_w),
                    height: Val::Px(portrait_h),
                    margin: UiRect {
                        left: Val::Px(portrait_left),
                        right: Val::Auto,
                        top: Val::Px(8.0 * scale),
                        bottom: Val::Px(6.0 * scale),
                    },
                    ..default()
                },
            ));

            // NPC name.
            let name_text = if let Some(ref prof) = profile.profession {
                format!("{} the {}", profile.name, prof)
            } else {
                profile.name.clone()
            };
            parent.spawn((
                Text::new(name_text),
                TextFont {
                    font_size: 14.0 * scale,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.9, 0.5)),
                Node {
                    margin: UiRect::bottom(Val::Px(6.0 * scale)),
                    ..default()
                },
            ));

            // Greeting text.
            if let Some(ref greeting) = profile.greeting_text {
                parent.spawn((
                    Text::new(greeting.clone()),
                    TextFont {
                        font_size: 11.0 * scale,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    Node {
                        max_width: Val::Px(panel_w - 12.0 * scale),
                        margin: UiRect::bottom(Val::Px(6.0 * scale)),
                        ..default()
                    },
                ));
            }

            // Day topic if available.
            if let Some(ref topic) = profile.day_topic {
                parent.spawn((
                    Text::new(topic.clone()),
                    TextFont {
                        font_size: 12.0 * scale,
                        ..default()
                    },
                    TextColor(Color::srgb(0.7, 0.85, 1.0)),
                    Node {
                        margin: UiRect::bottom(Val::Px(4.0 * scale)),
                        ..default()
                    },
                ));
            }

            // Day text if available.
            if let Some(ref day_text) = profile.day_text {
                parent.spawn((
                    Text::new(day_text.clone()),
                    TextFont {
                        font_size: 10.0 * scale,
                        ..default()
                    },
                    TextColor(Color::srgba(0.9, 0.9, 0.9, 0.9)),
                    Node {
                        max_width: Val::Px(panel_w - 12.0 * scale),
                        ..default()
                    },
                ));
            }

            // Hint at bottom.
            parent.spawn((
                Text::new("[ESC] Close"),
                TextFont {
                    font_size: 10.0 * scale,
                    ..default()
                },
                TextColor(Color::srgba(0.6, 0.6, 0.6, 0.7)),
                Node {
                    margin: UiRect::top(Val::Auto),
                    ..default()
                },
            ));
        });
}

/// Remove dialogue overlay when leaving NpcDialogue mode.
fn despawn_dialogue(
    ui: Res<UiState>,
    mut active: ResMut<NpcDialogueActive>,
    entities: Query<Entity, With<NpcDialogueUi>>,
    mut commands: Commands,
) {
    if ui.mode == UiMode::NpcDialogue || !active.0 {
        return;
    }
    active.0 = false;
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}
