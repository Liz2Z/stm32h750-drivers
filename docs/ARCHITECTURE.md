# STM32H750 Display 项目架构文档

## 项目概述

本项目是一个基于 STM32H750VB 微控制器的嵌入式显示系统，使用 Rust 语言开发。系统通过 ILI9341 驱动的 TFT LCD 屏幕显示温湿度数据，集成了 DHT11 温湿度传感器，并实现了完整的 UI 框架。

## 系统架构

```
┌─────────────────────────────────────────────────────────────┐
│                        应用层 (main.rs)                      │
│  - 系统初始化                                                 │
│  - 传感器数据采集                                             │
│  - UI 更新逻辑                                                │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                        UI 框架层 (ui/)                       │
│  ┌─────────────┐  ┌──────────────┐  ┌──────────────────┐   │
│  │   Screen    │  │   Widgets    │  │     Theme        │   │
│  │  (容器管理)  │  │ (控件实现)    │  │   (主题配置)     │   │
│  └─────────────┘  └──────────────┘  └──────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                      显示驱动层 (display.rs)                 │
│  - ILI9341 控制器驱动                                        │
│  - SPI 通信接口                                              │
│  - 帧缓冲管理                                                 │
│  - DMA 传输优化                                              │
│  - embedded-graphics DrawTarget 实现                         │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                      硬件抽象层 (HAL)                         │
│  - stm32h7xx-hal (硬件抽象层)                                │
│  - embedded-hal (通用嵌入式接口)                              │
│  - cortex-m (ARM Cortex-M 支持)                              │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                        硬件层                                 │
│  - STM32H750VB 微控制器                                      │
│  - ILI9341 TFT LCD 控制器 (240x320)                          │
│  - DHT11 温湿度传感器                                        │
│  - NTC 热敏电阻 (ADC)                                        │
└─────────────────────────────────────────────────────────────┘
```

## 目录结构

```
stm32h750-display/
├── src/
│   ├── main.rs              # 主程序入口
│   ├── display.rs           # ILI9341 屏幕驱动 (DMA 优化)
│   ├── dht11.rs             # DHT11 温湿度传感器驱动
│   ├── adc_ntc.rs           # NTC 热敏电阻 ADC 驱动
│   ├── serial.rs            # 串口通信模块
│   ├── profiler.rs          # 性能分析工具
│   └── ui/                  # UI 框架模块
│       ├── mod.rs           # UI 模块入口
│       ├── screen.rs        # 屏幕/容器管理
│       ├── theme.rs         # 主题配置
│       ├── sensor.rs        # 传感器数据封装
│       ├── icons.rs         # 图标定义
│       ├── bounding_box.rs  # 边界框工具
│       └── widgets/         # UI 控件
│           ├── mod.rs       # 控件枚举定义
│           ├── button.rs    # 按钮控件
│           ├── label.rs     # 文本标签
│           ├── progress.rs  # 进度条
│           ├── card.rs      # 温湿度卡片
│           └── history.rs   # 历史记录条
├── docs/                    # 文档目录
│   ├── learning/            # 学习笔记
│   └── ARCHITECTURE.md      # 本架构文档
├── examples/                # 示例代码
│   ├── adc.rs              # ADC 示例
│   └── temperature.rs       # 温度读取示例
├── memory.x                 # 链接器脚本
├── build.rs                 # 构建脚本
├── Cargo.toml               # Rust 项目配置
└── Makefile                 # 构建和烧录脚本
```

## 核心模块详解

### 1. 主程序 (main.rs)

**职责**：
- 系统时钟和电源配置
- GPIO 和外设初始化
- 主循环逻辑
- 传感器数据采集和 UI 更新

**关键流程**：
```rust
系统初始化 → 屏幕初始化 → UI 创建 → 循环 {
    读取传感器数据 → 更新 UI → DMA 刷新屏幕
}
```

### 2. 显示驱动层 (display.rs)

**职责**：
- ILI9341 控制器初始化
- SPI 通信管理
- 帧缓冲区管理 (150KB, AXISRAM)
- DMA 传输缓冲区 (8KB, AXISRAM)
- embedded-graphics DrawTarget trait 实现

**核心特性**：
- **帧缓冲机制**：完整帧缓冲 (240x320x2 = 150KB)
- **DMA 优化**：使用 DMA1 Stream3 进行 SPI2 数据传输
- **脏矩形优化**：只刷新变化的区域
- **双缓冲支持**：8KB DMA 传输缓冲区

