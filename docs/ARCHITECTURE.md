# STM32H750 Display 项目架构

本项目运行在 STM32H750VB 上，通过共享 I2C1 采集 AHT20 温湿度与 BMP280 气压，
使用 SPI2 驱动 ILI9341 横屏显示。普通固件每 5 秒采样，顶部动画约 60Hz，ESP8266 默认关闭。

## 分层与依赖

按采集组件、应用调度、界面、通用驱动、板级适配划分职责，在一个 no_std crate 内实现。

| 层 | 位置 | 职责 |
|---|---|---|
| 采集组件 | `src/application/acquisition/` | 拥有驱动、内部记录和采样计数，一次采样完成读取与记录更新，提供结果快照 |
| 应用编排 | `src/application/` | 周期调度、连接状态、采集快照到控件输入的转换 |
| 界面 | `src/ui/` | 声明控件输入，布局、格式化、绘图、动画和脏区域刷新 |
| 通用驱动 | `src/drivers/` | 设备协议、绘图和传输 trait，不依赖具体 HAL 或 UI |
| 板级适配 | `src/bsp/` | STM32H750 时钟、引脚、I2C 期限、SPI2/DMA1、AXI SRAM |
| 装配入口 | `src/main.rs` | 调用 BSP 装配资源，再启动应用循环 |

`Acquisition` 持有两个驱动和三个私有 `Record`，对外只提供采集操作、本轮错误和只读结果快照。
`acquisition/record.rs` 只依赖 core，模块私有，记录类型仅对父模块可见；运行入口不能
直接写入读数、标记失败或独立更新采样计数。一次 `sample` 完成这些操作。
`Record` 同时维护当前值、累计极值、最近六次历史和成功时间戳。

UI 在控件中声明 `TempHumidCardData`、`PressureCardData` 和底栏输入，不导入采集快照
或应用状态。`application/presentation.rs` 从快照提取数值范围与状态，再构造控件输入。
采集组件不依赖 UI；转换模块同时依赖采集接口和 UI 输入。连接与上传状态归应用层，
底栏只接收文字、颜色提示和计数。根目录没有独立的 `domain/` 或 `sampling.rs`。
详见 [采集组件说明](../src/application/acquisition/README.md)。

`ui::FlushTarget` 定义绘图后的刷新接口。实板 `DisplayDriver` 实现该接口，
宿主机测试使用模拟画面验证布局、脏区域和失败后重试。硬件依赖通过 ARM 条件编译隔离。
显示的 SPI2/DMA1 适配位于 BSP，实现通用驱动的 `DisplayInterface`。
UI 中实现 `FlushTarget` 适配，通用驱动不反向依赖 UI。

`main.rs` 仅 11 行；`bsp::board::with_board` 在栈上持有时钟，把借用时钟的资源和
时钟引用交给 `application::run`。不用全局可变时钟，也不构造自引用结构。
应用循环只使用 BSP 暴露的资源，不直接引用具体 HAL 类型。

`lib.rs` 是唯一模块入口；固件、示例与测试引用同一份实现。禁止再通过 `#[path]`
把同一个文件挂到多个模块路径，避免重复类型和不清晰的依赖方向。

## 显示模块

```
src/drivers/display/         # 与 MCU 无关
├── mod.rs                  # DisplayInterface 传输端口
├── ili9341.rs              # 初始化、方向、DrawTarget、整屏/局部刷新
├── pixel_pack.rs           # 窗口裁剪与原生像素打包
├── spi_transfer.rs         # 基于 embedded-hal 的有超时命令发送
├── error.rs                # Timeout / Spi / Dma / InvalidBuffer
└── tests.rs                # 协议、裁剪、分块、超时和失败清理测试
src/bsp/display/             # 当前 STM32H750 板子
├── mod.rs                  # 实板类型别名与构造入口
├── spi2.rs                 # SPI2/CS/DC、8/16 位模式和线路完成
├── dma.rs                  # DMA1 stream 0、停流与异常恢复
├── framebuffer.rs          # AXI SRAM 存储与一次性领取
└── bench.rs                # 可选的一次性实板测速
```

`drivers::display::DisplayDriver<T>` 只依赖 `DisplayInterface`，以传输 trait 隔离
命令/像素阶段和错误清理；宿主机测试可直接实例化真正的显示驱动。实板通过
`bsp::display::new_display` 移交 SPI2、CS/DC、DMA1 与其 RCC token，借用时钟。
应用使用 `bsp::display::DisplayDriver` 类型别名，DMA 和缓冲领取不对应用层公开。

### 绘制与传输

1. embedded-graphics 在 RGB565 帧缓冲中绘图。
2. UI 根据变化生成刷新窗口；空窗口直接返回，越界窗口裁剪。
3. 8 位 SPI 命令设置 ILI9341 窗口，发送 RAMWR。
4. 切换 SPI2 为 16 位 MSB-first，通过 DMA 发送像素。
5. DMA TC 与 SPI TXC 都完成后，恢复 8 位模式并释放 CS。

