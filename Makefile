.PHONY: all flash clean

all:
	cargo build --release

flash: all
	st-flash --connect-under-reset write target/thumbv7em-none-eabihf/release/rfid-stm32h750.bin 0x08000000

clean:
	cargo clean
