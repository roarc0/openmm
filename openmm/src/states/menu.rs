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
    // Use mm6title.pcx (no buttons) as background
    let bg = ui.get_or_load("mm6title.pcx", &game_assets, &mut images)
        .or_else(|| ui.get_or_load("title.pcx", &game_assets, &mut images));
    // Extract button overlay by diffing title.pcx vs mm6title.pcx
    let btn_overlay = load_button_overlay(&game_assets, &mut images, &mut ui);

    commands.spawn((
        fullscreen_bg(bg),
        OnScreen,
    )).with_children(|p| {
        // Full button overlay (always visible, from diff of title vs mm6title)
        if let Some(ref overlay) = btn_overlay {
            p.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                ImageNode::new(overlay.clone()),
            ));
        }
        // Invisible clickable regions over each button
        // Positions from pixel-diff: 163×57px, x=477, y step=62
        menu_btn(p, 477.0,   3.0, 163.0, 57.0, MenuAction::NewGame);
        menu_btn(p, 477.0,  65.0, 163.0, 57.0, MenuAction::LoadGame);
        menu_btn(p, 477.0, 127.0, 163.0, 57.0, MenuAction::Credits);
        menu_btn(p, 477.0, 190.0, 163.0, 57.0, MenuAction::Exit);
    });
}

/// Diff title.pcx (with buttons) vs mm6title.pcx (without) to get button overlay.
fn load_button_overlay(
    game_assets: &GameAssets,
    images: &mut Assets<Image>,
    ui: &mut UiAssets,
) -> Option<Handle<Image>> {
    use image::{GenericImageView, RgbaImage, DynamicImage};

    let with_btns = game_assets.lod_manager().icon("title.pcx")?;
    let without = game_assets.lod_manager().icon("mm6title.pcx")?;

    let (w, h) = with_btns.dimensions();
    let mut overlay = RgbaImage::new(w, h);

    for y in 0..h {
        for x in 0..w {
            let a = with_btns.get_pixel(x, y);
            let b = without.get_pixel(x, y);
            let dr = (a[0] as i32 - b[0] as i32).unsigned_abs();
            let dg = (a[1] as i32 - b[1] as i32).unsigned_abs();
            let db = (a[2] as i32 - b[2] as i32).unsigned_abs();
            if dr + dg + db > 30 {
                overlay.put_pixel(x, y, image::Rgba([a[0], a[1], a[2], 255]));
            }
        }
    }

    let bevy_img = bevy::image::Image::from_dynamic(
        DynamicImage::ImageRgba8(overlay), true,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    Some(images.add(bevy_img))
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

fn button_hover(
    mut query: Query<(&Interaction, &mut BackgroundColor), (Changed<Interaction>, With<Button>)>,
) {
    for (interaction, mut bg) in &mut query {
        *bg = match interaction {
            Interaction::Hovered => BackgroundColor(Color::srgba(1.0, 1.0, 0.8, 0.25)),
            Interaction::Pressed => BackgroundColor(Color::srgba(1.0, 0.9, 0.6, 0.35)),
            Interaction::None => BackgroundColor(Color::NONE),
        };
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
