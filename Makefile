.PHONY: build run dump_assets clean

build:
	cargo build

run:
ifdef map
	cargo run -p openmm -- --map $(map) --skip-intro true
else
	cargo run -p openmm
endif

dump_assets:
	cargo run --release -p lod --bin dump_assets

clean:
	cargo clean
