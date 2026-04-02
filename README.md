# OpenMM

[![CI](https://github.com/roarc0/openmm/actions/workflows/ci.yml/badge.svg)](https://github.com/roarc0/openmm/actions/workflows/ci.yml)
[![Release](https://github.com/roarc0/openmm/actions/workflows/release.yml/badge.svg)](https://github.com/roarc0/openmm/actions/workflows/release.yml)
[![codecov](https://codecov.io/gh/roarc0/openmm/branch/main/graph/badge.svg)](https://codecov.io/gh/roarc0/openmm)

Open-source reimplementation of the Might and Magic VI engine in Rust 🦀

Note: OpenMM is a fan project and is not affiliated with Ubisoft or New World Computing. It's a tribute to the timeless joy of the original game.

## Features

- Terrain rendering with textures (outdoor maps)
- Building rendering with BSP models
- Billboards (trees, rocks, fountains)
- NPCs and monsters with directional sprites and wander AI
- Player movement with terrain following and first-person camera
- Indoor map rendering (BLV files)
- Door interaction and animation
- Loading screens and map transitions
- Developer console (Tab key)

## Requirements

- **Might and Magic VI game data files** (you must own a copy of the original game)
- Rust toolchain (see Build section)
- System dependencies:
  - **Linux**: libasound2-dev, libudev-dev, pkg-config
  - **macOS**: No additional dependencies
  - **Windows**: No additional dependencies

## Installation

### Download Pre-built Binaries (Recommended)

1. Download the latest release for your platform from the [Releases page](https://github.com/roarc0/openmm/releases)
2. Extract the archive
3. Place your Might and Magic VI game data files in the correct location (see Setup Game Data below)
4. Run the executable

### Setup Game Data

You need the original Might and Magic VI game data files. Set the `OPENMM_6_PATH` environment variable to point to your MM6 installation directory, or place the data files in `./target/mm6/data/`:

```bash
# Option 1: Set environment variable
export OPENMM_6_PATH=/path/to/your/mm6/installation

# Option 2: Create default directory and copy files
mkdir -p ./target/mm6/data
cp /path/to/your/mm6/*.lod ./target/mm6/data/
```

The engine looks for LOD archive files (games.lod, bitmaps.lod, icons.lod, sprites.lod, etc.) in this directory.

## Build from Source

### Prerequisites

Install Rust via [rustup](https://rustup.rs/):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### System Dependencies

**Linux (Debian/Ubuntu):**
```bash
sudo apt-get install libasound2-dev libudev-dev pkg-config
```

**Linux (Arch):**
```bash
sudo pacman -S alsa-lib systemd-libs pkg-config
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

# Or with specific map
./target/release/openmm --map oute3
```

### Controls

- **Arrow Keys**: Move forward/back and rotate
- **A/D**: Strafe left/right
- **Mouse**: Look around
- **ESC**: Toggle cursor grab
- **Tab**: Open developer console

### Developer Console Commands

Press **Tab** to open the console, then type commands:

- `load <map>` - Load a map (e.g., `load oute3`, `load d01`)
- `msaa <value>` - Set MSAA samples (0, 2, 4, 8)
- `fullscreen` - Toggle fullscreen mode
- `borderless` - Toggle borderless window
- `windowed` - Switch to windowed mode
- `exit` - Exit the game

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
