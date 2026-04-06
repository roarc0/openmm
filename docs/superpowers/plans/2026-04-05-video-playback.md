# Video Playback Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Play MM6 Smacker (.smk) videos in a dedicated Bevy state, starting with the 3dologo intro before the main menu.

**Architecture:** Vendor libsmacker C source into the `openmm-data` crate, compile via `build.rs` + `cc` crate, wrap with a safe Rust `SmkDecoder`. Replace `GameState::Splash` with `GameState::Video`. A `VideoPlugin` drives frame upload into a live Bevy `Image` each tick. Mid-game videos pause the game without despawning game entities (achieved by moving InGame cleanup to `OnEnter(Loading)` instead of `OnExit(Game)`).

**Tech Stack:** libsmacker 1.2.0 (C, vendored), `cc` crate (build), Bevy 0.18 UI (`ImageNode`), existing `openmm_data::vid::Vid` parser for loading SMK bytes from VID archives.

---

## File Map

| File | Action | Responsibility |
|------|--------|---------------|
| `openmm-data/vendor/libsmacker/smacker.c` | Create (download) | C library implementation |
| `openmm-data/vendor/libsmacker/smacker.h` | Create (download) | C library public API |
| `openmm-data/vendor/libsmacker/smk_malloc.h` | Create (download) | C library internal header |
| `openmm-data/build.rs` | Create | Compile libsmacker with `cc` crate |
| `openmm-data/Cargo.toml` | Modify | Add `cc` to `[build-dependencies]` |
| `openmm-data/src/smk.rs` | Create | Safe Rust FFI bindings + `SmkDecoder` |
| `openmm-data/src/lib.rs` | Modify | Add `pub mod smk;` |
| `openmm/src/lib.rs` | Modify | Rename `Splash`→`Video`, add `VideoRequest` resource + `VideoPlugin`, initial state |
| `openmm/src/states/mod.rs` | Modify | Replace `splash` with `video` |
| `openmm/src/states/video.rs` | Create (replaces splash.rs) | Full `VideoPlugin` implementation |
| `openmm/src/states/splash.rs` | Delete | Was a placeholder, replaced by video.rs |
| `openmm/src/game/mod.rs` | Modify | Move InGame cleanup from `OnExit(Game)` to `OnEnter(Loading)` |
| `openmm/src/states/loading.rs` | Modify | Add `despawn_all::<InGame>` on `OnEnter(Loading)` |
| `openmm-data/src/evt.rs` | Modify | Add `PlayVideo` variant to `GameEvent` |
| `openmm/src/game/event_dispatch.rs` | Modify | Handle `GameEvent::PlayVideo` |

---

## Task 1: Vendor libsmacker C source

**Files:**
- Create: `openmm-data/vendor/libsmacker/smacker.c`
- Create: `openmm-data/vendor/libsmacker/smacker.h`
- Create: `openmm-data/vendor/libsmacker/smk_malloc.h`

- [ ] **Step 1: Create vendor directory and download source files**

```bash
mkdir -p openmm-data/vendor/libsmacker
curl -fsSL https://raw.githubusercontent.com/greg-kennedy/libsmacker/master/smacker.c \
     -o openmm-data/vendor/libsmacker/smacker.c
curl -fsSL https://raw.githubusercontent.com/greg-kennedy/libsmacker/master/smacker.h \
     -o openmm-data/vendor/libsmacker/smacker.h
curl -fsSL https://raw.githubusercontent.com/greg-kennedy/libsmacker/master/smk_malloc.h \
     -o openmm-data/vendor/libsmacker/smk_malloc.h
```

Expected: three files created. Verify with `head -5 openmm-data/vendor/libsmacker/smacker.h` — should show copyright comment and `#ifndef SMACKER_H`.

- [ ] **Step 2: Add `cc` to openmm-data/Cargo.toml build-dependencies**

In `openmm-data/Cargo.toml`, add after `[dependencies]`:

```toml
[build-dependencies]
cc = "1"
```

- [ ] **Step 3: Create openmm-data/build.rs**

