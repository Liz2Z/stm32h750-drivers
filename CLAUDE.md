# 项目踩坑记录

### 屏幕连接配置

| STM32H750 | ILI9341 | 说明      |
| --------- | ------- | --------- |
| PB15/MOSI | SDI     | 数据输入  |
| PB13/SCK  | SCL     | 时钟      |
| PB12/CS   | CS      | 片选      |
| PB14/MISO | SDO     | 数据输出  |
| PB1/RS    | D/C     | 数据/命令 |
| PB0/BLK   | BLK     | 背光控制  |

**注意**: 复位引脚(RST)直接接 VCC，使用软件复位初始化

### DMA 传输踩坑记录

#### 问题 1: DMA 无法访问栈内存

- **问题**: STM32H7 的 DMA 无法访问 DTCM (0x20000000)，只能访问 AXISRAM (0x24000000)
- **错误**: DMA 传输失败或数据损坏
- **解决方案**: 使用 `#[link_section = ".axisram.buffers"]` 将 DMA 缓冲区放在 AXISRAM

#### 问题 2: DMA 缓冲区必须是静态生命周期

- **问题**: DMA 传输可能在函数返回后仍在进行，栈缓冲区会被销毁
- **错误**: 内存崩溃 (HardFault)
- **解决方案**: 使用 `static mut` 定义全局 DMA 缓冲区

#### 问题 3: DMA 配置复杂

- **问题**: `stm32h7xx-hal` 的 DMA API 需要正确的类型参数和流配置
- **参考**: 官方示例 `[spi-dma.rs](https://github.com/stm32-rs/stm32h7xx-hal/blob/master/examples/spi-dma.rs)` 展示了正确的 DMA 配置方法

#### 当前优化方案

- **缓冲区**: 8KB AXISRAM 双缓冲区 (`#[link_section = ".axisram.buffers"]`)
- **传输方式**: 批量 SPI 传输 + DMA 缓冲区优化
- **性能**: 全屏刷新从 ~50ms 降至 ~10ms

#### 完整 DMA 驱动实现 (2026-03-01)

- 使用 `MaybeUninit` 初始化 DMA 缓冲区
- 实现了 `fill_rect` 使用 DMA 缓冲区批量传输
- 实现了 `clear` 使用 DMA 缓冲区清屏
- `DrawTarget` 实现使用 DMA 缓冲区进行像素批量传输
- 使用 `HalSpi::inner()` 检查 SPI 状态寄存器
- 使用 `txp` (TX FIFO 就绪) 和 `txc` (传输完成) 位进行流控

#### DMA API 正确用法

```rust
// 1. 需要导入 HalSpi trait
use stm32h7xx_hal::spi::HalSpi;

// 2. 检查 SPI TX FIFO 是否就绪
while spi.inner().sr.read().txp().bit_is_clear() {}

// 3. 检查 SPI 传输是否完成
while spi.inner().sr.read().txc().bit_is_clear() {}

// 4. 缓冲区必须使用 MaybeUninit 在 AXISRAM 中
#[link_section = ".axisram.buffers"]
static mut DMA_BUF0: MaybeUninit<[u8; DMA_BUF_SIZE]> = MaybeUninit::uninit();
```

### UI DMA 优化 (2026-03-01)

#### 优化策略
1. **使用 `fill_solid` 替代多个 `Rectangle` 绘制**
   - 按钮、进度条等控件使用 `fill_solid` 进行大面积填充
   - 减少 DMA 传输次数，提高批量传输效率

2. **脏矩形跟踪**
   - 添加 `BoundingBox` 结构表示控件边界
   - `Screen` 维护脏矩形列表，支持局部重绘
   - `ProgressBar` 记录 `last_fill_width` 检测变化

3. **DMA 友好的绘制方法**
   - `Screen::draw_with_dma()` - 全屏 DMA 绘制
   - `Screen::update_progress_bar_with_dma()` - 局部 DMA 更新
   - `ProgressBar::draw_incremental()` - 增量绘制

#### 关键数据结构
```rust
/// 边界框用于脏矩形检测
pub struct BoundingBox {
    pub x: i32, pub y: i32,
    pub width: u32, pub height: u32,
}

/// 屏幕维护脏矩形列表
pub struct Screen {
    pub widgets: heapless::Vec<Widget, 8>,
    pub dirty_rects: heapless::Vec<BoundingBox, 8>,
    // ...
}
```

#### 性能优化效果
- 按钮绘制：使用 `fill_solid` 替代 5 个独立 `Rectangle` 绘制
- 进度条更新：支持局部重绘，避免全屏刷新
- 全屏绘制：~10ms（使用 DMA 批量传输)

·
