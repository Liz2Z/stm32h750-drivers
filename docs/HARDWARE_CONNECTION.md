# 硬件连接说明

## STM32H750 与 ILI9341 屏幕连接

### SPI 连接 (软件 SPI)

| STM32H750 引脚 | 功能 | ILI9341 引脚 | 说明 |
|---------------|------|-------------|------|
| PB0 | GPIO_Output | BLK | 背光控制 (高电平点亮) |
| PB1 | GPIO_Output | RS/D/C | 数据/命令选择 |
| PB12 | GPIO_Output | CS | 片选信号 (低电平有效) |
| PB13 | GPIO_Output | SCK/SCL | SPI 时钟 |
| PB14 | GPIO_Input | MISO/SDO | SPI 数据输出 |
| PB15 | GPIO_Output | MOSI/SDI | SPI 数据输入 |

### 电源连接

| ILI9341 引脚 | 连接到 | 电压 |
|-----------|--------|------|
| VCC | 3.3V/5V | 根据屏幕规格 |
| GND | GND | 地线 |
| RST | 3.3V | 复位引脚接高电平（不复位） |

### 屏幕规格

- **分辨率**: 240x320
- **颜色深度**: 16位 (RGB565)
- **接口**: SPI
- **驱动芯片**: ILI9341
- **注意**: 复位引脚(RST)直接接 VCC，使用软件复位初始化

## 逻辑分析仪连接 (SPI 调试)

### 推荐通道分配

| 逻辑分析仪通道 | 信号 | STM32 引脚 | 说明 |
|--------------|------|-----------|------|
| CH0 | SCK | PB13 | SPI 时钟 |
| CH1 | MOSI | PB15 | 主设备输出 |
| CH2 | MISO | PB14 | 从设备输出 |
| CH3 | CS | PB12 | 片选信号 |
| CH4 | DC | PB1 | 数据/命令选择 |

### 逻辑分析仪设置

- **采样率**: 至少 10 MHz
- **触发条件**: CS 下降沿
- **协议解码**: SPI (Mode 0)

## 命令行烧录

```bash
# 生成 .bin 文件
make bin

# 烧录到 STM32H750
make flash

# 或使用 st-flash 直接烧录
st-flash --connect-under-reset write target/thumbv7em-none-eabihf/release/rfid-stm32h750.bin 0x08000000
```

## 代码中的引脚配置

参考 `src/main.rs` 中的配置:

```rust
// 屏幕引脚配置
let mut disp_blk = gpiob.pb0.into_push_pull_output();  // 背光
let disp_dc = gpiob.pb1.into_push_pull_output();       // 数据/命令
let disp_cs = gpiob.pb12.into_push_pull_output();      // 片选
let disp_sck = gpiob.pb13.into_push_pull_output();     // 时钟
let disp_miso = gpiob.pb14.into_input();               // MISO
let disp_mosi = gpiob.pb15.into_push_pull_output();    // MOSI
```

## 参考资源

- [ILI9341 数据手册](https://cdn-shop.adafruit.com/datasheets/ILI9341.pdf)
- [STM32H7 参考手册](https://www.st.com/resource/en/reference_manual/dm00176879.pdf)
- [embedded-graphics 文档](https://docs.rs/embedded-graphics/)
