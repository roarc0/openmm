use bevy::{app::AppExit, ecs::message::MessageWriter, prelude::*};

use crate::assets::GameAssets;
use crate::ui_assets::UiAssets;
use crate::{despawn_all, GameState};

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Menu), main_menu_setup)
            .add_systems(OnExit(GameState::Menu), despawn_all::<OnMenuScreen>)
            .add_systems(
                Update,
                menu_action.run_if(in_state(GameState::Menu)),
            )
            .add_systems(
                Update,
                button_hover.run_if(in_state(GameState::Menu)),
            );
    }
}

#[derive(Component)]
struct OnMenuScreen;

/// Actions triggered by menu buttons.
#[derive(Component)]
enum MenuAction {
    NewGame,
    LoadGame,
    Credits,
    Exit,
}

/// MM6 title screen button positions (640×480 reference resolution).
/// Buttons are baked into title.pcx — we overlay invisible click regions.
const BTN_X: f32 = 495.0;
const BTN_W: f32 = 130.0;
const BTN_H: f32 = 42.0;
const BTN_NEW_Y: f32 = 172.0;
const BTN_LOAD_Y: f32 = 227.0;
const BTN_CREDITS_Y: f32 = 282.0;
const BTN_EXIT_Y: f32 = 337.0;
const REF_W: f32 = 640.0;
const REF_H: f32 = 480.0;

fn main_menu_setup(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    mut images: ResMut<Assets<Image>>,
    mut ui: ResMut<UiAssets>,
) {
    commands.spawn((Camera2d, OnMenuScreen));

    // Load title screen from LOD (title.pcx in MM6)
    let bg = ui.get_or_load("title.pcx", &game_assets, &mut images)
        .or_else(|| ui.get_or_load("mm6title.pcx", &game_assets, &mut images));

    // Full-screen background with the title image
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        ImageNode::new(bg.unwrap_or_default()),
        OnMenuScreen,
    )).with_children(|parent| {
        // Invisible button overlays at the exact positions where
        // the buttons are drawn in the title.pcx image.
        // Positions are percentage-based relative to 640×480.
        spawn_menu_button(parent, BTN_NEW_Y, MenuAction::NewGame);
        spawn_menu_button(parent, BTN_LOAD_Y, MenuAction::LoadGame);
        spawn_menu_button(parent, BTN_CREDITS_Y, MenuAction::Credits);
        spawn_menu_button(parent, BTN_EXIT_Y, MenuAction::Exit);
    });
}

fn spawn_menu_button(parent: &mut ChildSpawnerCommands, y: f32, action: MenuAction) {
    parent.spawn((
        Button,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(BTN_X / REF_W * 100.0),
            top: Val::Percent(y / REF_H * 100.0),
            width: Val::Percent(BTN_W / REF_W * 100.0),
            height: Val::Percent(BTN_H / REF_H * 100.0),
            ..default()
        },
        BackgroundColor(Color::NONE),
        action,
    ));
}

fn button_hover(
    mut query: Query<(&Interaction, &mut BackgroundColor), (Changed<Interaction>, With<Button>)>,
) {
    for (interaction, mut bg) in &mut query {
        *bg = match interaction {
            Interaction::Hovered => BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.15)),
            Interaction::Pressed => BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.2)),
            Interaction::None => BackgroundColor(Color::NONE),
        };
    }
}

fn menu_action(
    query: Query<(&Interaction, &MenuAction), (Changed<Interaction>, With<Button>)>,
    mut game_state: ResMut<NextState<GameState>>,
    mut exit_writer: MessageWriter<AppExit>,
) {
    for (interaction, action) in &query {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            MenuAction::NewGame => {
                game_state.set(GameState::Loading);
            }
            MenuAction::LoadGame => {
                // TODO: show load game dialog
                game_state.set(GameState::Loading);
            }
            MenuAction::Credits => {
                // TODO: show credits
            }
            MenuAction::Exit => {
                exit_writer.write(AppExit::Success);
            }
        }
    }
}
