# 硬件 SPI + DMA 驱动 ILI9341 实现计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**目标:** 将软件 SPI 替换为 STM32H7 硬件 SPI2 + DMA，大幅提升屏幕刷新速度。

**架构:** 使用 stm32h7xx-hal 的 SPI2 外设配合 DMA2 Stream1 实现高速传输。采用渐进式策略：先实现基础硬件 SPI，验证通过后再添加 DMA。

**技术栈:** stm32h7xx-hal, embedded-hal, embedded-graphics

---

## Task 1: 修改 main.rs - 配置 SPI2 外设

**文件:**
- 修改: `src/main.rs:64-70`

**Step 1: 配置 SPI2 GPIO 引脚为复用功能**

```rust
// 替换原来的 GPIO 配置
let mut disp_blk = gpiob.pb0.into_push_pull_output();
let disp_dc = gpiob.pb1.into_push_pull_output();
let disp_cs = gpiob.pb12.into_push_pull_output();

// SPI2 引脚配置为 AF5
let disp_sck = gpiob.pb13.into_alternate::<5>();
let disp_miso = gpiob.pb14.into_alternate::<5>();
let disp_mosi = gpiob.pb15.into_alternate::<5>();
```

**Step 2: 在 ccdr 分配后初始化 SPI2**

在 `let ccdr = rcc...freeze(pwrcfg, &dp.SYSCFG);` 后添加：

```rust
// 初始化 SPI2 - Mode 0, 8位数据, 较保守的时钟配置
let spi = dp.SPI2.spi(
    (disp_sck, disp_miso, disp_mosi),
    stm32h7xx_hal::spi::MODE_0,
    4.MHz(),
    ccdr.peripheral.SPI2,
    &ccdr.clocks
);
```

**Step 3: 编译验证**

运行: `cargo build --release`

**Step 4: 提交**

```bash
git add src/main.rs
git commit -m "feat: 添加 SPI2 外设初始化配置"
```

---

## Task 2: 创建新的 display 模块结构 - 硬件 SPI 版本

**文件:**
- 修改: `src/display.rs`

**Step 1: 添加 SPI 相关依赖**

在文件开头添加：

```rust
use stm32h7xx_hal::spi::Spi;
use stm32h7xx_hal::gpio::{Alternate, PushPull};
use stm32h7xx_hal::gpio::gpioa::{PA0, PA1};
use stm32h7xx_hal::gpio::gpiob::{PB0, PB1, PB12, PB13, PB14, PB15};
```

**Step 2: 创建新的 DisplaySpiHardware 结构体**

```rust
pub struct DisplaySpiHardware<SPI, CS, DC> {
    spi: SPI,
    cs: CS,
    dc: DC,
}
```

**Step 3: 实现新的构造函数**

```rust
impl<SPI, CS, DC, E> DisplaySpiHardware<SPI, CS, DC>
where
    SPI: FullDuplex<u8, Error = E>,
    CS: OutputPin<Error = E>,
    DC: OutputPin<Error = E>,
    E: core::fmt::Debug,
{
    pub fn new(spi: SPI, cs: CS, dc: DC) -> Self {
        Self { spi, cs, dc }
    }
}
```

**Step 4: 实现 write_command 方法**

```rust
fn write_command(&mut self, cmd: u8) {
    let _ = self.cs.set_low();
    let _ = self.dc.set_low();
    let _ = block!(self.spi.send(cmd));
    let _ = self.spi.read();
    let _ = self.cs.set_high();
}
```

**Step 5: 实现 write_data 方法**

```rust
fn write_data(&mut self, data: u8) {
    let _ = self.cs.set_low();
    let _ = self.dc.set_high();
    let _ = block!(self.spi.send(data));
    let _ = self.spi.read();
    let _ = self.cs.set_high();
}
```

**Step 6: 编译验证**

运行: `cargo build --release`

**Step 7: 提交**

```bash
git add src/display.rs
git commit -m "feat: 添加硬件 SPI 版本的 Display 结构体"
```

---

## Task 3: 添加串口调试输出

**文件:**
- 修改: `src/main.rs`, `src/display.rs`

**Step 1: 修改 init 方法添加调试输出**

在 display.rs 的 init 方法中，每个关键步骤后添加调试：

