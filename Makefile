.PHONY: all build examples flash debug clean bin test check check-host check-bench check-features fmt bench-build bench-flash bench-read

GDB_PORT := 3333

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

check-host:
	cargo clippy --lib --target $(HOST_TARGET) -- -D warnings

check-bench:
	cargo clippy --lib --bin stm32h750-drivers --features display-bench -- -D warnings

check-features:
	cargo clippy --lib --bin stm32h750-drivers --all-features -- -D warnings

bin: build
	rust-objcopy -O binary target/thumbv7em-none-eabihf/release/stm32h750-drivers target/thumbv7em-none-eabihf/release/stm32h750-drivers.bin

flash: build
	openocd -f interface/stlink.cfg -f target/stm32h7x.cfg -c "program target/thumbv7em-none-eabihf/release/stm32h750-drivers verify reset exit"

debug:
	openocd -f interface/stlink.cfg -f target/stm32h7x.cfg -c "gdb_port $(GDB_PORT)"

# Separate output keeps the normal ELF available for restoring the application.
bench-build:
	cargo build --release --features display-bench --target-dir target/display-bench
	rust-objcopy -O binary target/display-bench/thumbv7em-none-eabihf/release/stm32h750-drivers target/display-bench/firmware.bin

bench-flash: bench-build
	openocd -f interface/stlink.cfg -f target/stm32h7x.cfg -c "program target/display-bench/thumbv7em-none-eabihf/release/stm32h750-drivers verify reset exit"

# Start make debug in another terminal; read only after the one-shot run finishes.
bench-read:
	arm-none-eabi-gdb -q -batch target/display-bench/thumbv7em-none-eabihf/release/stm32h750-drivers \
		-ex 'target extended-remote :$(GDB_PORT)' -ex 'monitor halt' \
		-ex 'set print pretty on' -ex 'print stm32h750_drivers::bsp::display::bench::DISPLAY_BENCH_RESULTS.0.value' \
		-ex 'monitor resume' -ex 'detach'

clean:
	cargo clean