```rust
fn main() {
    cc::Build::new()
        .file("vendor/libsmacker/smacker.c")
        .include("vendor/libsmacker")
        .warnings(false)
        .compile("smacker");
    println!("cargo:rerun-if-changed=vendor/libsmacker/smacker.c");
    println!("cargo:rerun-if-changed=vendor/libsmacker/smacker.h");
    println!("cargo:rerun-if-changed=vendor/libsmacker/smk_malloc.h");
}
```

- [ ] **Step 4: Verify it compiles**

```bash
cargo build -p openmm-data 2>&1 | grep -E "error|warning: unused|Compiling openmm-data"
```

Expected: `Compiling openmm-data ...` with no errors. Warnings from the C code are suppressed by `.warnings(false)`.

- [ ] **Step 5: Commit**

```bash
git add openmm-data/vendor/ openmm-data/build.rs openmm-data/Cargo.toml
git commit -S --no-gpg-sign -m "feat(openmm-data): vendor libsmacker C source and add build.rs"
```

---

## Task 2: Safe Rust FFI bindings — `openmm-data/src/smk.rs`

**Files:**
- Create: `openmm-data/src/smk.rs`
- Modify: `openmm-data/src/lib.rs`

- [ ] **Step 1: Add `pub mod smk;` to openmm-data/src/lib.rs**

In `openmm-data/src/lib.rs`, add to the module list (after `pub mod tft;` and `pub mod vid;`):

```rust
pub mod smk;
```

- [ ] **Step 2: Write the FFI declarations and SmkDecoder in openmm-data/src/smk.rs**

Create `openmm-data/src/smk.rs` with the full content below:

```rust
//! Safe Rust wrapper around the vendored libsmacker C library.
//!
//! libsmacker decodes Smacker (SMK2/SMK4) video files frame by frame.
//! The C library keeps a pointer into the raw data buffer, so `SmkDecoder`
//! owns the data for its entire lifetime.
//!
//! Usage:
//! ```ignore
//! let bytes = vid.smk_bytes(index).to_vec();
//! let mut dec = SmkDecoder::new(bytes)?;
//! while let Some(rgba) = dec.next_frame() {
//!     // rgba: Vec<u8> of width * height * 4 bytes (RGBA)
//! }
//! ```

use std::os::raw::{c_char, c_ulong};

// ── FFI declarations ────────────────────────────────────────────────────────

/// Opaque smk handle (C: `struct smk_t *`).
#[repr(C)]
struct SmkT {
    _private: [u8; 0],
}
type SmkHandle = *mut SmkT;

const SMK_DONE: c_char = 0;
const SMK_ERROR: c_char = -1;

#[link(name = "smacker", kind = "static")]
unsafe extern "C" {
    fn smk_open_memory(buffer: *const u8, size: c_ulong) -> SmkHandle;
    fn smk_close(object: SmkHandle);
    fn smk_info_all(
        object: SmkHandle,
        frame: *mut c_ulong,
        frame_count: *mut c_ulong,
        usf: *mut f64,
    ) -> c_char;
    fn smk_info_video(
        object: SmkHandle,
        w: *mut c_ulong,
        h: *mut c_ulong,
        y_scale_mode: *mut u8,
    ) -> c_char;
    fn smk_enable_video(object: SmkHandle, enable: u8) -> c_char;
    fn smk_enable_audio(object: SmkHandle, track: u8, enable: u8) -> c_char;
    fn smk_first(object: SmkHandle) -> c_char;
    fn smk_next(object: SmkHandle) -> c_char;
    fn smk_get_palette(object: SmkHandle) -> *const u8;
    fn smk_get_video(object: SmkHandle) -> *const u8;
}

// ── Safe wrapper ────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum SmkError {
    OpenFailed,
    InfoFailed,
}

impl std::fmt::Display for SmkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SmkError::OpenFailed => write!(f, "smk_open_memory failed"),
            SmkError::InfoFailed => write!(f, "smk_info_all/video failed"),
        }
    }
}

impl std::error::Error for SmkError {}

/// Decodes an SMK video frame by frame, yielding RGBA pixel data.
///
/// The `_data` field keeps the raw SMK bytes alive — libsmacker holds a pointer
/// into this buffer and reads from it during decoding.
pub struct SmkDecoder {
    handle: SmkHandle,
    _data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub frame_count: u32,
    /// Frames per second derived from the `usf` (microseconds per frame) field.
    pub fps: f32,
    started: bool,
    done: bool,
}

