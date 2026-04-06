# Video Playback Design

**Date:** 2026-04-05  
**Scope:** SMK video decoding + Bevy video player state  
**Files added/changed:** openmm-data/vendor/libsmacker/, openmm-data/build.rs, openmm-data/src/smk.rs, openmm/src/states/video.rs, openmm/src/lib.rs, openmm/src/game/event_dispatch.rs

---

## Goal

Play Smacker (.smk) videos embedded in VID archives at key points:
- Intro on launch: `3dologo.smk` before main menu
- Mid-game cutscenes triggered from the event system

Videos run in a dedicated Bevy state. Game state is paused (not deallocated) when video plays during gameplay.

---

## 1. libsmacker — Vendored C Library

Source: https://github.com/greg-kennedy/libsmacker (v1.2.0r43)

Vendor two files only:
```
openmm-data/vendor/libsmacker/smacker.c
openmm-data/vendor/libsmacker/smacker.h
```

`openmm-data/build.rs` compiles via the `cc` crate:
```rust
cc::Build::new()
    .file("vendor/libsmacker/smacker.c")
    .compile("smacker");
```

Add `cc` to `[build-dependencies]` in `openmm-data/Cargo.toml`.

The C library is self-contained (no external deps). Builds on Linux, Windows, macOS via `cc`.

---

## 2. Safe Rust Bindings — `openmm-data/src/smk.rs`

Wraps `smk_open_memory` / `smk_first` / `smk_next` / `smk_get_video` / `smk_get_palette` / `smk_close`.

### Raw FFI (private module)
Declare the C signatures as `extern "C"` block — no separate `-sys` crate needed given the tiny surface.

### Safe wrapper
```rust
pub struct SmkDecoder {
    handle: *mut smk_t,
    width: u32,
    height: u32,
    frame_count: u32,
    fps: f32,
    current_frame: u32,
}

impl SmkDecoder {
    /// Takes ownership of the raw SMK bytes for its lifetime.
    pub fn new(data: &[u8]) -> Result<Self, SmkError>
    pub fn width(&self) -> u32
    pub fn height(&self) -> u32
    pub fn frame_count(&self) -> u32
    pub fn fps(&self) -> f32
    /// Decode next frame. Returns RGBA pixels (width * height * 4 bytes).
    /// Returns None when all frames are exhausted.
    pub fn next_frame(&mut self) -> Option<Vec<u8>>
}

impl Drop for SmkDecoder {
    fn drop(&mut self) { unsafe { smk_close(self.handle); } }
}
```

`next_frame` internals:
1. Call `smk_get_palette` → 768-byte RGB palette
2. Call `smk_get_video` → width×height palette indices
3. Convert: `rgba[i] = [palette[idx*3], palette[idx*3+1], palette[idx*3+2], 255]`
4. Advance with `smk_next` (or `smk_first` on frame 0)

`SmkDecoder` is `Send` (the handle owns its data, not shared). This is safe because libsmacker is not thread-safe but we never share the handle across threads.

---

## 3. GameState Changes — `openmm/src/lib.rs`

```rust
pub(crate) enum GameState {
    Video,   // ← replaces Splash (Splash was a placeholder for this)
    Menu,
    Loading,
    Game,
}
```

Initial state: `GameState::Video`.

**Preventing game despawn on `Game → Video`:**  
Game entity cleanup currently lives in `OnExit(GameState::Game)` or uses `StateScoped`. Move all per-map cleanup to `OnEnter(GameState::Loading)` instead. `OnExit(Game)` only pauses `Time<Virtual>` — no despawning. This way `Game → Video → Game` leaves all game entities intact.

---

## 4. VideoRequest Resource

```rust
#[derive(Resource)]
pub(crate) struct VideoRequest {
    pub name: String,       // e.g. "3dologo" — looked up in data/dump/vid/ or VID archive
    pub skippable: bool,
    pub next: GameState,
}
```

Set before transitioning to `GameState::Video`. App startup inserts it pointing at `3dologo` → `GameState::Menu`.

---

## 5. VideoPlugin — `openmm/src/states/video.rs`

Systems:
- `OnEnter(Video)`: load SMK bytes from `data/dump/vid/{name}.smk` (dumped by `dump_vid`), create `SmkDecoder`, store in resource, spawn fullscreen `Camera2d` + `ImageNode` tagged `OnVideoScreen`, create `Image` asset handle.
- `Update` (in Video state): frame timer ticks at `1.0 / fps` seconds. On tick: call `decoder.next_frame()`, write RGBA pixels into the `Image` handle via `assets.insert(handle, image)`. On `None` (video done): transition to `next`.
- ESC handling (in Video state, if `skippable`): detect `KeyCode::Escape` → transition to `next`.
- `OnExit(Video)`: despawn `OnVideoScreen` entities, remove `SmkDecoder` resource.

Frame upload: use `Assets<Image>` with a pre-allocated `Image` (Rgba8UnormSrgb, width×height). Each tick overwrite pixel data in-place rather than reallocating.

---

## 6. Event Dispatch Integration

One-liner to trigger a mid-game video from `event_dispatch.rs`:

```rust
GameEvent::PlayVideo { name, skippable } => {
    commands.insert_resource(VideoRequest { name, skippable, next: GameState::Game });
    next_state.set(GameState::Video);
}
```

`PlayVideo` variant added to `openmm_data::evt::GameEvent` enum.

---

## 7. Launch Sequence

In `GamePlugin::build`:
```rust
app.insert_resource(VideoRequest {
    name: "3dologo".into(),
    skippable: false,
    next: GameState::Menu,
})
.insert_state(GameState::Video);
```

`skip_intro` config flag: if set, initial state is `GameState::Loading` directly (bypass video + menu).

---

## 8. Audio

Deferred. SMK audio tracks exist but are not decoded in this pass. `smk_enable_audio` called with `enable=0` for all tracks during decoder init.

---

## Out of Scope

- Audio decoding from SMK
- Seeking / frame scrubbing
- Video during loading screen
