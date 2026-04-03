.PHONY: build run release check clippy fix fmt test lint clean dump_assets dump_sounds test-ci

# --- Build ---

build:
	cargo build

release:
	cargo build --release

run:
ifdef map
	cargo run -p openmm -- --map $(map) --skip-intro true
else
	cargo run -p openmm
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