// Safety: SmkDecoder owns its data and handle exclusively; never shared across threads.
unsafe impl Send for SmkDecoder {}

impl SmkDecoder {
    /// Create a decoder from raw SMK bytes (e.g. from `Vid::smk_bytes(...).to_vec()`).
    pub fn new(data: Vec<u8>) -> Result<Self, SmkError> {
        let handle = unsafe {
            smk_open_memory(data.as_ptr(), data.len() as c_ulong)
        };
        if handle.is_null() {
            return Err(SmkError::OpenFailed);
        }

        // Read metadata
        let (frame_count, fps) = unsafe {
            let mut _frame: c_ulong = 0;
            let mut count: c_ulong = 0;
            let mut usf: f64 = 0.0;
            if smk_info_all(handle, &mut _frame, &mut count, &mut usf) == SMK_ERROR {
                smk_close(handle);
                return Err(SmkError::InfoFailed);
            }
            let fps = if usf > 0.0 { 1_000_000.0 / usf as f32 } else { 10.0 };
            (count as u32, fps)
        };

        let (width, height) = unsafe {
            let mut w: c_ulong = 0;
            let mut h: c_ulong = 0;
            let mut _yscale: u8 = 0;
            if smk_info_video(handle, &mut w, &mut h, &mut _yscale) == SMK_ERROR {
                smk_close(handle);
                return Err(SmkError::InfoFailed);
            }
            (w as u32, h as u32)
        };

        // Enable video only; disable all 7 audio tracks (audio deferred).
        unsafe {
            smk_enable_video(handle, 1);
            for track in 0..7u8 {
                smk_enable_audio(handle, track, 0);
            }
        }

        Ok(Self {
            handle,
            _data: data,
            width,
            height,
            frame_count,
            fps,
            started: false,
            done: false,
        })
    }

    /// Decode and return the next frame as RGBA pixels (`width * height * 4` bytes).
    /// Returns `None` when all frames are exhausted.
    pub fn next_frame(&mut self) -> Option<Vec<u8>> {
        if self.done {
            return None;
        }

        let result = unsafe {
            if !self.started {
                self.started = true;
                smk_first(self.handle)
            } else {
                smk_next(self.handle)
            }
        };

        if result == SMK_ERROR || result == SMK_DONE {
            self.done = true;
            return None;
        }

        // result == SMK_MORE (1) or SMK_LAST (2) — either way we have a frame
        if result == 2 {
            // SMK_LAST: this is the last frame; mark done for next call
            self.done = true;
        }

        Some(self.decode_current_frame())
    }

    fn decode_current_frame(&self) -> Vec<u8> {
        let pixels = self.width as usize * self.height as usize;
        let mut rgba = vec![0u8; pixels * 4];

        unsafe {
            let palette = smk_get_palette(self.handle);
            let video = smk_get_video(self.handle);
            for i in 0..pixels {
                let idx = *video.add(i) as usize;
                rgba[i * 4]     = *palette.add(idx * 3);
                rgba[i * 4 + 1] = *palette.add(idx * 3 + 1);
                rgba[i * 4 + 2] = *palette.add(idx * 3 + 2);
                rgba[i * 4 + 3] = 255;
            }
        }

        rgba
    }
}

