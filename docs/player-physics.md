# Player and Physics

## Player Settings

Default values from `PlayerSettings`:

| Field | Default | Notes |
|-------|---------|-------|
| `speed` | 2048 | Walk speed (halved unless `cfg.always_run`) |
| `fly_speed` | 4096 | Used in fly mode |
| `eye_height` | 160 | Camera height above ground |
| `gravity` | 9800 | Units/s² |
| `jump_velocity` | 1300 | Initial upward velocity on jump |
| `collision_radius` | 24 | Player capsule radius |
| `max_slope_height` | 200 | Step-up height for terrain |

## Camera

- FOV: 75° outdoors, 60° indoors (aligned with OpenEnroth values)
- Camera spawns with -8° pitch tilt
- `SpatialListener` (ear gap=4.0) is on the `PlayerCamera` child entity, **not** the `Player` root
- Camera component: `PlayerCamera` marker on the child entity

## Input

- `PlayerInputSet` system set label — all systems that read player position or react to player input must run `.after(PlayerInputSet)`
- Walk speed is half of `settings.speed`; `cfg.always_run` skips the halving
- Fly mode: toggle with F2 or gamepad Select; stored in `WorldState.player.fly_mode`
- `MouseLookEnabled` resource: initialized from `cfg.mouse_look`, toggled at runtime with CapsLock (if `cfg.capslock_toggle_mouse_look`)
- `MouseSensitivity` resource: adjusted with Home (increase) / End (decrease) in 5-unit steps
- Gamepad: left stick moves, right stick looks. Unmapped controllers (e.g. GameSir) expose right stick as LeftZ/RightZ axes — the code has a fallback

## Gravity and Vertical Movement

- `gravity_system` applies gravity, vertical velocity, and ceiling/floor clamping each frame
- Does **not** run when `HudView != World`
- Effective ground Y = `max(terrain_height, bsp_floor_height)`
- Ceiling Y from `BuildingColliders::ceiling_height_at`

## Slope Sliding

- Triggered on outdoor terrain when slope angle > `MAX_SLOPE_ANGLE = 0.6` rad (~35°)
- Slide speed: `SLOPE_SLIDE_SPEED = 4000` units/s
- Not applied indoors

## Building Collision

- `BuildingColliders::resolve_movement` iterates 3× to handle corners
- `MAX_STEP_UP = 50` — walls shorter than this are stepped over
- `CollisionWall`: plane (normal + dist) + XZ polygon for containment (ray-cast + edge distance)
- `CollisionTriangle`: 3 vertices + precomputed AABB + normal for barycentric floor height sampling

## Water

- `WaterMap` resource (outdoor, `cells: Vec<bool>`) + `WaterWalking` resource (toggled by EVT)
- Both are per-map resources inserted by `setup_collision_data`

## Door Collision

- `DoorColliders` resource (indoor): rebuilt each frame from `DoorCollisionFace` data + live door positions
- Used to push the player out of moving doors
