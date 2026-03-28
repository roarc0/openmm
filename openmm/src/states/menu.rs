use bevy::{app::AppExit, ecs::message::MessageWriter, prelude::*};

use crate::assets::GameAssets;
use crate::ui_assets::UiAssets;
use crate::{despawn_all, GameState};

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_sub_state::<MenuScreen>()
            .add_systems(OnEnter(MenuScreen::Title), title_setup)
            .add_systems(OnExit(MenuScreen::Title), despawn_all::<OnScreen>)
            .add_systems(OnEnter(MenuScreen::Segue), segue_setup)
            .add_systems(OnExit(MenuScreen::Segue), despawn_all::<OnScreen>)
            .add_systems(OnEnter(MenuScreen::PartyCreation), party_setup)
            .add_systems(OnExit(MenuScreen::PartyCreation), despawn_all::<OnScreen>)
            .add_systems(OnEnter(MenuScreen::LoadGame), load_game_setup)
            .add_systems(OnExit(MenuScreen::LoadGame), despawn_all::<OnScreen>)
            .add_systems(OnEnter(MenuScreen::Credits), credits_setup)
            .add_systems(OnExit(MenuScreen::Credits), despawn_all::<OnScreen>)
            .add_systems(Update, menu_action.run_if(in_state(GameState::Menu)))
            .add_systems(Update, button_hover.run_if(in_state(GameState::Menu)));
    }
}

#[derive(Clone, Copy, Default, Eq, PartialEq, Debug, Hash, SubStates)]
#[source(GameState = GameState::Menu)]
enum MenuScreen {
    #[default]
    Title,
    Segue,
    PartyCreation,
    LoadGame,
    Credits,
}

/// Shared marker for all menu screen entities — despawned on screen exit.
#[derive(Component)]
struct OnScreen;

#[derive(Component)]
enum MenuAction {
    // Title
    NewGame, LoadGame, Credits, Exit,
    // Segue
    CreateParty, QuickStart,
    // Party creation
    StartGame,
    // Load game
    LoadSlot,
    // Navigation
    Back,
}

const REF_W: f32 = 640.0;
const REF_H: f32 = 480.0;

// ── Title screen (title.pcx) ────────────────────────────

fn title_setup(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    mut images: ResMut<Assets<Image>>,
    mut ui: ResMut<UiAssets>,
) {
    commands.spawn((Camera2d, OnScreen));
    // Use title.pcx (with buttons baked in) as background
    let bg = ui.get_or_load("title.pcx", &game_assets, &mut images)
        .or_else(|| ui.get_or_load("mm6title.pcx", &game_assets, &mut images));

    commands.spawn((
        fullscreen_bg(bg),
        OnScreen,
    )).with_children(|p| {
        // Hover button images (start00a=NEW, b=LOAD, c=CREDITS, d=EXIT)
        let hover_new = ui.get_or_load("start00a", &game_assets, &mut images);
        let hover_load = ui.get_or_load("start00b", &game_assets, &mut images);
        let hover_credits = ui.get_or_load("start00c", &game_assets, &mut images);
        let hover_exit = ui.get_or_load("start00d", &game_assets, &mut images);

        // Button positions matched to hover images (135×45px, x=482, y step=62)
        title_btn(p, 482.0,   9.0, 135.0, 45.0, MenuAction::NewGame, hover_new);
        title_btn(p, 482.0,  71.0, 135.0, 45.0, MenuAction::LoadGame, hover_load);
        title_btn(p, 482.0, 133.0, 135.0, 45.0, MenuAction::Credits, hover_credits);
        title_btn(p, 482.0, 195.0, 135.0, 45.0, MenuAction::Exit, hover_exit);
    });
}


// ── Segue screen (segue_bg.pcx) ─────────────────────────

fn segue_setup(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    mut images: ResMut<Assets<Image>>,
    mut ui: ResMut<UiAssets>,
) {
    commands.spawn((Camera2d, OnScreen));
    let bg = ui.get_or_load("segue_bg.pcx", &game_assets, &mut images);

    commands.spawn((
        fullscreen_bg(bg),
        OnScreen,
    )).with_children(|p| {
        menu_btn(p, 16.0, 441.0, 170.0, 35.0, MenuAction::CreateParty);
        menu_btn(p, 451.0, 441.0, 170.0, 35.0, MenuAction::QuickStart);
    });
}

// ── Party creation (makeme.pcx) ─────────────────────────

fn party_setup(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    mut images: ResMut<Assets<Image>>,
    mut ui: ResMut<UiAssets>,
) {
    commands.spawn((Camera2d, OnScreen));
    let bg = ui.get_or_load("makeme.pcx", &game_assets, &mut images);

    commands.spawn((
        fullscreen_bg(bg),
        OnScreen,
    )).with_children(|p| {
        // OK button (580, 431) from OpenEnroth
        menu_btn(p, 580.0, 431.0, 51.0, 39.0, MenuAction::StartGame);
    });
}