impl Drop for SmkDecoder {
    fn drop(&mut self) {
        unsafe { smk_close(self.handle); }
    }
}
```

- [ ] **Step 3: Write a test that opens a real SMK file and decodes the first frame**

Add to the bottom of `openmm-data/src/smk.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::vid::Vid;

    #[test]
    fn smk_decoder_reads_3dologo() {
        let data_path = crate::get_data_path();
        let vid_path = std::path::Path::new(&data_path).join("Anims/Anims2.vid");
        if !vid_path.exists() {
            eprintln!("test: MM6 Anims not found — skipping");
            return;
        }
        let vid = Vid::open(&vid_path).expect("open Anims2.vid");
        let entry = vid.entries.iter().find(|e| e.name.eq_ignore_ascii_case("3dologo"))
            .expect("3dologo not found in Anims2.vid");
        let bytes = vid.smk_bytes(vid.entries.iter().position(|e| e.name.eq_ignore_ascii_case("3dologo")).unwrap()).to_vec();

        let mut dec = SmkDecoder::new(bytes).expect("SmkDecoder::new");
        assert_eq!(dec.width, 640);
        assert_eq!(dec.height, 480);
        assert!(dec.frame_count > 0, "should have frames");
        assert!(dec.fps > 0.0, "fps should be positive");

        let frame = dec.next_frame().expect("first frame should exist");
        assert_eq!(frame.len(), (dec.width * dec.height * 4) as usize);
        // RGBA — alpha must be 255 for all pixels
        assert!(frame.iter().skip(3).step_by(4).all(|&a| a == 255));
    }
}
```

- [ ] **Step 4: Run the test**

```bash
cargo test -p openmm-data smk_decoder_reads_3dologo -- --nocapture 2>&1
```

Expected: `test openmm_data::smk::tests::smk_decoder_reads_3dologo ... ok`

- [ ] **Step 5: Commit**

```bash
git add openmm-data/src/smk.rs openmm-data/src/lib.rs
git commit -S --no-gpg-sign -m "feat(openmm-data): add SmkDecoder — safe Rust wrapper around libsmacker"
```

---

## Task 3: Replace GameState::Splash with GameState::Video

**Files:**
- Modify: `openmm/src/lib.rs`
- Modify: `openmm/src/states/mod.rs`
- Delete: `openmm/src/states/splash.rs`

- [ ] **Step 1: Rename Splash to Video in GameState and update initial state**

In `openmm/src/lib.rs`:

```rust
// Change the imports line (remove SplashPlugin):
use states::{loading::LoadingPlugin, menu::MenuPlugin, video::VideoPlugin};

// Change the GameState enum:
#[derive(Clone, Copy, Default, Eq, PartialEq, Debug, Hash, States)]
pub(crate) enum GameState {
    #[default]
    Video,
    Menu,
    Loading,
    Game,
}

// In GamePlugin::build, replace SplashPlugin with VideoPlugin and update insert_resource/insert_state:
app.insert_resource(cfg)
    .insert_resource(game_assets)
    .insert_resource(game_fonts)
    .insert_resource(save_data)
    .init_resource::<ui_assets::UiAssets>()
    .insert_resource(VideoRequest {
        name: "3dologo".into(),
        skippable: false,
        next: GameState::Menu,
    })
    .add_plugins((BevyConfigPlugin, VideoPlugin, MenuPlugin, LoadingPlugin, InGamePlugin))
    .insert_state(GameState::Video);
```

Also add the import at the top of lib.rs:
```rust
use states::video::{VideoPlugin, VideoRequest};
```

- [ ] **Step 2: Update states/mod.rs**

Replace `openmm/src/states/mod.rs` contents:

```rust
pub(crate) mod loading;
pub(crate) mod menu;
pub(crate) mod video;
```

- [ ] **Step 3: Delete splash.rs**

```bash
rm openmm/src/states/splash.rs
```

- [ ] **Step 4: Check it compiles (video.rs doesn't exist yet — expect a module-not-found error)**

```bash
cargo check -p openmm 2>&1 | grep "error"
```

Expected: error about `video` module not found or `VideoPlugin`/`VideoRequest` not found — that's fine, video.rs is written in the next task.

---

## Task 4: VideoPlugin — `openmm/src/states/video.rs`

**Files:**
- Create: `openmm/src/states/video.rs`

- [ ] **Step 1: Create video.rs with the full VideoPlugin**

Create `openmm/src/states/video.rs`:

```rust
//! Video playback state — plays a single SMK video then transitions to the next state.
//!
//! Set `VideoRequest` resource before entering `GameState::Video`:
//! ```ignore
//! commands.insert_resource(VideoRequest { name: "3dologo".into(), skippable: false, next: GameState::Menu });
//! next_state.set(GameState::Video);
//! ```
use std::path::Path;
use std::time::Duration;

use bevy::prelude::*;
use openmm_data::smk::SmkDecoder;
use openmm_data::vid::Vid;

use crate::GameState;

pub struct VideoPlugin;

