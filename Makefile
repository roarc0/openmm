.PHONY: build run release check clippy fix fmt test lint clean dump_assets dump_sounds test-ci

# --- Build ---

build:
	cargo build -p openmm --features openmm/dev

release:
	cargo build --release -p openmm

run:
ifdef map
	cargo run -p openmm --features openmm/dev -- --map $(map) --skip-intro true
else
	cargo run -p openmm --features openmm/dev
endif

run-release:
	cargo run --release -p openmm

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

dump_assets:
	cargo run --release -p lod --bin dump_assets

dump_sounds:
	cargo run --release -p lod --bin dump_sounds

clean:
	cargo clean

help:
	@echo "Available commands:"
	@echo "  build         - Build the project (debug)"
	@echo "  release       - Build the project (release)"
	@echo "  run           - Run the game (use map=X to load specific map)"
	@echo "  run-release   - Run the game in release mode"
	@echo "  check         - Run cargo check"
	@echo "  clippy        - Run clippy linting"
	@echo "  fix           - Auto-fix clippy and run fmt"
	@echo "  fmt           - Format code"
	@echo "  test          - Run all tests"
	@echo "  lint          - Run format and clippy checks"
	@echo "  test-ci       - Run local CI script"
	@echo "  dump_assets   - Extract assets from LOD"
	@echo "  dump_sounds   - Extract sounds from LOD"
	@echo "  clean         - Clean cargo target directory"
