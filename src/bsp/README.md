# BSP：这块板子的硬件适配

BSP 是 **Board Support Package（板级支持包）**。本目录绑定当前 STM32H750 板子的
主控、时钟、接线和内存布局，负责把实际硬件装配成应用能够使用的资源。

例如，“屏幕接在 SPI2，CS 用 PB12，像素由 DMA1 发送”属于 BSP；
“ILI9341 用什么命令设置显示窗口”属于 [drivers](../drivers/README.md)。

## 文件职责

| 文件 | 负责什么 |
|---|---|
| [board.rs](board.rs) | 获取外设所有权，配置电源、400MHz 时钟和 GPIO，装配显示、I2C、串口、LED |
| [clock.rs](clock.rs) | 基于 TIM2 的微秒计数、期限判断和延时 |
| [bounded_i2c.rs](bounded_i2c.rs) | 为具体的 I2C1 控制器实现有超时、可恢复的 embedded-hal 总线接口 |
| [adc_ntc.rs](adc_ntc.rs) | 预留的 PA3 模拟引脚封装；当前尚未实现 ADC 采集和温度换算 |
| [profiler.rs](profiler.rs) | `profiler` 特性下的 Cortex-M DWT 性能计时 |
| [display/mod.rs](display/mod.rs) | 实板显示类型别名和 `new_display` 装配入口 |
| [display/spi2.rs](display/spi2.rs) | SPI2、CS/DC 控制、8/16 位切换，实现通用驱动的 `DisplayInterface` |
| [display/dma.rs](display/dma.rs) | DMA1 stream 0 像素发送、完成等待、超时停流和恢复 |
| [display/framebuffer.rs](display/framebuffer.rs) | AXI SRAM 帧缓冲和传输缓冲的存储、初始化与一次性领取 |
| [display/bench.rs](display/bench.rs) | `display-bench` 特性下的实板显示测速与调试结果快照 |

## BSP 与 drivers 的边界

这里的“板级”也包括 STM32 **片内外设**的具体适配，如 SPI2、I2C1、TIM2、DMA1。
`drivers/` 主要描述所连接器件的协议，此外也包含基于 trait 的通用传输辅助代码。
不能只看到“驱动”两个字，就把所有寄存器操作都放进 `drivers/`。

| 要改的内容 | 修改位置 |
|---|---|
| 屏幕 CS 从 PB12 换到其他引脚 | `board.rs` 和 `display/` 中对应的具体引脚类型 |
| SPI2 改成其他 SPI 控制器 | `board.rs` 和 `display/` 的外设适配 |
| ILI9341 初始化命令、显示方向寄存器 | `drivers/display/ili9341.rs` |
| AHT20 测量命令和数据换算 | `drivers/aht20.rs` |
| 传感器多久采样一次 | `application/runtime.rs` |
| 卡片颜色、布局、底栏内容 | `ui/` |

## 应用如何拿到硬件

启动关系如下：

```text
main.rs
  → bsp::board::with_board(application::run, 串口波特率)
      → 配置时钟、引脚与外设，构造 Board
      → application::run(board, &clock)
```

`Board` 将显示、I2C、串口和 LED 的所有权交给应用。时钟留在 `with_board` 的栈帧中，
显示与 I2C 借用它；应用循环持续运行，因此这些借用一直有效，不需要全局可变时钟。

显示装配时，BSP 把 `Spi2Display` 和两个独占缓冲交给通用
`drivers::display::DisplayDriver<T>`。BSP 提供的 `DisplayDriver<'a>` 是这个泛型类型
在当前板子上的类型别名，并非另一套显示协议实现。控制器初始化由应用调用 `init` 完成。

## 帧缓冲为什么在这里

帧缓冲是内存中的 RGB565 画布，显示驱动负责读写其中的像素；但它放在哪块 RAM、
DMA 能否访问、如何初始化，取决于主控，所以存储放在 BSP。

当前帧缓冲为 153,600 字节，窄窗口传输缓冲为 8,192 字节，放在
[memory.x](../../memory.x) 定义的 AXI SRAM NOLOAD 区域。领取前先初始化存储，
随后将独占引用移交给显示驱动；一次性领取守卫位于 BSS。

当前显示独占 DMA1，开启 I-Cache、保持 D-Cache 关闭。修改缓存策略、共享 DMA1 或
换主控时，需要同时检查 DMA 的可访问内存、缓冲生命周期、停流和缓存维护。

## 换板子时怎么改

保留相同传感器和屏幕控制器时，优先复用 `drivers/`，重新实现 BSP 的时钟、接线、
总线和显示传输接口。还需核对 Cargo 目标与 [memory.x](../../memory.x)，不能仅替换引脚。
本模块目前仅在 ARM 目标下编译；其中使用的具体 HAL 面向 STM32H750。

从项目根目录运行 `make build`、`make check-features` 和 `make examples` 检查构建，
用 `make flash` 烧录、`make debug` 调试，沿用 OpenOCD/GDB。
显示测速使用 `make bench-flash` 和 `make bench-read`，完成后用 `make flash` 恢复普通固件。

完整说明见 [项目架构](../../docs/ARCHITECTURE.md) 与
[显示、采集踩坑记录](../../docs/troubleshootings/DISPLAY_AND_SENSOR_RELIABILITY.md)。
