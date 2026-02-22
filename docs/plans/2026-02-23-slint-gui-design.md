# Slint GUI 集成设计

**日期**: 2026-02-23
**状态**: 设计阶段
**目标**: 将 embedded-graphics 驱动升级为 Slint GUI 框架

---

## 1. 概述

### 1.1 目标

在 STM32H750 + ILI9341 平台上集成 Slint GUI 框架，实现基于物理按键控制的图形用户界面。

### 1.2 范围

- **包含**: Slint 渲染器适配、UI 示例、物理按键输入处理
- **不包含**: 触摸输入支持（暂无硬件）

### 1.3 硬件环境

| 组件 | 型号/规格 |
| ---- | --------- |
| MCU | STM32H750VB |
| 屏幕 | ILI9341 240×320 |
| 连接 | SPI2 @ 48MHz |
| 输入 | 物理按键（非触摸） |

---

## 2. 架构设计

### 2.1 分层架构

```
┌─────────────────────────────────────────────────────┐
│                   Slint UI 层                         │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐          │
│  │  Button  │  │  Label   │  │  TextBox │          │
│  └──────────┘  └──────────┘  └──────────┘          │
└─────────────────────────────────────────────────────┘
                         ↓ Slint Renderer API
┌─────────────────────────────────────────────────────┐
│           SlintRenderer (新建)                        │
│  - 实现 slint::platform::Renderer trait              │
│  - 将 Slint 渲染指令转换为像素数据                    │
│  - 调用 DisplaySpi 绘制                              │
└─────────────────────────────────────────────────────┘
                         ↓ 调用
┌─────────────────────────────────────────────────────┐
│           DisplaySpi (现有, 复用)                     │
│  - 现有 DrawTarget 实现                              │
│  - ILI9341 硬件驱动                                  │
└─────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────┐
│           输入处理                                    │
│  按键中断 → Slint 事件模拟 (key press/聚焦)          │
└─────────────────────────────────────────────────────┘
```

### 2.2 设计原则

1. **复用现有驱动**: DisplaySpi 已有稳定的 ILI9341 驱动，无需重写
2. **最小改动**: 通过适配器模式连接 Slint 和现有驱动
3. **渐进式开发**: 先实现基本渲染，再添加交互功能

---

## 3. 组件设计

### 3.1 文件结构

```
src/
├── main.rs              # 入口，初始化 Slint
├── display.rs           # 现有 ILI9341 驱动（复用）
├── slint_renderer.rs    # 新增：Slint 渲染器适配器
└── ui/
    └── app.slint        # 新增：UI 定义文件
```

### 3.2 SlintRenderer 适配器

```rust
// src/slint_renderer.rs

use slint::platform::Renderer;
use embedded_graphics::pixelcolor::Rgb565;

pub struct SlintRenderer<SPI, CS, DC> {
    display: DisplaySpi<SPI, CS, DC>,
    buffer: [Rgb565; DISPLAY_WIDTH * DISPLAY_HEIGHT],
}

impl<SPI, CS, DC> Renderer for SlintRenderer<SPI, CS, DC>
where
    SPI: embedded_hal::blocking::spi::Transfer<u8>,
    CS: embedded_hal::digital::v2::OutputPin,
    DC: embedded_hal::digital::v2::OutputPin,
{
    type Error = DisplayError;

    fn render(&mut self, items: &[RenderItem]) -> Result<(), Self::Error> {
        // 1. 遍历渲染指令，将图形写入 buffer
        // 2. 计算需要更新的区域
        // 3. 调用 display.set_address() 设置显示区域
        // 4. 调用 display.send_pixels() 发送像素数据
    }
}
```

### 3.3 UI 示例 (ui/app.slint)

