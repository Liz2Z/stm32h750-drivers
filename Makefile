.PHONY: all flash flash-openocd debug clean bin

all:
	cargo build --release

bin: all
	rust-objcopy -O binary target/thumbv7em-none-eabihf/release/rfid-stm32h750 target/thumbv7em-none-eabihf/release/rfid-stm32h750.bin

# 使用 st-flash 烧录
flash: bin
	st-flash --connect-under-reset write target/thumbv7em-none-eabihf/release/rfid-stm32h750.bin 0x08000000

# 使用 OpenOCD 烧录
flash-openocd: bin
	openocd -f interface/stlink.cfg -f target/stm32h7x.cfg \
		-c "program target/thumbv7em-none-eabihf/release/rfid-stm32h750.bin verify reset exit 0x08000000"

# 启动 OpenOCD GDB 调试服务器
debug:
	openocd -f interface/stlink.cfg -f target/stm32h7x.cfg

# 在另一个终端用 GDB 连接:
# arm-none-eabi-gdb target/thumbv7em-none-eabihf/release/rfid-stm32h750
# (gdb) target remote :3333
# (gdb) load
# (gdb) break main
# (gdb) continue

clean:
	cargo clean
