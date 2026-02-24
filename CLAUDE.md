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

### LVGL 集成踩坑记录

#### 问题 1: lvgl-sys 需要 C 编译器
- **问题**: `lvgl` crate 依赖 `lvgl-sys`，需要 ARM GCC 交叉编译器 (`arm-none-eabi-gcc`)
- **错误**: `failed to find tool "arm-none-eabi-gcc"`
- **解决方案**: 创建纯 Rust UI 框架替代 LVGL，避免 C 依赖

#### 问题 2: lv_conf.h 配置复杂
- **问题**: `lvgl-sys` 需要设置 `DEP_LV_CONFIG_PATH` 环境变量指向配置文件
- **尝试方案**:
  - 在 `build.rs` 中设置环境变量 - 失败（依赖传递问题）
  - 在 `.cargo/config.toml` 中设置 - 可以工作但仍需 C 编译器
- **解决方案**: 放用 LVGL，使用纯 Rust 的 `ui.rs` 模块

#### 问题 3: heapless::Box 不存在
- **问题**: `heapless` crate 没有 `Box` 类型，无法存储 trait 对象
- **解决方案**: 使用枚举 `enum Widget` 存储不同类型的控件

#### 最终方案
- 创建纯 Rust UI 框架 (`src/ui.rs`)
- 支持的控件: Button, Label, ProgressBar
- 主题系统: 深色/浅色主题
- 完全基于 `embedded-graphics`，无 C 依赖
