# Drivers：器件协议与通用驱动

本目录描述“某个器件应该怎么操作”：向 AHT20 发什么测量命令、如何解码 BMP280
的寄存器、ILI9341 如何初始化和刷新画面。驱动通过 trait 使用总线或引脚，
不直接依赖 STM32 的具体 HAL 类型。

例如，AHT20 接到哪组 I2C 引脚由 [BSP](../bsp/README.md) 决定；
AHT20 的设备地址、命令和读数换算由这里的驱动决定。

## 文件职责

| 文件 | 器件或功能 | 当前接口与用途 |
|---|---|---|
| [aht20.rs](aht20.rs) | AHT20 温湿度传感器 | embedded-hal I2C trait；初始化、测量、状态检查和数据换算 |
| [bmp280.rs](bmp280.rs) | BMP280 气压传感器 | embedded-hal I2C trait；芯片识别、校准参数和补偿计算 |
| [dht11.rs](dht11.rs) | DHT11 温湿度传感器 | 泛型输入/输出引脚；预留驱动，普通固件未使用 |
| [esp8266.rs](esp8266.rs) | ESP8266 串口协处理器 | 泛型串口收发；普通固件默认关闭其通信流程 |
| [serial.rs](serial.rs) | 通用串口输出辅助 | embedded-hal 串口 trait；字符串和数字输出，不配置 USART 引脚 |
| [display/mod.rs](display/mod.rs) | 显示公共接口 | `DisplayInterface`、泛型 `DisplayDriver<T>`、方向与错误类型 |
| [display/ili9341.rs](display/ili9341.rs) | ILI9341 控制器 | 初始化命令、方向、RGB565 绘图、整屏与局部刷新 |
| [display/pixel_pack.rs](display/pixel_pack.rs) | 像素布局辅助 | 窗口裁剪、窄区域连续像素打包 |
| [display/spi_transfer.rs](display/spi_transfer.rs) | 命令发送辅助 | 基于 embedded-hal 的有超时 8 位发送 |
| [display/error.rs](display/error.rs) | 显示传输错误 | 超时、SPI 错误、DMA 错误、无效缓冲 |
| [display/tests.rs](display/tests.rs) | 显示驱动测试 | 用模拟传输接口验证命令、像素顺序、分块和失败清理 |

`serial.rs` 和像素打包等文件属于通用辅助功能，并非每个文件都对应一颗芯片。
目前 DHT11、ESP8266、串口模块仍通过 ARM 条件编译导出；这与是否使用具体 HAL
是两件事。AHT20、BMP280 和通用显示驱动可在宿主机上编译、测试。

## 驱动与应用的边界

| 问题 | 负责的模块 |
|---|---|
| AHT20 如何触发测量、读数是否有效 | `drivers/aht20.rs` |
| BMP280 如何使用校准参数换算气压 | `drivers/bmp280.rs` |
| ILI9341 用什么命令设置窗口 | `drivers/display/ili9341.rs` |
| I2C1 超时后如何恢复控制器 | `bsp/bounded_i2c.rs` |
| SPI2 如何通过 DMA1 发送像素 | `bsp/display/` |
| 两个传感器如何顺序采集、失败后何时重试 | `application/acquisition/mod.rs` |
| 每隔多久采样、刷新动画 | `application/runtime.rs` 与 `application/scheduling.rs` |
| 保留上次读数、标记 Fresh/Stale/Error | 采集组件私有的 `application/acquisition/record.rs` |
| 显示单位、颜色、控件和状态文字 | `ui/` |

## 传感器如何共用总线

AHT20 和 BMP280 驱动在每次操作时借用总线，不各自持有 I2C1 的原始引用。
`application::acquisition::Acquisition` 在同一调度线程中顺序调用两者，并管理各自的
初始化和重试状态；BSP 的 `BoundedI2c` 提供实际总线访问及事务超时。

因此，换成另一种实现了对应 embedded-hal trait 的 I2C 总线时，可以复用器件协议。
多任务并发访问时则需要额外的总线仲裁，当前顺序借用本身不提供并发仲裁。

## 显示驱动如何接到实板

```text
UI 绘制 RGB565 像素、提交刷新区域
  → drivers::display::DisplayDriver<T>
      组织 ILI9341 命令、裁剪窗口、读取或打包帧缓冲
      → DisplayInterface
          → bsp::display::Spi2Display
              操作 SPI2、CS/DC 和 DMA1
```

`DisplayInterface` 是本项目的传输接口，把控制器协议与具体硬件传输分开。
宿主机测试用模拟实现，实板用 BSP 实现。像素会话开始成功后，无论发送成功或失败，
驱动都调用 `finish_pixels`；传输实现必须在返回前停止读取借用的像素，完成必要的清理。

帧缓冲的存储和一次性领取属于 BSP；这里负责在缓冲中绘图、选择刷新窗口和像素顺序。
UI 的 `FlushTarget` 适配实现在 `ui/screen.rs`，驱动本身不依赖 UI。

## 换硬件时怎么复用

- **换主控、器件不变**：保留协议驱动，替换 BSP 和传输适配，并核对时序要求。
- **换接线、器件不变**：修改 BSP 的引脚配置及相应类型，通常无需修改器件命令。
- **同一块板子换成另一种屏幕控制器**：实现新控制器驱动，核对它的初始化、尺寸、
  像素格式和传输要求，按需调整 BSP。
- **新增传感器**：增加对应器件驱动；接入采集组件，由组件维护读数状态，主循环调度，UI 显示。

DHT11 当前仍包含按自旋次数估计时序的实现，移植到其他主控或频率时需要重新核验时序；
使用泛型引脚不意味着时间精度自动可移植。

## 验证入口

在项目根目录运行 `make test`，验证宿主机上的模拟总线、器件协议、显示及 UI 回归。
`make check-host` 检查宿主机库，`make check` 检查 ARM 固件，`make check-features` 检查全部特性，
`make examples` 构建外设示例。真实总线时序与屏幕显示还需上板验证。

完整说明见 [项目架构](../../docs/ARCHITECTURE.md) 与
[BSP 说明](../bsp/README.md)。
