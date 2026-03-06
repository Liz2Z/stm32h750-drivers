.PHONY: all build flash flash-openocd debug clean bin

all: build

build:
	cargo build --release

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
