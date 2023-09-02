use bevy::{app::AppExit, prelude::*};

use super::{despawn_screen, GameState};

const TEXT_COLOR: Color = Color::rgb(0.3, 0.9, 0.3);

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_state::<MenuState>()
            .add_systems(OnEnter(GameState::Menu), menu_setup)
            .add_systems(OnEnter(MenuState::Main), main_menu_setup)
            .add_systems(OnExit(MenuState::Main), despawn_screen::<OnMainMenuScreen>)
            .add_systems(OnEnter(MenuState::Settings), settings_menu_setup)
            .add_systems(
                OnExit(MenuState::Settings),
                despawn_screen::<OnSettingsMenuScreen>,
            )
            // .add_systems(
            //     OnEnter(MenuState::SettingsDisplay),
            //     display_settings_menu_setup,
            // )
            // .add_systems(
            //     Update,
            //     (setting_button::<WindowMode>.run_if(in_state(MenuState::SettingsDisplay)),),
            // )
            // .add_systems(
            //     OnExit(MenuState::SettingsDisplay),
            //     despawn_screen::<OnDisplaySettingsMenuScreen>,
            // )
            // .add_systems(OnEnter(MenuState::SettingsSound), sound_settings_menu_setup)
            // .add_systems(
            //     Update,
            //     setting_button::<Volume>.run_if(in_state(MenuState::SettingsSound)),
            // )
            .add_systems(
                OnExit(MenuState::SettingsSound),
                despawn_screen::<OnSoundSettingsMenuScreen>,
            )
            .add_systems(
                Update,
                (menu_action, button_system).run_if(in_state(GameState::Menu)),
            );
    }
}

// State used for the current menu screen
#[derive(Clone, Copy, Default, Eq, PartialEq, Debug, Hash, States)]
enum MenuState {
    Main,
    Settings,
    SettingsDisplay,
    SettingsSound,
    #[default]
    Disabled,
}

#[derive(Component)]
struct OnMainMenuScreen;

#[derive(Component)]
struct OnSettingsMenuScreen;

#[derive(Component)]
struct OnDisplaySettingsMenuScreen;

#[derive(Component)]
struct OnSoundSettingsMenuScreen;

const NORMAL_BUTTON: Color = Color::rgb(0.15, 0.15, 0.15);
const HOVERED_BUTTON: Color = Color::rgb(0.25, 0.25, 0.25);
const HOVERED_PRESSED_BUTTON: Color = Color::rgb(0.25, 0.65, 0.25);
const PRESSED_BUTTON: Color = Color::rgb(0.35, 0.75, 0.35);

#[derive(Component)]
struct SelectedOption;

#[derive(Component)]
enum MenuButtonAction {
    Play,
    Settings,
    SettingsDisplay,
    //SettingsSound,
    BackToMainMenu,
    BackToSettings,
    Quit,
}

fn button_system(
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor, Option<&SelectedOption>),
        (Changed<Interaction>, With<Button>),
    >,
) {
    for (interaction, mut color, selected) in &mut interaction_query {
        *color = match (*interaction, selected) {
            (Interaction::Pressed, _) | (Interaction::None, Some(_)) => PRESSED_BUTTON.into(),
            (Interaction::Hovered, Some(_)) => HOVERED_PRESSED_BUTTON.into(),
            (Interaction::Hovered, None) => HOVERED_BUTTON.into(),
            (Interaction::None, None) => NORMAL_BUTTON.into(),
        }
    }
}

fn setting_button<T: Resource + Component + PartialEq + Copy>(
    interaction_query: Query<(&Interaction, &T, Entity), (Changed<Interaction>, With<Button>)>,
    mut selected_query: Query<(Entity, &mut BackgroundColor), With<SelectedOption>>,
    mut commands: Commands,
    mut setting: ResMut<T>,
) {
    for (interaction, button_setting, entity) in &interaction_query {
        if *interaction == Interaction::Pressed && *setting != *button_setting {
            let (previous_button, mut previous_color) = selected_query.single_mut();
            *previous_color = NORMAL_BUTTON.into();
            commands.entity(previous_button).remove::<SelectedOption>();
            commands.entity(entity).insert(SelectedOption);
            *setting = *button_setting;
        }
    }
}

fn menu_setup(mut menu_state: ResMut<NextState<MenuState>>) {
    menu_state.set(MenuState::Main);
}

