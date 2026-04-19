# HUD, Rendering, and Shaders

## HUD Camera

- HUD uses a 2D camera (order=1, no clear color, `IsDefaultUiCamera`) that renders on top of the 3D camera (order=0)
- Reference dimensions: `REF_W=640`, `REF_H=480`
- All asset positions scale by `scale_x = window_w / 640` and `scale_y = window_h / 480` independently (non-uniform scaling for widescreen)

## HUD Elements

- Six border pieces (border1–6)
- Tap frames: tap1=morning, tap2=day, tap3=evening, tap4=night
- Compass strip
- 8 directional arrows: mapdir1=N … mapdir8=NW
- Footer strip

## Debug Gizmos

- In debug mode, event gizmo rendering also draws collision overlays:
	- actor collision cylinders (current runtime radius + body height)
	- indoor door collision walls/panels from `DoorColliders`
- This helps diagnose pathing issues where monsters appear to have visual space but are blocked by collision.
- Gizmos render only in the player 3D camera (dedicated render layer), not in screen/UI cameras.

## FooterText

`FooterText` resource has two modes:
- `set(text)` — show a hover hint
- `set_status(text, duration, now)` — show a timed status message that **locks out hover hints** until the timer expires
- `tick(now)` must be called each frame to expire the lock

## StatsBar

- Displays gold and food from `WorldState.game_vars`
- Rendered as bitmapped text using `GameFonts`
- Bitmap font shadow pixels use a dark black alpha of `192` for closer MM6 contrast
- Updates only when values change (change detection)

## Minimap

- 512×512 LOD icon image (e.g. `"oute3"`) scrolled to keep the player dot at center
- Zoom = 3.0×
- Direction arrow selected from 8 frames based on player yaw

## Lighting and Day/Night Cycle

- `WorldState.time_of_day` drives sun position, ambient light, and sky color (see `docs/game-state.md`)
- Full day cycle: `DAY_CYCLE_SECS = 1800` (30 minutes real time)
- Sun color: warm (reddish-orange) at horizon → white at noon. Illuminance: 300–1200 lux during day, 0 at night

### Lighting modes

Controlled by `cfg.lighting`:
- `"enhanced"` — full PBR with directional light; model `base_color = srgb(1.4, 1.4, 1.4)`
- anything else — unlit mode; model `base_color = srgb(0.69, 0.69, 0.69)`

The mode toggle updates both `StandardMaterial` (models) and `TerrainMaterial` (terrain) in the same frame.

## Sky

- Large flat quad above the camera rendered with custom `SkyMaterial` (WGSL shader `shaders/sky.wgsl`) with time-scrolling UVs
- The quad follows the camera each frame
- Indoor maps skip the sky dome and set `ClearColor = BLACK`
- Sky texture name from `Odm.sky_texture`; falls back to `"plansky1"` if empty or missing

## Terrain Material

`TerrainMaterial = ExtendedMaterial<StandardMaterial, WaterExtension>` (defined in `terrain_material.rs`).

`WaterExtension` adds two extra textures:
- `water_texture` — animated water
- `water_mask` — R8 image, white = water pixel

The WGSL shader (`shaders/terrain_water.wgsl`) replaces cyan marker pixels in the terrain atlas with animated water.

Water mask uses **nearest** filtering to keep cell boundaries sharp even when the terrain atlas uses linear filtering.
