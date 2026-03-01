# NTC 温度传感器实现计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**目标:** 在 STM32H750 上实现 NTC 热敏电阻温度传感器读取，并在屏幕上显示温度数据。

**架构:** 使用 ADC1 通道 3 (PA3) 读取 NTC 分压电路的模拟信号，通过预生成的查表法将 ADC 值转换为温度值，集成到现有的 TempHumidCard UI 中。

**技术栈:** Rust, stm32h7xx-hal, embedded-graphics, embedded-hal

---

## Task 1: 创建 ADC NTC 模块骨架

**文件:**
- 创建: `src/adc_ntc.rs`

**Step 1: 创建模块文件**

```rust
// src/adc_ntc.rs
//! NTC 热敏电阻温度传感器驱动模块
//!
//! 使用查表法将 ADC 值转换为温度

use stm32h7xx_hal::adc::{ADC1, Enabled, Resolution};
use stm32h7xx_hal::gpio::{gpioa::PA3, Analog};
use stm32h7xx_hal::time::Hertz;

/// NTC 温度传感器驱动
pub struct NtcDriver {
    _adc: ADC1<Enabled>,
    _pin: PA3<Analog>,
}

impl NtcDriver {
    /// 初始化 NTC 驱动
    pub fn new(adc: ADC1, pin: PA3<Analog>, clocks: &Hertz) -> Self {
        // ADC 配置将在下一步实现
        Self {
            _adc: adc.enable(),
            _pin: pin,
        }
    }
}
```

**Step 2: 在 main.rs 中声明模块**

在 `src/main.rs` 顶部添加：

```rust
mod adc_ntc;
```

**Step 3: 验证编译**

运行: `cargo check 2>&1 | head -30`

预期: 可能会有 ADC 类型相关的错误，这是预期的

**Step 4: 提交**

```bash
git add src/adc_ntc.rs src/main.rs
git commit -m "feat: add NTC driver module skeleton"
```

---

## Task 2: 实现 ADC 配置和读取

**文件:**
- 修改: `src/adc_ntc.rs`

**Step 1: 实现 ADC 配置**

完整替换 `src/adc_ntc.rs`:

```rust
// src/adc_ntc.rs
//! NTC 热敏电阻温度传感器驱动模块
//!
//! 使用查表法将 ADC 值转换为温度

use stm32h7xx_hal::adc::{ADC1, Enabled, Resolution, SampleTime};
use stm32h7xx_hal::gpio::{gpioa::PA3, Analog};
use stm32h7xx_hal::time::Hertz;
use stm32h7xx_hal::prelude::_stm32h7xx_hal_adc_AdcExt;

/// NTC 温度传感器驱动
pub struct NtcDriver {
    adc: ADC1<Enabled>,
    _pin: PA3<Analog>,
}

impl NtcDriver {
    /// 初始化 NTC 驱动
    pub fn new(mut adc: ADC1, pin: PA3<Analog>, _clocks: &Hertz) -> Self {
        // 配置 ADC
        adc.set_resolution(Resolution::TwelveBit);
        adc.set_sample_time(SampleTime::Cycles_640_5);

        Self {
            adc: adc.enable(),
            _pin: pin,
        }
    }

    /// 读取 ADC 原始值
    pub fn read_adc(&mut self) -> u16 {
        // 使用嵌入式 hal 的 ADC trait
        use embedded_hal::adc::OneShot;

        // 创建临时 ADC 引脚包装
        struct AdcWrapper<'a> {
            adc: &'a mut stm32h7xx_hal::adc::ADC1<Enabled>,
        }

        // 读取 ADC 值
        // 注意：stm32h7xx-hal 的 API 可能有差异，这里使用基本读取方法
        let adc_value: u16 = 0; // 占位符，将在调试中实现

        adc_value
    }
}
```

**Step 2: 检查 stm32h7xx-hal 的 ADC 示例**

运行: `find ~/.cargo -name "*.rs" -path "*stm32h7xx-hal*examples*" 2>/dev/null | xargs grep -l "adc" | head -5`

或查看在线文档以确认正确的 ADC API

**Step 3: 根据实际 API 调整实现**

**Step 4: 提交**

```bash
git add src/adc_ntc.rs
git commit -m "feat: implement ADC configuration and read method"
```

---

## Task 3: 生成温度-ADC 查表

**文件:**
- 创建: `scripts/gen_ntc_table.py` (Python 脚本用于生成查表)
- 修改: `src/adc_ntc.rs`

**Step 1: 创建查表生成脚本**

```bash
mkdir -p scripts
```

创建 `scripts/gen_ntc_table.py`:

```python
#!/usr/bin/env python3
"""生成 NTC 温度-ADC 查表"""

# NTC 参数
NTC_B = 3950.0      # B 常数
NTC_R25 = 10000.0   # 25°C 时阻值 (Ω)
PULL_R = 10000.0    # 分压电阻 (Ω)
VREF = 3.3          # 参考电压 (V)
ADC_MAX = 4095.0    # 12 位 ADC 最大值

# 查表参数
TABLE_MIN = -10     # 最低温度 (°C)
TABLE_MAX = 100     # 最高温度 (°C)

def kelvin_to_celsius(k: float) -> float:
    return k - 273.15

def celsius_to_kelvin(c: float) -> float:
    return c + 273.15

def ntc_resistance(temp_c: float) -> float:
    """计算指定温度下的 NTC 阻值"""
    temp_k = celsius_to_kelvin(temp_c)
    t25 = celsius_to_kelvin(25.0)
    return NTC_R25 * (NTC_B * (1.0/temp_k - 1.0/t25)).exp()

def adc_value(temp_c: float) -> int:
    """计算指定温度对应的 ADC 值"""
    r_ntc = ntc_resistance(temp_c)
    # 上拉配置: NTC 接 VCC，固定电阻接 GND
    # V_out = VREF * PULL_R / (NTC_R + PULL_R)
    v_out = VREF * PULL_R / (r_ntc + PULL_R)
    adc = int(v_out / VREF * ADC_MAX + 0.5)
    return max(0, min(int(ADC_MAX), adc))

# 生成 Rust 常量数组
print("// Auto-generated NTC temperature lookup table")
print("// DO NOT EDIT - run scripts/gen_ntc_table.py to regenerate")
print()
print("const ADC_TABLE: [u16; {}] = [".format(TABLE_MAX - TABLE_MIN + 1))

for temp in range(TABLE_MIN, TABLE_MAX + 1):
    adc = adc_value(temp)
    if temp % 10 == 0:
        print(f"    // {temp}°C")
    print(f"    {adc},")
    if (temp - TABLE_MIN + 1) % 8 == 0:
        print()

print("];")
```

**Step 2: 运行脚本生成查表数据**

```bash
python3 scripts/gen_ntc_table.py > src/adc_ntc_table.txt 2>&1
cat src/adc_ntc_table.txt
```

**Step 3: 将生成的查表添加到 adc_ntc.rs**

在 `src/adc_ntc.rs` 中添加：

```rust
// Auto-generated NTC temperature lookup table
// Temperature range: -10°C to 100°C (111 entries)

const ADC_TABLE: [u16; 111] = [
    // -10°C
    150, 151, 152, 153, 155, 156, 157, 158,
    // ... (实际生成值)
];
```

**Step 4: 提交**

```bash
git add scripts/gen_ntc_table.py src/adc_ntc.rs
git commit -m "feat: add NTC temperature lookup table"
```

---

## Task 4: 实现温度转换函数

**文件:**
- 修改: `src/adc_ntc.rs`

**Step 1: 实现 ADC 到温度的转换**

在 `NtcDriver` impl 中添加：

```rust
    /// 将 ADC 值转换为温度（摄氏度）
    ///
    /// 使用查表法和线性插值
    pub fn read_temperature(&mut self) -> f32 {
        let adc_value = self.read_adc();
        self.adc_to_temperature(adc_value)
    }

    /// ADC 值转温度（内部函数）
    fn adc_to_temperature(&self, adc_value: u16) -> f32 {
        const TABLE_MIN_TEMP: i16 = -10;
        const TABLE_MAX_TEMP: i16 = 100;

        // 边界检查
        if adc_value <= ADC_TABLE[0] {
            return TABLE_MIN_TEMP as f32;
        }
        if adc_value >= ADC_TABLE[ADC_TABLE.len() - 1] {
            return TABLE_MAX_TEMP as f32;
        }

        // 查找 ADC 值在表中的位置
        let mut i = 0;
        while i < ADC_TABLE.len() - 1 && ADC_TABLE[i + 1] < adc_value {
            i += 1;
        }

        // 线性插值
        let temp_low = (TABLE_MIN_TEMP + i as i16) as f32;
        let temp_high = temp_low + 1.0;
        let adc_low = ADC_TABLE[i] as f32;
        let adc_high = ADC_TABLE[i + 1] as f32;

        let ratio = (adc_value as f32 - adc_low) / (adc_high - adc_low);
        temp_low + ratio
    }
```

**Step 2: 验证编译**

运行: `cargo check 2>&1 | head -20`

**Step 3: 提交**

```bash
git add src/adc_ntc.rs
git commit -m "feat: implement ADC to temperature conversion"
```

---

## Task 5: 集成到 main.rs

**文件:**
- 修改: `src/main.rs`

**Step 1: 在 GPIO 初始化部分添加 PA3 配置**

找到 `let gpioa = dp.GPIOA.split(ccdr.peripheral.GPIOA);` 这一行，在其后添加：

```rust
    // NTC 温度传感器引脚
    let ntc_pin = gpioa.pa3.into_analog();
```

**Step 2: 在 ADC 初始化部分添加 ADC1 配置**

在 SPI 初始化之前，添加：

```rust
    // 初始化 ADC1 用于 NTC 温度传感器
    let mut adc1 = dp.ADC1.adc(
        &mut delay_ms,
        &ccdr.clocks,
        ccdr.peripheral.ADC12,
    );
```