fn main_menu_setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let button_style = Style {
        width: Val::Px(250.0),
        height: Val::Px(65.0),
        margin: UiRect::all(Val::Px(20.0)),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    };
    let button_icon_style = Style {
        width: Val::Px(30.0),
        position_type: PositionType::Absolute,
        left: Val::Px(10.0),
        right: Val::Auto,
        ..default()
    };
    let button_text_style = TextStyle {
        font_size: 40.0,
        color: TEXT_COLOR,
        ..default()
    };

    commands.spawn((Camera2dBundle::default(), OnMainMenuScreen));

    commands
        .spawn((
            NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    align_items: AlignItems::Stretch,
                    justify_content: JustifyContent::End,
                    ..default()
                },
                ..default()
            },
            OnMainMenuScreen,
        ))
        .with_children(|parent| {
            parent.spawn(ImageBundle {
                style: Style {
                    flex_grow: 1.,
                    //position_type: PositionType::Absolute,
                    ..default()
                },
                image: UiImage::new(asset_server.load("mm6title.png")),
                ..default()
            });

            parent
                .spawn(NodeBundle {
                    style: Style {
                        position_type: PositionType::Absolute,
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::End,
                        ..default()
                    },
                    ..default()
                })
                .with_children(|parent| {
                    parent
                        .spawn((
                            ButtonBundle {
                                style: button_style.clone(),
                                background_color: NORMAL_BUTTON.into(),
                                ..default()
                            },
                            MenuButtonAction::Play,
                        ))
                        .with_children(|parent| {
                            let icon = asset_server.load("right.png");
                            parent.spawn(ImageBundle {
                                style: button_icon_style.clone(),
                                image: UiImage::new(icon),
                                ..default()
                            });
                            parent.spawn(TextBundle::from_section(
                                "New Game",
                                button_text_style.clone(),
                            ));
                        });
                    parent
                        .spawn((
                            ButtonBundle {
                                style: button_style.clone(),
                                background_color: NORMAL_BUTTON.into(),
                                ..default()
                            },
                            MenuButtonAction::Settings,
                        ))
                        .with_children(|parent| {
                            let icon = asset_server.load("wrench.png");
                            parent.spawn(ImageBundle {
                                style: button_icon_style.clone(),
                                image: UiImage::new(icon),
                                ..default()
                            });
                            parent.spawn(TextBundle::from_section(
                                "Settings",
                                button_text_style.clone(),
                            ));
                        });
                    parent
                        .spawn((
                            ButtonBundle {
                                style: button_style,
                                background_color: NORMAL_BUTTON.into(),
                                ..default()
                            },
                            MenuButtonAction::Quit,
                        ))
                        .with_children(|parent| {
                            let icon = asset_server.load("exitRight.png");
                            parent.spawn(ImageBundle {
                                style: button_icon_style,
                                image: UiImage::new(icon),
                                ..default()
                            });
                            parent.spawn(TextBundle::from_section("Quit", button_text_style));
                        });
                });
        });
}

fn settings_menu_setup(mut commands: Commands) {
    let button_style = Style {
        width: Val::Px(200.0),
        height: Val::Px(65.0),
        margin: UiRect::all(Val::Px(20.0)),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    };

    let button_text_style = TextStyle {
        font_size: 40.0,
        color: TEXT_COLOR,
        ..default()
    };

    commands
        .spawn((
            NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                ..default()
            },
            OnSettingsMenuScreen,
        ))
        .with_children(|parent| {
            parent
                .spawn(NodeBundle {
                    style: Style {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    background_color: Color::CRIMSON.into(),
                    ..default()
                })
                .with_children(|parent| {
                    for (action, text) in [
                        (MenuButtonAction::SettingsDisplay, "Display"),
                        //(MenuButtonAction::SettingsSound, "Sound"),
                        (MenuButtonAction::BackToMainMenu, "Back"),
                    ] {
                        parent
                            .spawn((
                                ButtonBundle {
                                    style: button_style.clone(),
                                    background_color: NORMAL_BUTTON.into(),
                                    ..default()
                                },
                                action,
                            ))
                            .with_children(|parent| {
                                parent.spawn(TextBundle::from_section(
                                    text,
                                    button_text_style.clone(),
                                ));
                            });
                    }
                });
        });
}

// fn display_settings_menu_setup(mut commands: Commands, window_mode: Res<WindowMode>) {
//     let button_style = Style {
//         width: Val::Px(200.0),
//         height: Val::Px(65.0),
//         margin: UiRect::all(Val::Px(20.0)),
//         justify_content: JustifyContent::Center,
//         align_items: AlignItems::Center,
//         ..default()
//     };
//     let button_text_style = TextStyle {
//         font_size: 40.0,
//         color: TEXT_COLOR,
//         ..default()
//     };