**关键接口**：
```rust
pub struct DisplayDriver {
    spi: Option<SpiDma>,
    cs: PB12<Output<PushPull>>,
    dc: PB1<Output<PushPull>>,
}

impl DisplayDriver {
    pub fn init(&mut self, delay_ms: &mut impl FnMut(u32));
    pub fn flush(&mut self);                    // 全屏刷新
    pub fn flush_rect(&mut self, x, y, w, h);   // 区域刷新
    pub fn clear(&mut self, color: Rgb565);     // 清屏
}
```

### 3. UI 框架层 (ui/)

#### 3.1 Screen (screen.rs)

**职责**：
- 控件容器管理
- 脏矩形跟踪
- 绘制调度

**核心结构**：
```rust
pub struct Screen {
    pub widgets: heapless::Vec<Widget, 8>,      // 最多 8 个控件
    pub theme: GrayTheme,                       // 主题配置
    pub dirty_rects: heapless::Vec<BoundingBox, 8>, // 脏矩形列表
}
```

#### 3.2 Widgets (ui/widgets/)

**支持的控件**：
- **Button**：按钮控件
- **Label**：文本标签
- **ProgressBar**：进度条
- **TempHumidCard**：温湿度卡片
- **HistoryBar**：历史记录条形图

**控件接口**：
```rust
pub trait Widget {
    fn draw<D: DrawTarget<Color = Rgb565>>(&self, display: &mut D) -> Result<(), D::Error>;
    fn bounding_box(&self) -> BoundingBox;
}
```

#### 3.3 Theme (theme.rs)

**职责**：
- 统一的颜色管理
- 8 级灰度主题

**颜色定义**：
```rust
pub struct GrayTheme {
    g0: Rgb565,  // 黑色 - 线条、文字
    g1: Rgb565,  // 最深灰 - 按下按钮
    g2: Rgb565,  // 深灰 - 进度条填充
    g3: Rgb565,  // 中深灰 - 边框阴影
    g4: Rgb565,  // 中浅灰 - 普通按钮
    g5: Rgb565,  // 浅灰 - 禁用状态
    g6: Rgb565,  // 最浅灰 - 辅助背景
    g7: Rgb565,  // 白色 - 主背景
}
```

### 4. 传感器驱动层

#### 4.1 DHT11 (dht11.rs)

**职责**：
- DHT11 单总线协议实现
- 温湿度数据读取

**通信协议**：
- 启动信号：拉低 18ms + 拉高 20-40μs
- 响应信号：低 80μs + 高 80μs
- 数据格式：40位 (湿度整数 + 湿度小数 + 温度整数 + 温度小数 + 校验和)

#### 4.2 NTC 热敏电阻 (adc_ntc.rs)

**职责**：
- ADC 初始化
- NTC 温度转换 (查表法)

### 5. 工具模块

#### 5.1 Profiler (profiler.rs)

**职责**：
- 性能分析
- 时间测量

#### 5.2 Serial (serial.rs)

**职责**：
- 串口通信
- 调试输出

## 内存布局

```
STM32H750VB 内存映射：
┌──────────────────┬──────────────┬─────────────┐
│ 区域             │ 地址范围      │ 大小        │
├──────────────────┼──────────────┼─────────────┤
│ FLASH            │ 0x08000000   │ 128KB       │
│ RAM              │ 0x20000000   │ 128KB       │
│ AXISRAM          │ 0x24000000   │ 512KB       │
└──────────────────┴──────────────┴─────────────┘

内存分配：
- FLASH: 代码段 (.text)、只读数据 (.rodata)
- RAM: 栈、堆、数据段 (.data)、BSS 段 (.bss)
- AXISRAM: 
  - 帧缓冲区 (150KB): FRAME_BUFFER
  - DMA 缓冲区 (8KB): DMA_BUF
```

## 硬件连接

### ILI9341 屏幕 (SPI2)

| STM32H750 | ILI9341 | 说明      |
| --------- | ------- | --------- |
| PB15/MOSI | SDI     | 数据输入  |
| PB13/SCK  | SCL     | 时钟      |
| PB12/CS   | CS      | 片选      |
| PB14/MISO | SDO     | 数据输出  |
| PB1/RS    | D/C     | 数据/命令 |
| PB0/BLK   | BLK     | 背光控制  |