整行区域直接读取帧缓冲；超过 65,535 像素时分段发送。
窄区域逐行复制到 4,096 个 u16 的传输缓冲，跳过窗口外像素。
原生 u16 由 SPI MSB-first 输出正确线序，不再保留旧的大端字节打包发送实现。

### 内存和所有权

- RGB565 帧缓冲：76,800 个 u16，共 153,600 字节。
- 窄窗口传输缓冲：8,192 字节。
- 两者均位于 `.axisram.buffers`，链接到 AXI SRAM 的 NOLOAD 区域。
- 存储使用 `UnsafeCell<MaybeUninit<_>>`，先初始化再创建独占引用，不使用裸 `static mut`。
- BSS 中的 AtomicBool 保证只能领取一次；不能把这个守卫放进未初始化的 NOLOAD 区域。
- 显示独占整个 DMA1，使用 stream 0 / DMAMUX request 40。
- DMA 函数保持源数据不可变借用，停流与内存屏障完成后才返回。
- 开启 I-Cache，保持 D-Cache 关闭；开启 D-Cache 或共享 DMA1 前必须修改驱动。

### 错误恢复

命令/像素发送等待有 100ms 上界；SPI 挂起与 DMA 停流等待各有 1ms 上界。
DMA 停流失败时复位 DMA1。所有事务退出均释放 CS，错误后恢复命令字长并清标志。
主循环重试初始化和整屏重绘，不把失败的传输记入刷新率。
通用驱动开始像素会话后，成功与失败都调用 `finish_pixels`；BSP 返回前停止 DMA
读取源缓冲。开始会话失败时由 BSP 自行释放 CS。

DMA FIFO 暂时饥饿与实际传输错误分开处理，细节及实板故障注入结果见
[显示与采集踩坑记录](troubleshootings/DISPLAY_AND_SENSOR_RELIABILITY.md)。

## 采集与调度

- `application::acquisition::Acquisition` 独占两个驱动、初始化/重试状态、记录及计数，顺序借用同一总线。
- AHT20 失败不阻止 BMP280，反之亦然；下次周期分别重试。
- `bsp::bounded_i2c` 为实板 I2C 事务提供期限和恢复。
- 采集组件在失败时保留有效读数并标为 Stale，没有有效读数时标为 Error；外部不能单独改状态。
- `application::scheduling::Periodic` 使用回绕差值，错过期限时跳帧，不补执行积压任务。
- `RefreshRate` 按已完成的 10 秒窗口统计成功内容更新，不计底栏自身。
- `ui::FlowAnimation` 按实际经过时间计算相位，独立于 5 秒采样周期。

## 测试边界

单元测试归对应模块：记录、格式化、调度、显示转换、AHT20、BMP280 的测试内联在
源码的 `tests` 子模块；已有实现目录的显示驱动使用 `drivers/display/tests.rs`。
单文件模块无需为了测试另建同名目录。
根目录 `tests/acquisition.rs` 和 `tests/ui_rendering.rs` 通过库的公开 API 验证跨模块
流程与画面回归，模拟屏幕共用 `tests/common/mod.rs`。根目录源码不再集中挂载回归测试。

固件可执行文件要求默认启用的 `firmware` 特性；宿主机测试关闭默认特性，跳过裸机入口，
再通过 `--lib --tests` 同时运行单元与集成测试。该特性不改变库的运行逻辑。
CI 的宿主机测试作业复用 Makefile，说明见 [测试文档](../tests/README.md)。

## 验证入口

| 命令 | 用途 |
|---|---|
| `make test` | 宿主机模块单元测试与根目录 `tests/` 集成测试 |
| `make check` / `make check-host` | rustfmt、ARM 固件 / 宿主机库与全部测试 Clippy |
| `make check-bench` / `make check-features` | 测速配置 / 全特性（含 profiler）Clippy |
| `make examples` | 外设示例构建 |
| `make flash` | 普通固件构建、烧录与校验 |
| `make bench-flash` | 独立目录构建并烧录测速固件 |
| `make debug` / `make bench-read` | 在测速完成后读取结果 |

烧录与调试沿用 OpenOCD/GDB，不强制使用 probe-rs。
测速变量用 UnsafeCell 包装，GDB 读取路径为
`stm32h750_drivers::bsp::display::bench::DISPLAY_BENCH_RESULTS.0.value`。
测速期间不要暂停 MCU，结果 status 必须为 2；测试后用 `make flash` 恢复普通固件。

重构前实板 50MHz SCK 下整屏传输约 24.58ms，接近纯像素线速下限 24.576ms；
这是传输吞吐，不包含视频解码，也不是面板扫描帧率。历次结果与故障验证保存在踩坑记录中。