//     commands
//         .spawn((
//             NodeBundle {
//                 style: Style {
//                     width: Val::Percent(100.0),
//                     align_items: AlignItems::Center,
//                     justify_content: JustifyContent::Center,
//                     ..default()
//                 },
//                 ..default()
//             },
//             OnDisplaySettingsMenuScreen,
//         ))
//         .with_children(|parent| {
//             parent
//                 .spawn(NodeBundle {
//                     style: Style {
//                         flex_direction: FlexDirection::Column,
//                         align_items: AlignItems::Center,
//                         ..default()
//                     },
//                     background_color: Color::CRIMSON.into(),
//                     ..default()
//                 })
//                 .with_children(|parent| {
//                     // Create a new `NodeBundle`, this time not setting its `flex_direction`. It will
//                     // use the default value, `FlexDirection::Row`, from left to right.
//                     parent
//                         .spawn(NodeBundle {
//                             style: Style {
//                                 align_items: AlignItems::Center,
//                                 ..default()
//                             },
//                             background_color: Color::CRIMSON.into(),
//                             ..default()
//                         })
//                         .with_children(|parent| {
//                             // Display a label for the current setting
//                             parent.spawn(TextBundle::from_section(
//                                 "Display Quality",
//                                 button_text_style.clone(),
//                             ));
//                             // Display a button for each possible value
//                             for window_mode_setting in
//                                 [WindowMode::Windowed, WindowMode::FullScreen]
//                             {
//                                 let mut entity = parent.spawn(ButtonBundle {
//                                     style: Style {
//                                         width: Val::Px(150.0),
//                                         height: Val::Px(65.0),
//                                         ..button_style.clone()
//                                     },
//                                     background_color: NORMAL_BUTTON.into(),
//                                     ..default()
//                                 });
//                                 entity.insert(window_mode_setting).with_children(|parent| {
//                                     parent.spawn(TextBundle::from_section(
//                                         format!("{window_mode_setting:?}"),
//                                         button_text_style.clone(),
//                                     ));
//                                 });
//                                 if *window_mode == window_mode_setting {
//                                     entity.insert(SelectedOption);
//                                 }
//                             }
//                         });
//                     parent
//                         .spawn((
//                             ButtonBundle {
//                                 style: button_style,
//                                 background_color: NORMAL_BUTTON.into(),
//                                 ..default()
//                             },
//                             MenuButtonAction::BackToSettings,
//                         ))
//                         .with_children(|parent| {
//                             parent.spawn(TextBundle::from_section("Back", button_text_style));
//                         });
//                 });
//         });
// }

// fn sound_settings_menu_setup(mut commands: Commands, volume: Res<Volume>) {
//     let button_style = Style {
//         width: Val::Px(200.0),
//         height: Val::Px(65.0),
//         margin: UiRect::all(Val::Px(20.0)),
//         justify_content: JustifyContent::Center,
//         align_items: AlignItems::Center,
//         ..default()
//     };
//     let button_text_style = TextStyle {
//         font_size: 40.0,
//         color: TEXT_COLOR,
//         ..default()
//     };

//     commands
//         .spawn((
//             NodeBundle {
//                 style: Style {
//                     width: Val::Percent(100.0),
//                     align_items: AlignItems::Center,
//                     justify_content: JustifyContent::Center,
//                     ..default()
//                 },
//                 ..default()
//             },
//             OnSoundSettingsMenuScreen,
//         ))
//         .with_children(|parent| {
//             parent
//                 .spawn(NodeBundle {
//                     style: Style {
//                         flex_direction: FlexDirection::Column,
//                         align_items: AlignItems::Center,
//                         ..default()
//                     },
//                     background_color: Color::CRIMSON.into(),
//                     ..default()
//                 })
//                 .with_children(|parent| {
//                     parent
//                         .spawn(NodeBundle {
//                             style: Style {
//                                 align_items: AlignItems::Center,
//                                 ..default()
//                             },
//                             background_color: Color::CRIMSON.into(),
//                             ..default()
//                         })
//                         .with_children(|parent| {
//                             parent.spawn(TextBundle::from_section(
//                                 "Volume",
//                                 button_text_style.clone(),
//                             ));
//                             for volume_setting in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9] {
//                                 let mut entity = parent.spawn(ButtonBundle {
//                                     style: Style {
//                                         width: Val::Px(30.0),
//                                         height: Val::Px(65.0),
//                                         ..button_style.clone()
//                                     },
//                                     background_color: NORMAL_BUTTON.into(),
//                                     ..default()
//                                 });
//                                 entity.insert(Volume(volume_setting));
//                                 if *volume == Volume(volume_setting) {
//                                     entity.insert(SelectedOption);
//                                 }
//                             }
//                         });
//                     parent
//                         .spawn((
//                             ButtonBundle {
//                                 style: button_style,
//                                 background_color: NORMAL_BUTTON.into(),
//                                 ..default()
//                             },
//                             MenuButtonAction::BackToSettings,
//                         ))
//                         .with_children(|parent| {
//                             parent.spawn(TextBundle::from_section("Back", button_text_style));
//                         });
//                 });
//         });
// }

fn menu_action(
    interaction_query: Query<
        (&Interaction, &MenuButtonAction),
        (Changed<Interaction>, With<Button>),
    >,
    mut app_exit_events: EventWriter<AppExit>,
    mut menu_state: ResMut<NextState<MenuState>>,
    mut game_state: ResMut<NextState<GameState>>,
) {
    for (interaction, menu_button_action) in &interaction_query {
        if *interaction == Interaction::Pressed {
            match menu_button_action {
                MenuButtonAction::Quit => app_exit_events.send(AppExit),
                MenuButtonAction::Play => {
                    game_state.set(GameState::Game);
                    menu_state.set(MenuState::Disabled);
                }
                MenuButtonAction::Settings => menu_state.set(MenuState::Settings),
                MenuButtonAction::SettingsDisplay => menu_state.set(MenuState::SettingsDisplay),
                MenuButtonAction::BackToMainMenu => menu_state.set(MenuState::Main),
                MenuButtonAction::BackToSettings => menu_state.set(MenuState::Settings),
            }
        }
    }
}
