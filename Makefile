.PHONY: run release clean

# Runs the project in debug mode
run:
	cargo run

# Builds the project in release mode
release:
	cargo build --release

# Cleans build files
clean:
	cargo clean
