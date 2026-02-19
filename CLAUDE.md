# 项目踩坑记录

## Task 12: 烧录测试

### 问题 1: arm-none-eabi-objcopy 不可用

**现象**: 执行 `arm-none-eabi-objcopy` 命令时报错 `command not found`

**原因**: macOS 系统默认没有安装 ARM 交叉编译工具链

**解决方案**:
- 使用 Rust 工具链自带的 `rust-objcopy` 替代
- `rust-objcopy` 功能与 `arm-none-eabi-objcopy` 相同
- 更新 Makefile 使用 `rust-objcopy`

### 问题 2: .bin 文件生成后为 0 字节

**现象**: 使用 `rust-objcopy` 生成 .bin 文件后，文件大小为 0 字节

**原因**: 链接器脚本 (`memory.x`) 没有被正确加载
- 入口地址为 `0x0` 而不是 `0x08000000`
- 代码被加载到 `0x10000` 而不是 Flash 地址

**解决方案**:
在 `.cargo/config.toml` 中添加 `rustflags` 配置:

```toml
[target.thumbv7em-none-eabihf]
rustflags = [
  "-C", "link-arg=-Tlink.x",
]
```

这会告诉 Cargo 使用链接器脚本 `link.x`（由 `cortex-m-rt` 提供），该脚本会自动包含项目根目录下的 `memory.x` 配置。

### 问题 3: 重新编译后 .bin 文件才正常

**现象**: 修改配置后需要 `cargo clean` 再重新编译

**原因**: Cargo 会缓存之前的编译结果，链接器配置更改后需要重新链接

**解决方案**:
```bash
cargo clean
cargo build --release
```

## 项目配置总结

### 关键配置文件

1. **`.cargo/config.toml`**: 指定 target 和 rustflags
2. **`memory.x`**: 定义内存布局（Flash 和 RAM）
3. **`Makefile`**: 简化编译和烧录流程

### 烧录工作流

```bash
# 方法 1: 使用 Makefile
make flash

# 方法 2: 分步执行
make bin              # 生成 .bin 文件
make flash           # 烧录到设备

# 方法 3: 手动执行
cargo build --release
rust-objcopy -O binary target/thumbv7em-none-eabihf/release/rfid-stm32h750 target/thumbv7em-none-eabihf/release/rfid-stm32h750.bin
st-flash --connect-under-reset write target/thumbv7em-none-eabihf/release/rfid-stm32h750.bin 0x08000000
```

### 硬件连接要点

- **SPI2**: PB13(SCK), PB14(MISO), PB15(MOSI), PB12(NSS)
- **RST**: PE0
- **SPI 模式**: Mode 0 (CPOL=0, CPHA=0)
- **时钟频率**: 1 MHz
- **逻辑分析仪**: 建议 CH0=SCK, CH1=MOSI, CH2=MISO, CH3=NSS

## 调试建议

1. 使用逻辑分析仪验证 SPI 信号
2. 读取 RC522 版本寄存器 (0x37) 验证通信
3. 确认 3.3V 供电和天线连接
4. 检查片选和复位信号时序