```slint
export component App inherits Window {
    width: 240px;
    height: 320px;
    title: "STM32H750 Slint Demo";

    VerticalLayout {
        padding: 16px;
        spacing: 12px;

        Text {
            text: "Hello Slint!";
            font-size: 24px;
            horizontal-alignment: center;
        }

        Button {
            text: "Click me";
            font-size: 18px;
            clicked => {
                // 按钮点击回调
                debug("Button clicked!");
            }
        }

        Text {
            text: "Use Tab to focus, Enter to click";
            font-size: 12px;
            color: #808080;
            wrap: word-wrap;
        }
    }
}
```

### 3.4 按键输入映射

```rust
// 物理按键 → Slint 键盘事件

#[derive(Clone, Copy)]
enum KeyCode {
    Tab,    // 切换焦点
    Enter,  // 激活按钮
}

fn handle_key_event(key: KeyCode) {
    use slint::platform::Key;

    let slint_key = match key {
        KeyCode::Tab => Key::Tab,
        KeyCode::Enter => Key::Return,
    };

    // 发送事件到 Slint
    slint::platform::process_events(
        &[WindowEvent::KeyPressed { text: slint_key }]
    );
}
```

---

## 4. 数据流设计

### 4.1 初始化流程

```
main()
  ↓
初始化 SPI/引脚 (stm32h7xx-hal)
  ↓
创建 DisplaySpi 实例
  ↓
display.init() - 初始化 ILI9341
  ↓
创建 SlintRenderer { display, buffer }
  ↓
注册为 Slint 平台渲染器
  ↓
加载 UI 编译后的 app.slint
  ↓
进入事件循环
```

### 4.2 渲染流程

```
Slint 检测到 UI 变化
  ↓
调用 SlintRenderer::render()
  ↓
遍历需要重绘的区域 (RenderItem[])
  ↓
将图形转换为 RGB565 像素 → buffer
  ↓
display.set_address(x, y, w, h) - 设置 ILI9341 显示区域
  ↓
display.send_pixels(&buffer) - 通过 SPI 发送像素
  ↓
ILI9341 显示更新
```

### 4.3 输入流程

```
物理按键按下 (中断或轮询)
  ↓
映射为 Slint KeyPress 事件
  ↓
Slint 处理事件:
  - Tab: 切换焦点到下一个控件
  - Enter: 激活当前聚焦的按钮
  ↓
触发 UI 回调 (如 button-clicked)
  ↓
UI 状态变化 → 触发渲染流程
```

---

## 5. 依赖项

### 5.1 Cargo.toml 变化

```toml
[dependencies]
stm32h7xx-hal = { version = "0.16", features = ["stm32h750v", "rt"] }
cortex-m = { version = "0.7", features = ["critical-section-single-core"] }
cortex-m-rt = {  version = "0.7" }

# GUI 框架
slint = { version = "1.8", default-features = false, features = ["renderer-software"] }

# 现有依赖（保留）
embedded-graphics = "0.7"
embedded-hal = "0.2"
panic-halt = "1.0"
nb = "1.0"

[build-dependencies]
slint-build = "1.8"
```

### 5.2 build.rs (新增)

```rust
fn main() {
    slint_build::compile("ui/app.slint").unwrap();
}
```

---

## 6. 成功标准

- [ ] Slint UI 能正确显示在 ILI9341 屏幕上
- [ ] 按键能切换控件焦点
- [ ] 按键能激活按钮并触发回调
- [ ] 渲染性能可接受（无明显卡顿）

---

## 7. 风险与缓解

| 风险 | 缓解措施 |
| ---- | -------- |
| Slint 内存占用过大 | 禁用不需要的 features，使用 renderer-software |
| 渲染性能不足 | 局部刷新，只更新变化区域 |
| 按键输入响应延迟 | 使用中断而非轮询 |
| 编译后固件过大 | 优化编译选项，检查 LTO 设置 |

---

## 8. 参考资料

- [Slint 官方文档](https://slint.dev/docs/)
- [Slint Rust 教程](https://slint.dev/releases/docs/latest/rust/)
- [Slint 自定义渲染器](https://slint.dev/releases/docs/latest/rust/custom_rendering/)
