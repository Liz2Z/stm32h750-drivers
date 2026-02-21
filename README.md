# STM32H750 ILI9341 Display Driver

使用 Rust 编写的 STM32H750 单片机 ILI9341 屏幕驱动程序，支持硬件 SPI 加速。

## 硬件规格

| 项目 | 规格 |
|------|------|
| 主控芯片 | STM32H750VB |
| 屏幕驱动 | ILI9341 |
| 分辨率 | 240x320 |
| 颜色深度 | 16位 RGB565 |
| 系统时钟 | 96 MHz |
| SPI 时钟 | 1.5 MHz |

## 硬件连接

### SPI2 连接 (硬件 SPI)

| STM32H750 引脚 | 功能 | ILI9341 引脚 |
|---------------|------|-------------|
| PA4 | CS | 片选 |
| PB0 | BLK | 背光控制 |
| PB1 | DC | 数据/命令选择 |
| PB3/AF6 | SPI2_SCK | 时钟 |
| PB4/AF6 | SPI2_MISO | 数据输出 |
| PB5/AF6 | SPI2_MOSI | 数据输入 |

### 电源连接

| ILI9341 引脚 | 连接 |
|-----------|------|
| VCC | 3.3V |
| GND | GND |
| RST | 3.3V (软件复位) |

## 快速开始

### 前置要求

- Rust 工具链 (target: `thumbv7em-none-eabihf`)
- ST-Link V2/V3 调试器
- OpenOCD 或 st-flash 工具

### 安装目标

```bash
rustup target add thumbv7em-none-eabihf
```

### 编译

```bash
make build
# 或
cargo build --release
```

### 烧录

```bash
# 使用 OpenOCD (推荐)
make flash-openocd

# 或使用 st-flash
make flash
```

### GDB 调试

```bash
# 终端 1: 启动 OpenOCD 服务器
make debug

# 终端 2: 连接 GDB
arm-none-eabi-gdb target/thumbv7em-none-eabihf/release/rfid-stm32h750
(gdb) target remote :3333
(gdb) load
(gdb) break main
(gdb) continue
```

## 项目结构

```
src/
├── main.rs              # 主程序入口
├── display.rs           # 软件 SPI 驱动 (备用)
├── display_hardware_spi.rs  # 硬件 SPI 驱动 (当前使用)
└── display.legacy.rs    # 旧版本代码
```

## 技术实现

### 硬件 SPI 寄存器直接操作

由于 `stm32h7xx-hal` 的 SPI 初始化存在兼容性问题，本项目采用直接操作 SPI2 寄存器的方式实现高速通信：

```rust
// 配置 SPI2 为 Master 模式, Mode 0, 8位数据帧
const CR1_MSTR | CR1_SSM | CR1_SSI | CR1_BR
```

### 性能对比

| 操作 | 软件 SPI | 硬件 SPI | 提升 |
|------|---------|---------|-----|
| 清屏 (240x320) | ~500ms | ~50ms | 10x |
| 100x100 矩形 | ~50ms | ~5ms | 10x |

### 图形库集成

使用 `embedded-graphics` 库实现绘图功能：

```rust
use embedded_graphics::{pixelcolor::Rgb565, prelude::*};

// 绘制图形
display.fill_rect(0, 0, 100, 100, Rgb565::RED);
display.clear(Rgb565::BLACK);
```

## 调试输出

程序通过 USART2 (PA2/PA3, 9600 波特率) 输出调试信息：

```
=== STM32H750 Hardware SPI Display ===
Display initialized!
```

## 开发踩坑记录

项目开发过程中遇到的问题和解决方案详见 [CLAUDE.md](./CLAUDE.md)，包括：

- `arm-none-eabi-objcopy` 替代方案
- 链接器脚本配置
- `stm32h7xx-hal` SPI 初始化问题
- embedded_hal 版本冲突
- CS 引脚控制时序问题

## 许可证

MIT License

## 参考资料

- [ILI9341 数据手册](https://cdn-shop.adafruit.com/datasheets/ILI9341.pdf)
- [STM32H7 参考手册](https://www.st.com/resource/en/reference_manual/dm00176879.pdf)
- [embedded-graphics 文档](https://docs.rs/embedded-graphics/)
