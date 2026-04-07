.PHONY: build run release release-native run-release run-release-native check clippy fix fmt test lint clean dump_assets dump_sounds dump_saves dump_vid check-data test-ci profile-chrome profile-tracy

# --- Build ---

build:
	cargo build -p openmm --features openmm/dev

release:
	cargo build --release -p openmm

release-native:
	RUSTFLAGS="-C target-cpu=native $$RUSTFLAGS" cargo build --release -p openmm

run:
ifdef map
	cargo run -p openmm --features openmm/dev -- --map $(map) --skip-intro true
else
	cargo run -p openmm --features openmm/dev
endif

run-release:
	cargo run --release -p openmm

run-release-native:
	RUSTFLAGS="-C target-cpu=native $$RUSTFLAGS" cargo run --release -p openmm

# --- Profiling ---
# Chrome tracing: opens a JSON file in https://ui.perfetto.dev
profile-chrome:
ifdef map
	cargo run --profile profiling -p openmm --features bevy/trace_chrome -- --map $(map) --skip-intro true
else
	cargo run --profile profiling -p openmm --features bevy/trace_chrome -- --skip-intro true
endif

# Tracy: start Tracy UI first (connect mode), then run this.
# Required Tracy GUI version: 0.11.x  (matches tracy-client v0.18.4)
# Install: https://github.com/wolfpld/tracy/releases  or  pacman -S tracy
profile-tracy:
ifdef map
	cargo run --profile profiling -p openmm --features bevy/trace_tracy -- --map $(map) --skip-intro true
else
	cargo run --profile profiling -p openmm --features bevy/trace_tracy -- --skip-intro true
endif

# --- Quality ---

check:
	cargo check --workspace

clippy:
	cargo clippy --workspace -- -W clippy::all

fix:
	cargo clippy --fix --allow-dirty --workspace -- -W clippy::all
	cargo fmt --all
	cargo clippy --workspace -- -W clippy::all

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

test:
	cargo test --workspace

lint: fmt-check clippy
	@echo "All lint checks passed"

test-ci:
	bash scripts/test_ci.sh

# --- Tools ---

dump: dump_assets dump_sounds dump_saves dump_vid

dump_assets:
	cargo run --release -p openmm-data --bin dump_assets

dump_sounds:
	cargo run --release -p openmm-data --bin dump_sounds

dump_saves:
	cargo run --release -p openmm-data --bin dump_saves

dump_vid:
	cargo run --release -p openmm-data --bin dump_vid

clean:
	cargo clean

check-data:
	cargo run --release -p openmm-data --example data_roundtrip

help:
	@echo "Available commands:"
	@echo "  build               - Build the project (debug)"
	@echo "  release             - Build the project (release, portable)"
	@echo "  release-native      - Build release with -C target-cpu=native (local only, faster)"
	@echo "  run                 - Run the game (use map=X to load specific map)"
	@echo "  run-release         - Run the game in release mode"
	@echo "  run-release-native  - Run release-native (local only, faster)"
	@echo "  check         - Run cargo check"
	@echo "  clippy        - Run clippy linting"
	@echo "  fix           - Auto-fix clippy and run fmt"
	@echo "  fmt           - Format code"
	@echo "  test          - Run all tests"
	@echo "  lint          - Run format and clippy checks"
	@echo "  test-ci       - Run local CI script"
	@echo "  dump_assets   - Extract assets from LOD"
	@echo "  dump_sounds   - Extract sounds from LOD"
	@echo "  dump_saves    - Dump MM6 save files to data/dump/saves/"
	@echo "  dump_vid      - Extract SMK videos from Anims VID archives"
	@echo "  check-data    - Run full data round-trip verification (generates ./data/mm6_serialized/)"
	@echo "  clean         - Clean cargo target directory"
	@echo "  profile-chrome - Run with Chrome/Perfetto tracing (open .json at ui.perfetto.dev)"
	@echo "  profile-tracy  - Run with Tracy profiler (start Tracy UI first)"