**Step 3: 创建 NTC 驱动实例**

在屏幕创建之后，添加：

```rust
    // 初始化 NTC 温度传感器
    let mut ntc = adc_ntc::NtcDriver::new(adc1, ntc_pin, &ccdr.clocks);
```

**Step 4: 在主循环中添加温度读取**

找到主循环 `loop {`，修改：

```rust
    // 主循环
    loop {
        frame_count += 1;

        // LED 慢闪表示运行中
        if frame_count % 60 == 0 {
            let _ = led.toggle();
        }

        // 每 30 帧 (约 500ms) 读取一次温度
        if frame_count % 30 == 0 {
            let temp = ntc.read_temperature();

            // 更新温度卡片
            if let Some(card) = screen.widgets.temp_humid_cards.get_mut(0) {
                card.sensor.update_temp(temp);

                // 更新历史数据
                if let Some(history) = screen.widgets.history_bars.get_mut(0) {
                    // 获取当前历史数据并更新
                    let mut temps = [temp; 6];
                    // TODO: 这里需要实现历史数据的滚动更新
                    history.update(&temps);
                }
            }

            // 重绘屏幕
            screen.draw_with_dma(&mut display).unwrap();
        }

        delay_ms(16); // 约 60fps
    }
```

**Step 5: 验证编译**

运行: `cargo check 2>&1 | head -30`

**Step 6: 提交**

```bash
git add src/main.rs
git commit -m "feat: integrate NTC temperature reading into main loop"
```

---

## Task 6: 添加历史数据滚动更新

**文件:**
- 修改: `src/main.rs` 或添加历史缓冲区

**Step 1: 在 main 函数开始处添加历史缓冲区**

```rust
    // 温度历史缓冲区（最近 6 个读数）
    let mut temp_history: [f32; 6] = [25.0; 6];
    let mut history_index: usize = 0;
```

**Step 2: 修改主循环中的温度读取部分**

```rust
        // 每 30 帧 (约 500ms) 读取一次温度
        if frame_count % 30 == 0 {
            let temp = ntc.read_temperature();

            // 更新历史数据（滚动缓冲区）
            temp_history[history_index] = temp;
            history_index = (history_index + 1) % 6;

            // 更新温度卡片
            if let Some(card) = screen.widgets.temp_humid_cards.get_mut(0) {
                card.sensor.update_temp(temp);
            }

            // 更新历史图表
            if let Some(history) = screen.widgets.history_bars.get_mut(0) {
                history.update(&temp_history);
            }

            // 重绘屏幕
            screen.draw_with_dma(&mut display).unwrap();
        }
```

**Step 3: 验证编译**

运行: `cargo check 2>&1 | head -20`

**Step 4: 提交**

```bash
git add src/main.rs
git commit -m "feat: add temperature history rolling buffer"
```

---

## Task 7: 烧录测试

**Step 1: 编译**

```bash
cargo build 2>&1 | tail -20
```

**Step 2: 烧录**

```bash
probe-rs run --chip STM32H750VBTx target/thumbv7em-none-eabihf/debug/rfid-stm32h750
```

**Step 3: 验证功能**

预期行为：
- 屏幕显示温度卡片
- 温度值每 500ms 更新一次
- 历史图表显示最近的温度趋势
- 用手握住 NTC，温度应该上升

**Step 4: 调试（如有问题）**

通过串口输出调试信息检查 ADC 原始值

**Step 5: 记录测试结果**

更新项目文档或踩坑记录

---

## 附录: 查表数据参考

根据 NTC 参数计算的温度-ADC 对应关系（上拉配置）:

| 温度 (°C) | NTC 阻值 (kΩ) | 输出电压 (V) | ADC 值 |
|----------|--------------|-------------|--------|
| -10      | ~57          | ~0.57       | ~706   |
| 0        | ~35          | ~0.86       | ~1067  |
| 10       | ~21          | ~1.18       | ~1464  |
| 20       | ~12.5        | ~1.55       | ~1924  |
| 25       | 10           | 1.65        | 2048   |
| 30       | ~8           | ~1.83       | ~2274  |
| 40       | ~5.1         | ~2.20       | ~2732  |
| 50       | ~3.4         | ~2.47       | ~3065  |
| 60       | ~2.3         | ~2.69       | ~3340  |
| 70       | ~1.6         | ~2.86       | ~3553  |
| 80       | ~1.1         | ~2.99       | ~3714  |
| 90       | ~0.8         | ~3.09       | ~3836  |
| 100      | ~0.6         | ~3.17       | ~3934  |

---

## 注意事项

1. **ADC 校准**: STM32H7 的 ADC 可能需要校准以获得准确读数
2. **采样时间**: 使用较长的采样时间 (640.5 周期) 确保稳定
3. **电源噪声**: 3.3V 电源的噪声会影响 ADC 读数，可添加软件滤波
4. **NTC 参数**: 如果温度读数不准，可能需要调整 NTC_B 和 NTC_R25 参数
