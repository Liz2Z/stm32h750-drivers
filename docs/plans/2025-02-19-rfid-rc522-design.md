# RFID-RC522 STM32H750 驱动设计

**日期**: 2025-02-19
**目标**: 使用 Rust + stm32h7xx-hal 在 STM32H750 上实现完整的 RFID-RC522 读写功能

## 1. 项目结构

```
rfid-stm32h750/
├── Cargo.toml              # Rust 项目配置
├── Cargo.lock              # 依赖锁定
├── memory.x                # 内存布局配置
├── .cargo/
│   └── config.toml         # Cargo 配置（目标选择）
├── src/
│   ├── main.rs             # 主程序入口、初始化、应用逻辑
│   ├── rc522.rs            # RC522 驱动模块（使用 embedded-hal SPI trait）
│   └── types.rs            # 共享类型定义
└── Makefile                # 构建和烧录命令
```

**架构变更**: 使用 `stm32h7xx-hal` 替代 bare-metal PAC，简化 GPIO 和 SPI 配置。

## 2. 硬件引脚分配

| 功能 | STM32H750 引脚 | RC522 引脚 | 说明 |
|------|---------------|-----------|------|
| SPI_SCK | PB13 | SCK | 时钟 |
| SPI_MISO | PB14 | MISO | 主入从出 |
| SPI_MOSI | PB15 | MOSI | 主出从入 |
| SPI_NSS | PB12 | SDA | 片选（软件控制） |
| RC522_RST | PE0 | RST | 复位 |
| 3.3V | - | 3.3V | 电源 |
| GND | - | GND | 地 |

**SPI 配置**:
- 模式：SPI Mode 0 (CPOL=0, CPHA=0)
- 数据位：8 位
- 时钟频率：初始化时 < 1MHz，正常工作 < 10MHz

## 3. RC522 驱动模块 API

```rust
pub struct RC522<SPI> {
    spi: SPI,
    nss: PE0<Output>,
}

impl<SPI, E> RC522<SPI> where SPI: Transfer<u8, Error = E> {
    // 初始化
    pub fn new(spi: SPI, nss: PE0<Output>) -> Result<Self, E>;
    pub fn reset(&mut self) -> Result<(), E>;

    // 寄存器操作
    fn read_reg(&mut self, reg: u8) -> Result<u8, E>;
    fn write_reg(&mut self, reg: u8, val: u8) -> Result<(), E>;

    // RFID 功能
    pub fn request(&mut self) -> Result<CardType, Error<E>>;
    pub fn anticoll(&mut self) -> Result<[u8; 4], Error<E>>;
    pub fn select_card(&mut self, uid: &[u8; 4]) -> Result<(), Error<E>>;
    pub fn authenticate(&mut self, block: u8, key_type: KeyType, key: &[u8; 6]) -> Result<(), Error<E>>;
    pub fn read(&mut self, block: u8) -> Result<[u8; 16], Error<E>>;
    pub fn write(&mut self, block: u8, data: &[u8; 16]) -> Result<(), Error<E>>;
}
```

## 4. 主程序流程

```rust
fn main() -> ! {
    // 1. 初始化时钟、GPIO、SPI
    // 2. 初始化 RC522
    // 3. 循环：寻卡 → 获取UID → 认证 → 读写

    loop {
        if let Ok(CardType::Mifare1K) = rc522.request() {
            if let Ok(uid) = rc522.anticoll() {
                if rc522.select_card(&uid).is_ok() {
                    if rc522.authenticate(4, KeyType::KeyA, &DEFAULT_KEY).is_ok() {
                        let data = rc522.read(4)?;
                        // 处理数据
                    }
                }
            }
        }
        delay_ms(500);
    }
}
```

## 5. 依赖配置

```toml
[dependencies]
stm32h7xx-hal = { version = "0.16", features = ["stm32h750v", "rt", "spi"] }
cortex-m = { version = "0.7", features = ["critical-section-single-core"] }
cortex-m-rt = "0.7"
embedded-hal = "1.0"
panic-halt = "1.0"
nb = "1.0"
```

## 6. 调试计划

使用 GH340C 逻辑分析仪：
1. SPI 通信验证（SCK/MOSI/MISO/NSS 时序）
2. RC522 寄存器读写验证
3. 卡片交互完整流程验证

分步测试：SPI HAL → RC522 初始化 → 寻卡 → 完整读写
