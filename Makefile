.PHONY: build run dump_assets clean

build:
	cargo build

run:
	cargo run -p openmm

dump_assets:
	cargo run --release -p lod --bin dump_assets

clean:
	cargo clean