// ── Load game (lsave640.pcx) ────────────────────────────

fn load_game_setup(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    mut images: ResMut<Assets<Image>>,
    mut ui: ResMut<UiAssets>,
) {
    commands.spawn((Camera2d, OnScreen));
    let bg = ui.get_or_load("lsave640.pcx", &game_assets, &mut images);

    commands.spawn((
        fullscreen_bg(bg),
        OnScreen,
    )).with_children(|p| {
        // X/close button — top-right area of the load dialog
        menu_btn(p, 545.0, 399.0, 50.0, 40.0, MenuAction::Back);
        // Check/load button
        menu_btn(p, 490.0, 399.0, 50.0, 40.0, MenuAction::LoadSlot);
    });
}

// ── Credits (mm6title.pcx as backdrop, scrolling text later) ──

fn credits_setup(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    mut images: ResMut<Assets<Image>>,
    mut ui: ResMut<UiAssets>,
) {
    commands.spawn((Camera2d, OnScreen));
    let bg = ui.get_or_load("mm6title.pcx", &game_assets, &mut images);

    commands.spawn((
        fullscreen_bg(bg),
        OnScreen,
    )).with_children(|p| {
        // Click anywhere to go back
        p.spawn((
            Button,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
            MenuAction::Back,
        )).with_children(|btn| {
            btn.spawn((
                Text::new("Credits - Click to return"),
                TextFont { font_size: 32.0, ..default() },
                TextColor(Color::WHITE),
            ));
        });
    });
}

// ── Shared helpers ──────────────────────────────────────

fn fullscreen_bg(tex: Option<Handle<Image>>) -> (Node, ImageNode) {
    (
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        ImageNode::new(tex.unwrap_or_default()),
    )
}

/// Title screen button with hover image overlay.
fn title_btn(
    parent: &mut ChildSpawnerCommands,
    x: f32, y: f32, w: f32, h: f32,
    action: MenuAction,
    hover_img: Option<Handle<Image>>,
) {
    let mut btn = parent.spawn((
        Button,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(x / REF_W * 100.0),
            top: Val::Percent(y / REF_H * 100.0),
            width: Val::Percent(w / REF_W * 100.0),
            height: Val::Percent(h / REF_H * 100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::NONE),
        action,
    ));
    // Hover image sized to fill the button area (scales with window like the background)
    if let Some(img) = hover_img {
        btn.with_children(|btn_parent| {
            btn_parent.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                ImageNode::new(img),
                Visibility::Hidden,
                HoverImage,
            ));
        });
    }
}

/// Generic menu button (no hover image).
fn menu_btn(parent: &mut ChildSpawnerCommands, x: f32, y: f32, w: f32, h: f32, action: MenuAction) {
    parent.spawn((
        Button,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(x / REF_W * 100.0),
            top: Val::Percent(y / REF_H * 100.0),
            width: Val::Percent(w / REF_W * 100.0),
            height: Val::Percent(h / REF_H * 100.0),
            ..default()
        },
        BackgroundColor(Color::NONE),
        action,
    ));
}

/// Marker for hover image children of buttons.
#[derive(Component)]
struct HoverImage;

fn button_hover(
    query: Query<(&Interaction, &Children), (Changed<Interaction>, With<Button>)>,
    mut hover_query: Query<&mut Visibility, With<HoverImage>>,
) {
    for (interaction, children) in &query {
        let show = matches!(interaction, Interaction::Hovered | Interaction::Pressed);
        for child in children.iter() {
            if let Ok(mut vis) = hover_query.get_mut(child) {
                *vis = if show { Visibility::Inherited } else { Visibility::Hidden };
            }
        }
    }
}

fn menu_action(
    query: Query<(&Interaction, &MenuAction), (Changed<Interaction>, With<Button>)>,
    mut game_state: ResMut<NextState<GameState>>,
    mut menu_screen: ResMut<NextState<MenuScreen>>,
    mut exit_writer: MessageWriter<AppExit>,
) {
    for (interaction, action) in &query {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            MenuAction::NewGame => menu_screen.set(MenuScreen::Segue),
            MenuAction::LoadGame => menu_screen.set(MenuScreen::LoadGame),
            MenuAction::Credits => menu_screen.set(MenuScreen::Credits),
            MenuAction::Exit => { exit_writer.write(AppExit::Success); }
            MenuAction::CreateParty => menu_screen.set(MenuScreen::PartyCreation),
            MenuAction::QuickStart | MenuAction::StartGame | MenuAction::LoadSlot => {
                game_state.set(GameState::Loading);
            }
            MenuAction::Back => menu_screen.set(MenuScreen::Title),
        }
    }
}
