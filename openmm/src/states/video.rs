use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use lod::smk::SmkDecoder;
use lod::vid::Vid;

use crate::GameState;
use crate::config::GameConfig;

/// Build a minimal WAV file from raw PCM bytes.
/// Supports 8-bit (unsigned) and 16-bit (signed little-endian).
fn build_wav(pcm: &[u8], channels: u8, sample_rate: u32, bitdepth: u8) -> Vec<u8> {
    let data_len = pcm.len() as u32;
    let block_align = (channels as u32) * (bitdepth as u32 / 8);
    let byte_rate = sample_rate * block_align;
    let mut wav = Vec::with_capacity(44 + pcm.len());

    // RIFF header
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVE");

    // fmt chunk
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes()); // chunk size
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&(channels as u16).to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&(block_align as u16).to_le_bytes());
    wav.extend_from_slice(&(bitdepth as u16).to_le_bytes());

    // data chunk
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    wav.extend_from_slice(pcm);

    wav
}

pub struct VideoPlugin;

impl Plugin for VideoPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Video), video_setup)
            .add_systems(Update, video_tick.run_if(in_state(GameState::Video)))
            .add_systems(Update, video_skip.run_if(in_state(GameState::Video)))
            .add_systems(OnExit(GameState::Video), video_cleanup);
    }
}

/// Request resource — set before entering `GameState::Video`.
#[derive(Resource)]
pub struct VideoRequest {
    /// SMK name without extension, e.g. "3dologo".
    pub name: String,
    /// If true, ESC skips to `next`.
    pub skippable: bool,
    /// State to transition to when video ends or is skipped.
    pub next: GameState,
}

/// Runtime state for the playing video — inserted by `video_setup`, removed by `video_cleanup`.
#[derive(Resource)]
struct VideoPlayer {
    decoder: Option<SmkDecoder>,
    image_handle: Handle<Image>,
    frame_timer: f32,
    spf: f32,
    skippable: bool,
    next: GameState,
    /// True if video failed to load or playback is complete.
    finished: bool,
}

/// Marker component for all entities spawned during video playback.
#[derive(Component)]
struct OnVideoScreen;

fn video_setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut audio_sources: ResMut<Assets<AudioSource>>,
    request: Res<VideoRequest>,
    cfg: Res<GameConfig>,
) {
    if cfg.skip_intro {
        commands.insert_resource(VideoPlayer {
            decoder: None,
            image_handle: Handle::default(),
            frame_timer: 0.0,
            spf: 1.0,
            skippable: true,
            next: request.next,
            finished: true,
        });
        return;
    }

    let data_path = lod::get_data_path();
    let anims_dir = std::path::Path::new(&data_path).join("Anims");

    // Search Anims1.vid then Anims2.vid for the requested name.
    let smk_bytes = ["Anims1.vid", "Anims2.vid"].iter().find_map(|fname| {
        let path = anims_dir.join(fname);
        let vid = Vid::open(&path).ok()?;
        vid.smk_by_name(&request.name).map(|b| b.to_vec())
    });

    let Some(bytes) = smk_bytes else {
        warn!("VideoPlugin: '{}' not found in Anims1.vid / Anims2.vid", request.name);
        commands.insert_resource(VideoPlayer {
            decoder: None,
            image_handle: Handle::default(),
            frame_timer: 0.0,
            spf: 1.0,
            skippable: request.skippable,
            next: request.next,
            finished: true,
        });
        return;
    };

    let mut decoder = match SmkDecoder::new(bytes.clone()) {
        Ok(d) => d,
        Err(e) => {
            warn!("VideoPlugin: failed to open SMK decoder for '{}': {e}", request.name);
            commands.insert_resource(VideoPlayer {
                decoder: None,
                image_handle: Handle::default(),
                frame_timer: 0.0,
                spf: 1.0,
                skippable: request.skippable,
                next: request.next,
                finished: true,
            });
            return;
        }
    };

    let width = decoder.width;
    let height = decoder.height;
    let spf = if decoder.fps > 0.0 {
        1.0 / decoder.fps
    } else {
        1.0 / 10.0
    };

    // Pre-decode all audio using a separate decoder pass, then play as a single WAV.
    if let Some(audio_info) = decoder.audio
        && let Ok(mut audio_dec) = SmkDecoder::new(bytes)
    {
        let mut pcm: Vec<u8> = Vec::new();
        while audio_dec.next_frame().is_some() {
            pcm.extend_from_slice(&audio_dec.decode_current_audio());
        }
        if !pcm.is_empty() {
            let wav = build_wav(&pcm, audio_info.channels, audio_info.rate, audio_info.bitdepth);
            let handle = audio_sources.add(AudioSource { bytes: wav.into() });
            commands.spawn((AudioPlayer(handle), PlaybackSettings::ONCE, OnVideoScreen));
        }
    }

    // Allocate image.
    let mut image = Image::new_fill(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );

    // Decode first frame immediately so there's no black flash.
    if let Some(rgba) = decoder.next_frame() {
        image.data = Some(rgba);
    }

    let image_handle = images.add(image);

    // Spawn camera and fullscreen image node.
    commands.spawn((Camera2d, OnVideoScreen));
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        ImageNode::new(image_handle.clone()),
        OnVideoScreen,
    ));

    commands.insert_resource(VideoPlayer {
        decoder: Some(decoder),
        image_handle,
        frame_timer: 0.0,
        spf,
        skippable: request.skippable,
        next: request.next,
        finished: false,
    });
}

fn video_tick(
    mut player: ResMut<VideoPlayer>,
    mut images: ResMut<Assets<Image>>,
    mut next_state: ResMut<NextState<GameState>>,
    time: Res<Time>,
) {
    let player = player.as_mut();

    if player.finished {
        next_state.set(player.next);
        return;
    }

    player.frame_timer += time.delta_secs();
    if player.frame_timer < player.spf {
        return;
    }
    player.frame_timer -= player.spf;

    let Some(decoder) = player.decoder.as_mut() else {
        next_state.set(player.next);
        return;
    };

    match decoder.next_frame() {
        Some(rgba) => {
            if let Some(img) = images.get_mut(&player.image_handle) {
                img.data = Some(rgba);
            }
        }
        None => {
            next_state.set(player.next);
        }
    }
}

fn video_skip(player: Res<VideoPlayer>, keys: Res<ButtonInput<KeyCode>>, mut next_state: ResMut<NextState<GameState>>) {
    if player.skippable && keys.just_pressed(KeyCode::Escape) {
        next_state.set(player.next);
    }
}

fn video_cleanup(mut commands: Commands, to_despawn: Query<Entity, With<OnVideoScreen>>) {
    for entity in &to_despawn {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<VideoPlayer>();
}
