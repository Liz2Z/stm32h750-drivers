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

## STM32H7 SPI 初始化挂起问题

### 问题现象

使用 `stm32h7xx-hal` 的 `.spi()` 方法初始化 SPI1/SPI2 时程序挂起：
- LED 快闪后常亮（程序在 SPI 初始化时卡死）
- 串口无输出
- SPI1 和 SPI2 都有同样的问题

### 调试过程

1. ✅ 只配置 GPIO 引脚（不初始化 SPI 外设）- 正常
2. ✅ 只获取 `ccdr.peripheral.SPI1` - 正常
3. ✅ 读取 SPI 寄存器 - 正常
4. ❌ 调用 `.spi()` 方法 - 挂起

尝试过的解决方案：
- 配置 VOS1 电源模式 - 无效
- 降低系统时钟到 96MHz - 无效
- 在初始化前复位 SPI 外设 - 无效

### 根本原因

`stm32h7xx-hal` 的 `.spi()` 方法内部有问题，可能与 H7 系列的特殊寄存器结构或时钟配置有关。

### 解决方案：使用软件 SPI

放弃硬件 SPI，使用 GPIO 模拟 SPI 协议：

```rust
struct SoftSpi {
    sck: PB13<Output<PushPull>>,
    mosi: PB15<Output<PushPull>>,
    miso: PB14<Input>,
    cs: PB12<Output<PushPull>>,
}

impl SoftSpi {
    fn transfer(&mut self, data: u8) -> u8 {
        let mut received = 0u8;
        self.cs.set_low();

        for i in (0..8).rev() {
            // 写入 MOSI
            if (data >> i) & 1 == 1 {
                self.mosi.set_high();
            } else {
                self.mosi.set_low();
            }

            // 时钟上升沿
            for _ in 0..10 { cortex_m::asm::nop(); }
            self.sck.set_high();

            // 读取 MISO
            if self.miso.is_high() {
                received |= 1 << i;
            }

            // 时钟下降沿
            for _ in 0..10 { cortex_m::asm::nop(); }
            self.sck.set_low();
        }

        self.cs.set_high();
        received
    }
}
```

### 关键配置

- **VOS**: `pwr.vos1().freeze()` - 配置电源缩放
- **时钟**: `rcc.sys_ck(96.MHz()).freeze()` - 保守的 96MHz
- **SPI 引脚**: PB12(CS), PB13(SCK), PB14(MISO), PB15(MOSI)
- **SPI 模式**: Mode 0 (CPOL=0, CPHA=0)

## Task 13: 添加 ILI9341 屏幕支持

### 问题 1: embedded_hal 版本冲突

**现象**: 编译时报错 trait bound 不满足

**原因**:
- `stm32h7xx-hal` 使用的是 `embedded-hal` 0.2
- 最初 `Cargo.toml` 中指定的是 `embedded-hal` 1.0
- 两个版本的 trait 不兼容

**解决方案**:
将 `Cargo.toml` 中的 `embedded-hal` 版本改为 0.2：
```toml
embedded-hal = "0.2"
```

### 问题 2: embedded_hal 0.2 中 InputPin trait 的路径问题

**现象**: `use embedded_hal::digital::InputPin` 无法找到 `is_high()` 方法

**原因**:
- `embedded-hal` 0.2 中有两个版本的 `InputPin`：
  - `embedded_hal::digital::InputPin` (v1, 已弃用)
  - `embedded_hal::digital::v2::InputPin` (v2, 推荐使用)
- stm32h7xx-hal 实现的是 v2 版本

**解决方案**:
使用 v2 版本的 trait：
```rust
use embedded_hal::digital::v2::{InputPin, OutputPin};

// 在 impl 块中指定 Error 类型
impl<SCK, MOSI, MISO, CS, DC, E> DisplaySpi<SCK, MOSI, MISO, CS, DC>
where
    SCK: OutputPin<Error = E>,
    MOSI: OutputPin<Error = E>,
    MISO: InputPin<Error = E>,
    CS: OutputPin<Error = E>,
    DC: OutputPin<Error = E>,
    E: core::fmt::Debug,
```

### 问题 3: ILI9341 初始化序列

**现象**: 屏幕初始化后无显示

**原因**:
- ILI9341 需要严格的初始化序列
- 需要正确的 Gamma 校正参数
- 需要设置正确的颜色格式 (16位 RGB565)

**解决方案**:
参考 `src/display.rs` 中的 `init()` 方法，包含：
1. 软件复位 (0x01 命令)
2. 退出睡眠模式 (0x11)
3. 设置帧率控制 (0xB1-0xB3)
4. 设置电源控制 (0xC0-0xC5)
5. 设置 VCOM 控制 (0xC5)
6. 设置颜色格式为 16位 (0x3A = 0x55)
7. 设置扫描方向 (0x36)
8. 设置 Gamma 校正 (0xE0, 0xE1)
9. 打开显示 (0x29)

### 屏幕连接配置

| STM32H750   | ILI9341 | 说明      |
| ----------- | ------- | --------- |
| PB15/MOSI   | SDI     | 数据输入  |
| PB13/SCK    | SCL     | 时钟      |
| PB12/CS     | CS      | 片选      |
| PB14/MISO   | SDO     | 数据输出  |
| PB1/RS      | D/C     | 数据/命令 |
| PB0/BLK     | BLK     | 背光控制  |

**注意**: 复位引脚(RST)直接接 VCC，使用软件复位初始化
