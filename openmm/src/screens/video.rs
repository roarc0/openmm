//! Inline SMK video playback: spawning and per-frame tick.

use bevy::asset::RenderAssetUsages;
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use openmm_data::assets::SmkDecoder;

use super::runtime::{InlineVideo, ScreenActions, ScreenLayer};
use super::{REF_H, REF_W, VideoElement};
use crate::assets::GameAssets;
use crate::game::optional::OptionalWrite;

// ── Video spawning ──────────────────────────────────────────────────────────

pub(super) fn spawn_video_element(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    audio_sources: &mut Assets<AudioSource>,
    vid: &VideoElement,
    layer_tag: &ScreenLayer,
    game_assets: &GameAssets,
) {
    let Some(bytes) = game_assets.smk_bytes(&vid.video) else {
        warn!(
            "video element '{}': '{}' not found in Anims VID archives",
            vid.id, vid.video
        );
        return;
    };

    let mut decoder = match SmkDecoder::new(bytes.clone()) {
        Ok(d) => d,
        Err(e) => {
            warn!("video element '{}': failed to decode '{}': {e}", vid.id, vid.video);
            return;
        }
    };

    let native_w = decoder.width;
    let native_h = decoder.height;
    let spf = if decoder.fps > 0.0 {
        1.0 / decoder.fps
    } else {
        1.0 / 15.0
    };

    if decoder.audio.is_some()
        && let Some(wav) = game_assets.smk_audio(&vid.video)
        && !wav.is_empty()
    {
        let handle = audio_sources.add(AudioSource { bytes: wav.into() });
        let mode = if vid.looping {
            bevy::audio::PlaybackMode::Loop
        } else {
            bevy::audio::PlaybackMode::Despawn
        };
        commands.spawn((
            AudioPlayer(handle),
            PlaybackSettings { mode, ..default() },
            layer_tag.clone(),
        ));
    }

    let mut image = Image::new_fill(
        Extent3d {
            width: native_w,
            height: native_h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    if let Some(rgba) = decoder.next_frame() {
        image.data = Some(rgba);
    }
    let image_handle = images.add(image);

    let (w, h) = if vid.size.0 > 0.0 && vid.size.1 > 0.0 {
        vid.size
    } else {
        (native_w as f32, native_h as f32)
    };

    let initial_vis = if vid.hidden {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };

    let _video_entity = commands.spawn((
        ImageNode::new(image_handle.clone()),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(vid.position.0 / REF_W * 100.0),
            top: Val::Percent(vid.position.1 / REF_H * 100.0),
            width: Val::Percent(w / REF_W * 100.0),
            height: Val::Percent(h / REF_H * 100.0),
            ..default()
        },
        ZIndex(vid.z),
        initial_vis,
        layer_tag.clone(),
        InlineVideo {
            decoder,
            image_handle,
            frame_timer: 0.0,
            spf,
            looping: vid.looping,
            skippable: vid.skippable,
            on_end: vid.on_end.clone(),
            smk_bytes: bytes,
            finished: false,
            life_timer: 0.0,
        },
    ));

    info!(
        "video element '{}': '{}' ({}x{}, {:.1}fps, loop={})",
        vid.id,
        vid.video,
        native_w,
        native_h,
        1.0 / spf,
        vid.looping
    );
}

/// Advance inline video frames and dispatch on_end actions.
pub(super) fn video_tick(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut InlineVideo)>,
    mut images: ResMut<Assets<Image>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut actions: Option<MessageWriter<ScreenActions>>,
) {
    for (entity, mut vid) in &mut query {
        if vid.finished {
            continue;
        }
        vid.life_timer += time.delta_secs();

        // Skip check.
        // Ignore skips in the first 100ms to prevent "leaked" inputs from previous screens.
        if vid.skippable && vid.life_timer > 0.1 && keys.just_pressed(KeyCode::Escape) {
            vid.finished = true;
            if !vid.on_end.is_empty() {
                actions.try_write(ScreenActions {
                    actions: vid.on_end.clone(),
                });
            }
            commands.entity(entity).despawn();
            continue;
        }

        vid.frame_timer += time.delta_secs();
        if vid.frame_timer < vid.spf {
            continue;
        }
        vid.frame_timer -= vid.spf;

        match vid.decoder.next_frame() {
            Some(rgba) => {
                if let Some(img) = images.get_mut(&vid.image_handle) {
                    img.data = Some(rgba);
                }
            }
            None => {
                if vid.looping {
                    // Restart decoder from beginning.
                    if let Ok(new_dec) = SmkDecoder::new(vid.smk_bytes.clone()) {
                        vid.decoder = new_dec;
                        vid.frame_timer = 0.0;
                    } else {
                        vid.finished = true;
                    }
                } else {
                    vid.finished = true;
                    if !vid.on_end.is_empty() {
                        actions.try_write(ScreenActions {
                            actions: vid.on_end.clone(),
                        });
                    }
                    commands.entity(entity).despawn();
                }
            }
        }
    }
}
