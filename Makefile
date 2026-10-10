.PHONY: all build examples flash flash-openocd debug clean bin test check fmt

HOST_TARGET := $(shell rustc -vV | sed -n 's/^host: //p')

all: build

build:
	cargo build --release

examples:
	cargo build --release --examples

test:
	cargo test --lib --target $(HOST_TARGET)

fmt:
	cargo fmt --all

check:
	cargo fmt --all -- --check
	cargo clippy --lib --bin stm32h750-drivers -- -D warnings

bin: build
	rust-objcopy -O binary target/thumbv7em-none-eabihf/release/stm32h750-drivers target/thumbv7em-none-eabihf/release/stm32h750-drivers.bin

flash: flash-openocd

flash-openocd: bin
	openocd -f interface/stlink.cfg -f target/stm32h7x.cfg \
		-c "program target/thumbv7em-none-eabihf/release/stm32h750-drivers.bin verify reset exit 0x08000000"

debug:
	openocd -f interface/stlink.cfg -f target/stm32h7x.cfg

clean:
	cargo clean