impl Plugin for VideoPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Video), video_setup)
            .add_systems(
                Update,
                (video_tick, video_skip).run_if(in_state(GameState::Video)),
            )
            .add_systems(OnExit(GameState::Video), video_cleanup);
    }
}

/// Set this resource before transitioning to `GameState::Video`.
#[derive(Resource)]
pub struct VideoRequest {
    /// SMK name without extension, e.g. `"3dologo"`. Looked up in Anims1.vid then Anims2.vid.
    pub name: String,
    /// If true, pressing ESC skips to `next` immediately.
    pub skippable: bool,
    /// State to transition to when the video ends (or is skipped).
    pub next: GameState,
}

/// Marker for entities spawned by the video state.
#[derive(Component)]
struct OnVideoScreen;

/// Runtime decoder state kept as a resource while video plays.
#[derive(Resource)]
struct VideoPlayer {
    decoder: SmkDecoder,
    image_handle: Handle<Image>,
    /// Seconds elapsed since last frame was shown.
    frame_timer: f32,
    /// Seconds per frame (1 / fps).
    spf: f32,
    skippable: bool,
    next: GameState,
}

fn video_setup(
    mut commands: Commands,
    request: Res<VideoRequest>,
    mut images: ResMut<Assets<Image>>,
) {
    let data_path = openmm_data::get_data_path();
    let anims_dir = Path::new(&data_path).join("Anims");

    // Search Anims1.vid then Anims2.vid for the requested video.
    let smk_bytes = ["Anims1.vid", "Anims2.vid"].iter().find_map(|vid_name| {
        let path = anims_dir.join(vid_name);
        let vid = Vid::open(&path).ok()?;
        let idx = vid.entries.iter().position(|e| e.name.eq_ignore_ascii_case(&request.name))?;
        Some(vid.smk_bytes(idx).to_vec())
    });

    let Some(bytes) = smk_bytes else {
        warn!("VideoPlugin: '{}' not found in any VID archive — skipping to next state", request.name);
        // Can't transition here (no NextState access); mark with a sentinel player.
        // video_tick will handle the None decoder case.
        commands.insert_resource(VideoPlayer {
            decoder: SmkDecoder::new(vec![]).unwrap_err_skip(),
            image_handle: Handle::default(),
            frame_timer: 0.0,
            spf: 0.0,
            skippable: true,
            next: request.next,
        });
        return;
    };

    let mut decoder = match SmkDecoder::new(bytes) {
        Ok(d) => d,
        Err(e) => {
            warn!("VideoPlugin: failed to open '{}': {} — skipping", request.name, e);
            // Create a dummy 1x1 image and immediately end.
            let dummy = Image::new_fill(
                bevy::render::render_resource::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
                bevy::render::render_resource::TextureDimension::D2,
                &[0, 0, 0, 255],
                bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
                bevy::render::render_asset::RenderAssetUsages::MAIN_WORLD | bevy::render::render_asset::RenderAssetUsages::RENDER_WORLD,
            );
            let handle = images.add(dummy);
            commands.spawn((Camera2d, OnVideoScreen));
            commands.insert_resource(VideoPlayer {
                decoder: {
                    // Use a zero-frame sentinel by creating a stub that immediately returns None.
                    // Simplest: use an empty valid decoder via a 1-byte buffer that open_memory rejects —
                    // but we already handled the error above. Instead set done=true via a valid tiny SMK.
                    // Since we can't easily construct a "done" decoder, we abuse frame_timer: set spf=0
                    // so the first tick immediately calls next_frame which returns None.
                    SmkDecoder::new(vec![]).unwrap_or_else(|_| unsafe { std::hint::unreachable_unchecked() })
                },
                image_handle: handle,
                frame_timer: f32::MAX, // force immediate end
                spf: 1.0,
                skippable: true,
                next: request.next,
            });
            return;
        }
    };

    let width = decoder.width;
    let height = decoder.height;
    let spf = if decoder.fps > 0.0 { 1.0 / decoder.fps } else { 0.1 };

    // Allocate the image upfront; we'll overwrite pixel data each frame.
    let image = Image::new_fill(
        bevy::render::render_resource::Extent3d { width, height, depth_or_array_layers: 1 },
        bevy::render::render_resource::TextureDimension::D2,
        &[0, 0, 0, 255],
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        bevy::render::render_asset::RenderAssetUsages::MAIN_WORLD | bevy::render::render_asset::RenderAssetUsages::RENDER_WORLD,
    );
    let image_handle = images.add(image);

    // Decode and display the very first frame immediately.
    if let Some(rgba) = decoder.next_frame() {
        if let Some(img) = images.get_mut(&image_handle) {
            img.data = Some(rgba);
        }
    }

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
        decoder,
        image_handle,
        frame_timer: 0.0,
        spf,
        skippable: request.skippable,
        next: request.next,
    });
}

