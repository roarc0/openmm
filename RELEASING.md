# Release Process

This document describes how to create a new release of OpenMM.

## Initial Setup

### Update Repository References

Repository references have been updated to use `roarc0` as the GitHub username.

### Setup Codecov (Optional)

For code coverage reporting:
1. Go to [codecov.io](https://codecov.io/) and sign in with GitHub
2. Add your repository
3. Copy the upload token
4. Add it as a secret named `CODECOV_TOKEN` in your GitHub repository settings

## Creating a Release

Releases are automatically built and published via GitHub Actions when you push a version tag.

### Steps

1. **Update version numbers** (if needed):
   ```bash
   # Update version in openmm/Cargo.toml and openmm-data/Cargo.toml
   vim openmm/Cargo.toml openmm-data/Cargo.toml
   ```

2. **Commit any pending changes**:
   ```bash
   git add .
   git commit -m "Prepare for release vX.Y.Z"
   ```

3. **Create and push a version tag**:
   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```

4. **Wait for GitHub Actions** to build binaries for:
   - Linux (x86_64)
   - Windows (x86_64)
   - macOS (x86_64 and ARM64)

5. The release will appear at: `https://github.com/roarc0/openmm/releases`

## Release Artifacts

Each release includes:
- `openmm-linux-x86_64.tar.gz` - Linux binary
- `openmm-windows-x86_64.zip` - Windows binary
- `openmm-macos-x86_64.tar.gz` - macOS Intel binary
- `openmm-macos-aarch64.tar.gz` - macOS Apple Silicon binary

> [!NOTE]
> Each archive also includes the `openmm/assets` directory, which contains required engine metadata and configuration.

### Game Data (end-user setup)

The binaries do not include MM6 game data. End users must supply their own copy. The engine searches for data in this order:

1. **`OPENMM_PATH_MM6` environment variable** — set it to the MM6 installation directory (the one containing the `data/` subfolder with `.lod` files).
2. **`mm6/` folder next to the binary** — place your MM6 installation as `mm6/` in the same directory as the executable, with original `.lod`, `.snd`, and `.vid` files inside `mm6/data/`. This is the recommended layout for release builds.
3. **`data/mm6/` folder** — fallback path used primarily for development in the source repository (`data/mm6/data/`).

See the [README](README.md#game-data-setup) for a full example.

## Versioning

We follow [Semantic Versioning](https://semver.org/):
- `MAJOR.MINOR.PATCH` (e.g., `1.0.0`)
- Increment MAJOR for breaking changes
- Increment MINOR for new features
- Increment PATCH for bug fixes

Since we're pre-1.0, use `0.MINOR.PATCH` format.