```rust
pub fn init(&mut self, tx: &mut impl embedded_hal::serial::Write<u8>) {
    write_str!(tx, "[Display] Start init...\r\n");

    let _ = self.cs.set_high();
    let _ = self.dc.set_high();

    write_str!(tx, "[Display] Software reset...\r\n");
    self.write_command(commands::SWRESET);
    for _ in 0..500000 { cortex_m::asm::nop(); }

    write_str!(tx, "[Display] Exit sleep...\r\n");
    self.write_command(commands::SLPOUT);
    for _ in 0..500000 { cortex_m::asm::nop(); }

    // ... 其余初始化代码
    write_str!(tx, "[Display] Init complete!\r\n");
}
```

**Step 2: 修改 main.rs 传递 tx 到 init**

```rust
// 初始化屏幕
let mut display = DisplaySpiHardware::new(spi, disp_cs, disp_dc);

// 使用可变引用访问 tx
display.init(&mut tx);
```

**Step 3: 编译并烧录测试**

```bash
cargo build --release
make flash
```

**Step 4: 观察串口输出**

串口应该显示初始化进度，如果挂起可以看到在哪一步卡住。

**Step 5: 提交**

```bash
git add src/main.rs src/display.rs
git commit -m "feat: 添加串口调试输出"
```

---

## Task 4: 实现 DrawTarget trait

**文件:**
- 修改: `src/display.rs`

**Step 1: 实现 DrawTarget for DisplaySpiHardware**

```rust
impl<SPI, CS, DC, E> DrawTarget for DisplaySpiHardware<SPI, CS, DC>
where
    SPI: FullDuplex<u8, Error = E>,
    CS: OutputPin<Error = E>,
    DC: OutputPin<Error = E>,
    E: core::fmt::Debug,
{
    type Color = Rgb565;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels.into_iter() {
            let x = point.x as u16;
            let y = point.y as u16;

            if x < DISPLAY_WIDTH as u16 && y < DISPLAY_HEIGHT as u16 {
                self.set_address_window(x, y, x, y);

                let pixel_color = color.into_storage();
                self.write_command(commands::RAMWR);
                self.write_data((pixel_color >> 8) as u8);
                self.write_data((pixel_color & 0xFF) as u8);
            }
        }
        Ok(())
    }

    // ... 实现 fill_contiguous, fill_solid, clear 方法
}
```

**Step 2: 实现 OriginDimensions trait**

**Step 3: 编译测试**

**Step 4: 提交**

```bash
git add src/display.rs
git commit -m "feat: 实现 DrawTarget trait"
```

---

## Task 5: 添加 DMA 支持（可选，性能优化）

**文件:**
- 修改: `src/main.rs`, `src/display.rs`

**Step 1: 配置 DMA2**

在 main.rs 中：

```rust
let dma = dp.DMA2;
let mut dma_stream = dma.stream1();

// 配置 DMA 为 SPI2 TX
// 具体配置参考 stm32h7xx-hal 文档
```

**Step 2: 实现 DMA 传输的 fill_rect**

```rust
pub fn fill_rect_dma(&mut self, x: u16, y: u16, w: u16, h: u16, color: Rgb565) {
    // 设置地址窗口
    self.set_address_window(x, y, x + w - 1, y + h - 1);

    let pixel_color = color.into_storage();
    let buffer = [pixel_color; 1024]; // 使用缓冲区

    // 使用 DMA 传输
    // ...
}
```

**Step 3: 提交**

```bash
git add src/main.rs src/display.rs
git commit -m "feat: 添加 DMA 传输支持"
```

---

## Task 6: 清理和优化

**文件:**
- 修改: `src/display.rs`

**Step 1: 删除旧的软件 SPI 代码**

移除 `DisplaySpi` 软件实现，保留 `DisplaySpiHardware`

**Step 2: 重命名类型别名**

```rust
pub type DisplaySpi<SPI, CS, DC> = DisplaySpiHardware<SPI, CS, DC>;
```

**Step 3: 更新 CLAUDE.md 记录**

添加硬件 SPI 配置成功的记录

**Step 4: 最终测试和提交**

```bash
cargo build --release
make flash
git add src/display.rs CLAUDE.md
git commit -m "refactor: 完成硬件 SPI 迁移，移除软件 SPI"
```

---

## 测试检查清单

- [ ] 编译无错误
- [ ] LED 快闪 10 次
- [ ] 串口输出初始化进度
- [ ] 屏幕显示红色方块
- [ ] 屏幕显示蓝色方块
- [ ] 屏幕显示绿色方块
- [ ] LED 慢闪表示运行中

## 故障排查

| 问题 | 解决方案 |
|------|----------|
| SPI 挂起 | 降低 SPI 时钟到 1MHz |
| 无显示 | 检查 CS/DC 引脚配置 |
| 花屏 | 检查 SPI 模式和字节序 |
| DMA 不工作 | 先用基本 SPI 验证 |