**注意**: 复位引脚(RST)直接接 VCC，使用软件复位初始化

### DHT11 温湿度传感器

| STM32H750 | DHT11 | 说明       |
| --------- | ----- | ---------- |
| PA2       | DATA  | 数据线     |

**注意**: DATA 引脚需要 5K 上拉电阻

### 其他外设

| STM32H750 | 外设   | 说明           |
| --------- | ------ | -------------- |
| PA1       | LED    | 状态指示灯     |
| PA3       | NTC    | ADC 输入       |

## 性能优化

### 1. DMA 传输优化

- 使用 DMA1 Stream3 进行 SPI2 数据传输
- CPU 零开销，传输期间可执行其他任务
- 8KB 双缓冲区支持乒乓传输

### 2. 帧缓冲优化

- 完整帧缓冲避免频繁 SPI 通信
- 绘图操作直接写入内存
- 批量传输提高效率

### 3. 脏矩形优化

- 跟踪变化的区域
- 只刷新需要更新的部分
- 减少不必要的数据传输

### 4. SPI 高速配置

- SPI 时钟：80MHz
- GPIO 速度：VeryHigh
- 传输模式：Mode 3

## 依赖库

| 库名称            | 版本  | 用途                        |
| ----------------- | ----- | --------------------------- |
| stm32h7xx-hal     | 0.16  | STM32H7 硬件抽象层          |
| cortex-m          | 0.7   | ARM Cortex-M 支持           |
| cortex-m-rt       | 0.7   | Cortex-M 运行时             |
| embedded-graphics | 0.7   | 2D 图形库                   |
| embedded-hal      | 0.2   | 嵌入式硬件抽象接口          |
| heapless          | 0.7   | 无堆数据结构                |
| micromath         | 1.1   | 数学函数库                  |
| nb                | 1.0   | 非阻塞 I/O 工具             |
| panic-halt        | 1.0   | panic 处理                  |

## 构建系统

### Makefile 命令

```bash
make all              # 编译项目 (release)
make bin              # 生成二进制文件
make flash-openocd    # 使用 OpenOCD 烧录
make debug            # 启动 OpenOCD 调试服务器
make clean            # 清理构建产物
```

### 构建配置

- **目标平台**: thumbv7em-none-eabihf
- **优化级别**: opt-level 1 (保留调试信息)
- **LTO**: 禁用 (保留调试信息)
- **调试信息**: 完整 (debug = 2)

## 开发流程

1. **编译**: `make all`
2. **烧录**: `make flash-openocd`
3. **调试**: 
   - 终端1: `make debug`
   - 终端2: `arm-none-eabi-gdb target/thumbv7em-none-eabihf/release/rfid-stm32h750`

## 扩展指南

### 添加新控件

1. 在 `src/ui/widgets/` 创建新控件文件
2. 实现 `draw()` 方法和 `bounding_box()` 方法
3. 在 `src/ui/widgets/mod.rs` 的 `Widget` 枚举中添加新变体
4. 在 `src/ui/mod.rs` 中导出新控件

### 添加新传感器

1. 在 `src/` 创建新的传感器驱动文件
2. 实现传感器初始化和数据读取接口
3. 在 `main.rs` 中集成传感器
4. 创建对应的 UI 控件显示数据

### 优化性能

1. 使用 `profiler` 模块测量关键代码
2. 启用 `profiler` feature: `cargo build --features profiler`
3. 分析帧率和刷新时间
4. 优化热点代码

## 注意事项

1. **内存安全**: 使用 `heapless` 避免动态内存分配
2. **实时性**: 主循环延时 16ms (约 60 FPS)
3. **传感器读取**: DHT11 读取间隔 > 1 秒
4. **DMA 缓冲**: 位于 AXISRAM，确保 DMA 可访问
5. **SPI 时序**: CS 引脚必须在传输前后正确控制

## 参考文档

- [ILI9341 数据手册](https://cdn-shop.adafruit.com/datasheets/ILI9341.pdf)
- [STM32H7 参考手册](https://www.st.com/resource/en/reference_manual/dm00176879.pdf)
- [embedded-graphics 文档](https://docs.rs/embedded-graphics/)
- [stm32h7xx-hal 文档](https://docs.rs/stm32h7xx-hal/)
