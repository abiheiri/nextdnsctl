.PHONY: build build-linux build-raspi clean

build:
	cargo build --release

build-linux:
	cross build --release --target x86_64-unknown-linux-gnu

build-raspi:
	cross build --release --target aarch64-unknown-linux-gnu

clean:
	cargo clean
