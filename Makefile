.PHONY: all flash clean bin

all:
	cargo build --release

bin: all
	rust-objcopy -O binary target/thumbv7em-none-eabihf/release/rfid-stm32h750 target/thumbv7em-none-eabihf/release/rfid-stm32h750.bin

flash: bin
	st-flash --connect-under-reset write target/thumbv7em-none-eabihf/release/rfid-stm32h750.bin 0x08000000

clean:
	cargo clean
