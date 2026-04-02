# OpenMM

[![CI](https://github.com/roarc0/openmm/actions/workflows/ci.yml/badge.svg)](https://github.com/roarc0/openmm/actions/workflows/ci.yml)
[![Release](https://github.com/roarc0/openmm/actions/workflows/release.yml/badge.svg)](https://github.com/roarc0/openmm/actions/workflows/release.yml)
[![codecov](https://codecov.io/gh/roarc0/openmm/branch/main/graph/badge.svg)](https://codecov.io/gh/roarc0/openmm)

Open-source reimplementation of the Might and Magic VI engine in Rust 🦀

The goal is to reproduce original MM6 gameplay — movement, combat, dialogue, quests — with clean, maintainable code. Graphical improvements are welcome where they enhance the experience without compromising accuracy.

> OpenMM is a fan project, not affiliated with Ubisoft or New World Computing. You must own a copy of Might and Magic VI to use it.

## Current Features

- Terrain rendering with textures (outdoor maps / ODM files)
- BSP model rendering (buildings) with textures
- Billboards (decorations: trees, rocks, fountains) with sprite caching
- NPCs and monsters with directional sprites, wander AI, and animation
- Player entity with terrain-following movement and first-person camera
- Loading screen with step-based map loader and sprite preloading
- Splash screen and menu scaffolding
- Developer console (Tab key) with commands: load, msaa, fullscreen, borderless, windowed, exit
- Seamless map boundary transitions between adjacent outdoor zones
- Indoor map rendering (BLV files) with face-based geometry and collision
- Indoor door interaction: clickable faces dispatch EVT events, door animation state machine

## Requirements

- **Might and Magic VI game data files** (you must own a copy of the original game)
- Rust toolchain (see Build section)
- System dependencies:
  - **All platforms**: Rust toolchain [rustup](https://rustup.rs/)
  - **Linux**: libasound2-dev, libudev-dev, pkg-config, libwayland-dev, libxkbcommon-dev
  - **macOS / Windows**: No additional system dependencies

## Installation

### Download Pre-built Binaries (Recommended)

1. Download the latest release for your platform from the [Releases page](https://github.com/roarc0/openmm/releases)
2. Extract the archive
3. Place your Might and Magic VI game data files in the correct location (see Setup Game Data below)
4. Run the executable

## Game Data Setup

OpenMM requires the original Might and Magic VI data files. These are the `.lod` archive files from your MM6 installation (`games.lod`, `bitmaps.lod`, `icons.lod`, `sprites.lod`, etc.).

**Option 1 — Environment variable (recommended):**
```bash
export OPENMM_6_PATH=/path/to/your/mm6/installation
```

**Option 2 — Default directory:**
```bash
mkdir -p ./target/mm6/data
cp /path/to/your/mm6/*.lod ./target/mm6/data/
```

The engine looks for LOD files in the directory pointed to by `OPENMM_6_PATH`, defaulting to `./target/mm6/data/`.

> MM6 is available on [GOG](https://www.gog.com). The GOG version installs directly to a folder you can point `OPENMM_6_PATH` at.

## Build from Source

### Prerequisites

Install Rust via [rustup](https://rustup.rs/):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### System Dependencies

**Linux (Debian/Ubuntu):**
```bash
sudo apt-get install libasound2-dev libudev-dev pkg-config libwayland-dev libxkbcommon-dev
```

**Linux (Arch):**
```bash
sudo pacman -S alsa-lib systemd-libs pkg-config wayland libxkbcommon
```

**Optional: Install mold linker for faster builds (Linux only):**
```bash
# Arch
sudo pacman -S mold

# Ubuntu/Debian
sudo apt install mold
```

### Build Commands

```bash
make build        # Debug build
make release      # Optimized release build
make run          # Run debug build
make run map=oute3  # Run with specific map
make test         # Run tests
make lint         # Check formatting and linting
make fmt          # Auto-format code
make clippy       # Run clippy linter
```

Or use cargo directly:

```bash
cargo build --release
cargo run --release
```

## Running the Game

After building, run the executable:

```bash
# From workspace root
./target/release/openmm

# Start on a specific map
./target/release/openmm --map oute3
```

## Controls

| Input | Action |
|-------|--------|
| **W / Up Arrow** | Move forward |
| **S / Down Arrow** | Move backward |
| **A / D** | Strafe left / right |
| **Left / Right Arrow** | Rotate left / right |
| **Mouse** | Look around |
| **E / Enter** | Interact (talk to NPC, enter building, open door) |
| **F2** | Toggle fly mode |
| **CapsLock** | Toggle mouse look |
| **Home / End** | Increase / decrease mouse sensitivity |
| **ESC** | Release / grab mouse cursor |
| **Tab** | Open developer console |

## Developer Console

Press **Tab** to open the console (requires `console = true` in `openmm.toml`).

**Navigation**

| Command | Description |
|---------|-------------|
| `load <map>` | Load an outdoor map (`oute3`, `outb2`, …) or indoor dungeon (`d01`–`d20`, and more) |
| `reload` | Reload the current map |
| `go <north\|south\|east\|west>` | Move to the adjacent outdoor zone |
| `pos` | Print current position and yaw |

**Player**

| Command | Description |
|---------|-------------|
| `fly` | Toggle fly mode (no gravity, free vertical movement) |
| `speed <value>` | Set movement speed (default 2048) |
| `sensitivity <value>` | Set mouse sensitivity |

**Rendering**

| Command | Description |
|---------|-------------|
| `lighting <enhanced\|flat>` | Toggle PBR / unlit rendering |
| `fog <start> <end>` | Set fog distances |
| `draw_distance <value>` | Set entity draw distance |
| `msaa <off\|2\|4\|8>` | Set MSAA sample count |
| `wireframe` | Toggle wireframe mode |
| `shadows` | Toggle shadows |
| `bloom` | Toggle bloom |
| `ssao` | Toggle ambient occlusion |
| `filtering <nearest\|linear>` | Set global texture filtering |
| `tonemapping <none\|aces\|agx\|…>` | Set tonemapping operator |
| `exposure <value>` | Set camera exposure |

**Audio**

| Command | Description |
|---------|-------------|
| `music <vol>` | Set music volume (0.0–1.0) |
| `sfx <vol>` | Set sound effects volume (0.0–1.0) |
| `mute` | Mute all audio |
| `unmute` | Unmute all audio |

**Window**

| Command | Description |
|---------|-------------|
| `fullscreen` | Switch to fullscreen |
| `borderless` | Switch to borderless fullscreen |
| `windowed` | Switch to windowed mode |
| `vsync <on\|off\|fast>` | Set vsync mode |

**Debug / Misc**

| Command | Description |
|---------|-------------|
| `debug` | Toggle debug HUD overlay |
| `clear` | Clear console output |
| `help` | List all available commands |
| `exit` | Quit the game |

Type `help` in-game for the current full list.

## Project Structure

- **`lod/`** - Library for reading MM6 data formats (LOD archives, maps, sprites, etc.)
- **`openmm/`** - Bevy game engine application
- **`docs/`** - Technical documentation
- **`assets/`** - Asset metadata and configuration files

## Development

See [CLAUDE.md](CLAUDE.md) for detailed development documentation, architecture, and conventions.

### Contributing

Contributions are welcome! Please ensure:
- Code passes `make lint` (formatting and clippy checks)
- Tests pass with `make test`
- Follow the conventions documented in CLAUDE.md

See [COVERAGE.md](COVERAGE.md) for information about code coverage setup.

### Creating Releases

See [RELEASING.md](RELEASING.md) for instructions on creating new releases. The CI pipeline automatically builds binaries for Linux, Windows, and macOS when you push a version tag.

## License

This is a fan project. You must own a copy of the original Might and Magic VI to use this software.
