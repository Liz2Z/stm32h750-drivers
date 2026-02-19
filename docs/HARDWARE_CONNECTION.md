# 硬件连接说明

## STM32H750 与 RC522 连接

### SPI 连接 (SPI2)

| STM32H750 引脚 | 功能 | RC522 引脚 | 说明 |
|---------------|------|-----------|------|
| PB13 | SPI2_SCK | SCK | SPI 时钟 |
| PB14 | SPI2_MISO | MISO | 主设备输入/从设备输出 |
| PB15 | SPI2_MOSI | MOSI | 主设备输出/从设备输入 |
| PB12 | GPIO_Output | NSS | 片选信号 (低电平有效) |
| PE0  | GPIO_Output | RST | 复位信号 (高电平有效) |

### 电源连接

| RC522 引脚 | 连接到 | 电压 |
|-----------|--------|------|
| VCC       | 3.3V   | 3.3V 供电 |
| GND       | GND    | 地线 |
| SDA       | PB12   | 片选信号 |

### SPI 配置参数

- **SPI 模式**: Mode 0 (CPOL=0, CPHA=0)
  - CPOL=0: 时钟空闲时为低电平
  - CPHA=0: 第一个时钟边沿采样数据
- **时钟频率**: 1 MHz
- **数据位宽**: 8 位
- **MSB 优先**: 是

## GH340C 逻辑分析仪连接 (SPI 调试)

### 推荐通道分配

| 逻辑分析仪通道 | 信号 | STM32/RC522 引脚 | 说明 |
|--------------|------|-----------------|------|
| CH0          | SCK  | PB13 / RC522 SCK | SPI 时钟，用于触发 |
| CH1          | MOSI | PB15 / RC522 MOSI | 主设备输出 |
| CH2          | MISO | PB14 / RC522 MISO | 从设备输出 |
| CH3          | NSS  | PB12 / RC522 NSS | 片选信号 |
| CH4          | RST  | PE0 / RC522 RST  | 复位信号 (可选) |

### 逻辑分析仪设置

- **采样率**: 至少 10 MHz (建议 20-50 MHz)
- **触发条件**: SCK 上升沿或 NSS 下降沿
- **协议解码**: SPI (设置正确的极性和相位)

### 逻辑分析仪连接示例

```
GH340C           STM32H750          RC522
-------         ----------         ------
CH0  ---> SCK  ---------------> SCK
CH1  ---> MOSI ---------------> MOSI
CH2  <--- MISO <-------------- MISO
CH3  ---> NSS  ---------------> SDA/NSS
GND  ---> GND  ---------------> GND
```

## 预期的 SPI 时序 (Mode 0)

```
NSS  ─────┐              ┌────────────────
           │              │
SCK  ────┐ └──┐ └──┐ └──┐ └──┐ └──┐ └──┐
       │    │    │    │    │    │    │
MOSI  ──┤   D7  │   D6  │   D5  │   D4  │  ...
           │    │    │    │    │
MISO  ─────┤   D7  │   D6  │   D5  │   D4  │  ...
            ↑    ↑    ↑
            采样点 (上升沿)
```

### 时序要求

- **建立时间 (tSU)**: 数据在时钟上升沿前至少稳定
- **保持时间 (tH)**: 数据在时钟上升沿后至少保持稳定
- **最小时钟周期**: 1 μs (对应 1 MHz)
- **片选建立时间**: NSS 在第一个 SCK 边沿前至少保持低电平一段时间

## 测试建议

### 1. 基本通信测试

使用逻辑分析仪验证：
- [ ] SCK 时钟频率正确 (~1 MHz)
- [ ] SPI 模式正确 (Mode 0)
- [ ] 数据格式正确 (8 位，MSB 优先)
- [ ] NSS 信号正确切换

### 2. RC522 命令测试

验证以下命令的 SPI 通信：
- [ ] 写寄存器命令 (0x02)
- [ ] 读寄存器命令 (0x03)
- [ ] 固件版本查询 (地址 0x37)

### 3. 预期响应

读取固件版本寄存器 (地址 0x37) 时：
- MOSI 发送: `[0x03, 0x37, 0x00]`
- MISO 应返回: `[0x00, 0x00, 0x92]` (0x92 = 版本 2.0)

## 故障排查

### 问题 1: 无 SPI 信号

**检查项**:
- 确认 RC522 电源正常 (3.3V)
- 检查 SPI 引脚连接
- 验证代码中的 GPIO 和 SPI 配置

### 问题 2: SPI 信号正常但无响应

**检查项**:
- 验证 NSS 片选信号时序
- 检查 RC522 RST 复位信号
- 确认 RC522 天线连接
- 读取 RC522 VersionReg 寄存器 (0x37)

### 问题 3: 读卡无反应

**检查项**:
- 使用逻辑分析仪验证 Request 命令
- 检查天线连接和调谐
- 验证卡片类型 (支持 Mifare 1K/4K)
- 增加天线功率 (写 TxControl 寄存器)

## 调试工具

### 推荐软件

1. **PulseView** - 开源逻辑分析仪软件
2. **Sigrok** - 命令行工具
3. **VS Code** - 代码调试

### 命令行烧录

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
// SPI 引脚
let sck = gpiob.pb13.into_alternate::<5>();
let miso = gpiob.pb14.into_alternate::<5>();
let mosi = gpiob.pb15.into_alternate::<5>();

// 控制引脚
let nss = gpiob.pb12.into_push_pull_output();  // 片选
let rst = gpioe.pe0.into_push_pull_output();   // 复位
```

## 参考资源

- [RC522 数据手册](https://www.nxp.com/docs/en/data-sheet/MFRC522.pdf)
- [STM32H7 参考手册](https://www.st.com/resource/en/reference_manual/dm00176879.pdf)
- [SPI 协议说明](https://en.wikipedia.org/wiki/Serial_Peripheral_Interface)