fn video_tick(
    mut player: ResMut<VideoPlayer>,
    mut next_state: ResMut<NextState<GameState>>,
    mut images: ResMut<Assets<Image>>,
    time: Res<Time>,
) {
    player.frame_timer += time.delta_secs();
    if player.frame_timer < player.spf {
        return;
    }
    player.frame_timer -= player.spf;

    match player.decoder.next_frame() {
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

fn video_skip(
    player: Res<VideoPlayer>,
    mut next_state: ResMut<NextState<GameState>>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    if player.skippable && keys.just_pressed(KeyCode::Escape) {
        next_state.set(player.next);
    }
}

fn video_cleanup(mut commands: Commands, query: Query<Entity, With<OnVideoScreen>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<VideoPlayer>();
}
```

**Note:** The error-handling path with `unwrap_or_else(|_| unsafe { std::hint::unreachable_unchecked() })` is a dead code path (we already returned early). Replace that entire `Err` branch with the simpler version below — rewrite the `Err` arm of `match SmkDecoder::new(bytes)`:

```rust
    Err(e) => {
        warn!("VideoPlugin: failed to open '{}': {} — skipping", request.name, e);
        commands.insert_resource(VideoPlayer {
            image_handle: Handle::default(),
            frame_timer: 0.0,
            spf: 1.0,
            skippable: true,
            next: request.next,
            // Decoder field: we need a valid SmkDecoder that immediately returns None.
            // Trick: use frame_timer = MAX so tick fires immediately but we handle via a flag.
        });
        // We can't easily construct a "done" SmkDecoder without valid data.
        // Instead, remove VideoPlayer immediately and set next state.
        // Use a 1-frame dummy.
        return;
    }
```

Actually, to avoid complexity, simplify the whole `video_setup` error path: if video can't load, just transition immediately in the next tick by storing a flag. Use this clean version instead — **replace the entire `video_setup` function** with:

```rust
fn video_setup(
    mut commands: Commands,
    request: Res<VideoRequest>,
    mut images: ResMut<Assets<Image>>,
) {
    let data_path = openmm_data::get_data_path();
    let anims_dir = std::path::Path::new(&data_path).join("Anims");

    // Search Anims1.vid then Anims2.vid for the requested video.
    let smk_bytes = ["Anims1.vid", "Anims2.vid"].iter().find_map(|vid_name| {
        let path = anims_dir.join(vid_name);
        let vid = openmm_data::vid::Vid::open(&path).ok()?;
        let idx = vid.entries.iter().position(|e| e.name.eq_ignore_ascii_case(&request.name))?;
        Some(vid.smk_bytes(idx).to_vec())
    });

    let decoder = smk_bytes
        .and_then(|b| openmm_data::smk::SmkDecoder::new(b).ok())
        .or_else(|| {
            warn!("VideoPlugin: '{}' not found or decode failed — skipping", request.name);
            None
        });

    let Some(mut decoder) = decoder else {
        // No valid video — insert a zero-duration player that tick() will immediately finish.
        commands.insert_resource(VideoPlayer {
            image_handle: Handle::default(),
            frame_timer: 0.0,
            spf: 0.0,
            skippable: true,
            next: request.next,
            finished: true,
        });
        return;
    };

    let width = decoder.width;
    let height = decoder.height;
    let spf = if decoder.fps > 0.0 { 1.0 / decoder.fps } else { 0.1 };

    let image = Image::new_fill(
        bevy::render::render_resource::Extent3d { width, height, depth_or_array_layers: 1 },
        bevy::render::render_resource::TextureDimension::D2,
        &[0, 0, 0, 255],
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        bevy::render::render_asset::RenderAssetUsages::MAIN_WORLD
            | bevy::render::render_asset::RenderAssetUsages::RENDER_WORLD,
    );
    let image_handle = images.add(image);

    // Decode and display the very first frame.
    if let Some(rgba) = decoder.next_frame() {
        if let Some(img) = images.get_mut(&image_handle) {
            img.data = Some(rgba);
        }
    }

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
        decoder,
        image_handle,
        frame_timer: 0.0,
        spf,
        skippable: request.skippable,
        next: request.next,
        finished: false,
    });
}
```

And update `VideoPlayer` to include `finished: bool` and remove `decoder` from the error path. The full final `VideoPlayer` struct:

```rust
#[derive(Resource)]
struct VideoPlayer {
    decoder: Option<SmkDecoder>,  // None when video failed to load
    image_handle: Handle<Image>,
    frame_timer: f32,
    spf: f32,
    skippable: bool,
    next: GameState,
    finished: bool,
}
```

Update `video_setup` to use `decoder: Some(decoder)` (success) or `decoder: None, finished: true` (failure).

Update `video_tick`:
```rust
fn video_tick(
    mut player: ResMut<VideoPlayer>,
    mut next_state: ResMut<NextState<GameState>>,
    mut images: ResMut<Assets<Image>>,
    time: Res<Time>,
) {
    if player.finished {
        next_state.set(player.next);
        return;
    }
    player.frame_timer += time.delta_secs();
    if player.frame_timer < player.spf {
        return;
    }
    player.frame_timer -= player.spf;

    let Some(ref mut decoder) = player.decoder else {
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
```

> **Implementation note:** Write the final clean version from scratch in video.rs — no need to reproduce the intermediate drafts above. Use `Option<SmkDecoder>` for the decoder field throughout.

- [ ] **Step 2: Verify it compiles**

```bash
cargo check -p openmm 2>&1 | grep "error"
```

Expected: no errors.

- [ ] **Step 3: Run the game and confirm 3dologo plays**

```bash
make run 2>&1 | head -20
```

Expected: game window opens, 3dologo plays (81 frames at ~15fps ≈ 5 seconds), then main menu appears.

- [ ] **Step 4: Commit**

```bash
git add openmm/src/states/video.rs openmm/src/states/mod.rs openmm/src/lib.rs
git rm openmm/src/states/splash.rs
git commit -S --no-gpg-sign -m "feat(openmm): add VideoPlugin, replace Splash with Video state, 3dologo intro"
```

---

## Task 5: Move InGame cleanup — preserve game entities through Video state

**Files:**
- Modify: `openmm/src/game/mod.rs` (remove OnExit cleanup)
- Modify: `openmm/src/states/loading.rs` (add InGame despawn on OnEnter)

- [ ] **Step 1: Remove InGame despawn from OnExit(Game)**

In `openmm/src/game/mod.rs`, remove this line:

```rust
.add_systems(OnExit(GameState::Game), despawn_all::<InGame>);
```

- [ ] **Step 2: Add InGame despawn to OnEnter(Loading) in loading.rs**

In `openmm/src/states/loading.rs`, add `InGame` to the imports and register despawn on enter:

At the top of `LoadingPlugin::build`, the current registration is:
```rust
app.add_systems(OnEnter(GameState::Loading), loading_setup)
    .add_systems(Update, loading_step.run_if(in_state(GameState::Loading)))
    .add_systems(OnExit(GameState::Loading), despawn_all::<InLoading>);
```

Change to:
```rust
app.add_systems(
        OnEnter(GameState::Loading),
        (despawn_all::<crate::game::InGame>, loading_setup).chain(),
    )
    .add_systems(Update, loading_step.run_if(in_state(GameState::Loading)))
    .add_systems(OnExit(GameState::Loading), despawn_all::<InLoading>);
```

The `.chain()` ensures InGame entities are despawned before loading_setup runs (which sets up new map resources).

- [ ] **Step 3: Verify it compiles and game still loads correctly**

```bash
cargo check -p openmm 2>&1 | grep "error"
make run 2>&1 | head -5
```

Expected: no errors, game loads normally after 3dologo + menu.

- [ ] **Step 4: Commit**

```bash
git add openmm/src/game/mod.rs openmm/src/states/loading.rs
git commit -S --no-gpg-sign -m "fix(openmm): move InGame cleanup to OnEnter(Loading) so Video state preserves game entities"
```

---

## Task 6: Add PlayVideo to GameEvent and event_dispatch

**Files:**
- Modify: `openmm-data/src/evt.rs` (add `PlayVideo` variant)
- Modify: `openmm/src/game/event_dispatch.rs` (handle it)

- [ ] **Step 1: Add PlayVideo variant to GameEvent in openmm-data/src/evt.rs**

In `openmm-data/src/evt.rs`, add to the `GameEvent` enum after the `Exit` variant:

```rust
/// Play a video by name. Transitions to GameState::Video; returns to Game when done.
PlayVideo { name: String, skippable: bool },
```

Also add a `Display` arm in the `impl std::fmt::Display for GameEvent` block:
```rust
Self::PlayVideo { name, skippable } => write!(f, "PlayVideo('{}' skippable={})", name, skippable),
```

- [ ] **Step 2: Handle PlayVideo in event_dispatch.rs**

In `openmm/src/game/event_dispatch.rs`, add to the `process_events` match:

```rust
GameEvent::PlayVideo { name, skippable } => {
    commands.insert_resource(crate::states::video::VideoRequest {
        name: name.clone(),
        skippable: *skippable,
        next: GameState::Game,
    });
    transition.game_state.set(GameState::Video);
}
```

Make sure `crate::states::video::VideoRequest` is accessible — add to imports at top of event_dispatch.rs if needed:
```rust
use crate::states::video::VideoRequest;
```

Then use `VideoRequest { name: name.clone(), skippable: *skippable, next: GameState::Game }`.

- [ ] **Step 3: Handle the new enum variant in evt.rs Display + any exhaustive match**

Search for any exhaustive match on `GameEvent` that would now fail to compile:

```bash
cargo check -p openmm-data -p openmm 2>&1 | grep "error\[E"
```

Fix any non-exhaustive match errors by adding `GameEvent::PlayVideo { .. } => {}` (or a proper handler).

- [ ] **Step 4: Verify full build**

```bash
cargo build -p openmm-data -p openmm 2>&1 | grep "error"
```

Expected: no errors.

- [ ] **Step 5: Commit**

```bash
git add openmm-data/src/evt.rs openmm/src/game/event_dispatch.rs
git commit -S --no-gpg-sign -m "feat: add PlayVideo event — trigger video from EVT event dispatch"
```

---

## Task 7: Update docs

**Files:**
- Modify: `CLAUDE.md`
- Modify: `docs/todo.md` (if video playback was listed)

- [ ] **Step 1: Update CLAUDE.md game states section**

In `CLAUDE.md`, find the "Game states" section and update:

```
- `Video` -> `Menu` -> `Loading` -> `Game`
- Video state plays a Smacker (.smk) video from the VID archives, then transitions to `next` from `VideoRequest`.
- `VideoRequest` resource: set `name`, `skippable`, `next` before entering `GameState::Video`.
- `Game → Video → Game` preserves all game entities (InGame cleanup is in `OnEnter(Loading)` not `OnExit(Game)`).
```

- [ ] **Step 2: Update docs/openmm-data-crate.md if it exists**

Add `smk` to the module listing in `docs/openmm-data-crate.md`:
- `openmm_data::smk` — `SmkDecoder`: safe wrapper around vendored libsmacker; decodes SMK2/SMK4 video to RGBA frames

- [ ] **Step 3: Commit**

```bash
git add CLAUDE.md docs/
git commit -S --no-gpg-sign -m "docs: document video playback system and VideoRequest"
```

---

## Verification Checklist

After all tasks complete:

- [ ] `make build` — clean build, no errors
- [ ] `cargo test -p openmm-data smk` — SMK decoder test passes
- [ ] `make run` — 3dologo plays on startup, transitions to main menu
- [ ] `make run map=oute3` — game loads directly (skip_intro path via config or `--skip-intro true`)
- [ ] Mid-game video test: open console (`Tab`), but we can't test PlayVideo from console yet — verify that `GameEvent::PlayVideo` compiles and the event_dispatch arm is reachable
