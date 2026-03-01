# NTC 温度传感器读取模块设计

**日期**: 2026-03-01
**状态**: 已批准

## 概述

在 STM32H750 项目中添加 NTC 热敏电阻温度传感器读取功能，通过 PA3 引脚的 ADC 读取温度数据并显示在屏幕上。

## 硬件配置

| 参数 | 值 |
|------|-----|
| 传感器 | NTC 热敏电阻 |
| B 常数 | 3950 |
| 25°C 阻值 (R25) | 10kΩ |
| 分压电阻 | 10kΩ |
| 电路类型 | 上拉配置 (NTC 接 3.3V) |
| ADC 引脚 | PA3 (ADC1 Channel 3) |
| ADC 分辨率 | 12 位 (0-4095) |
| 参考电压 | 3.3V |

## 架构

```
┌─────────────┐      ┌─────────────┐      ┌─────────────┐
│    NTC      │──────│   ADC1      │──────│  NtcDriver  │
│  (PA3)      │      │  CH3        │      │             │
└─────────────┘      └─────────────┘      └──────┬──────┘
                                                  │
                                                  ▼
                                          ┌─────────────┐
                                          │TempHumidCard│
                                          │  (UI显示)   │
                                          └─────────────┘
```

## 模块结构

### 新增文件

```
src/adc_ntc.rs           # NTC 温度传感器驱动模块
```

### 修改文件

```
src/main.rs              # 集成 ADC 读取和温度更新
Cargo.toml               # 添加 embedded-hal 依赖
```

## API 设计

```rust
/// NTC 温度传感器驱动
pub struct NtcDriver {
    adc: ADC1,
    pin: PA3<Analog>,
    // 查表数据（静态生成）
}

impl NtcDriver {
    /// 初始化 NTC 驱动
    pub fn new(adc: ADC1, pin: PA3<Analog>, clocks: &Clocks) -> Self;

    /// 读取当前温度（摄氏度）
    pub fn read_temperature(&mut self) -> f32;

    /// 内部：ADC 原始值转温度（查表+插值）
    fn adc_to_temperature(&self, adc_value: u16) -> f32;
}
```

## 数据流

```
主循环 (每 500ms)
    │
    ├─→ ADC 读取 PA3
    │       └─→ 获取 12 位原始值 (0-4095)
    │
    ├─→ 查表转换
    │       ├─→ 找到温度区间
    │       └─→ 线性插值 → 精确温度值
    │
    └─→ 更新 UI
            ├─→ temp_card.sensor.update_temp(温度)
            ├─→ history.push(温度)
            └─→ screen.draw_with_dma()
```

## 查表设计

- **温度范围**: -10°C 到 100°C
- **步进**: 1°C (共 111 个条目)
- **精度**: 线性插值后约 0.1°C

### 温度计算公式

```
R_ntc = R25 * exp(B * (1/T - 1/298.15))  # Steinhart-Hart 简化版
V_out = Vref * PULL_R / (NTC_R + PULL_R)  # 分压公式
ADC = V_out / Vref * 4095
```

## UI 更新逻辑

1. 每 500ms 读取一次温度
2. 更新 TempHumidCard 显示
3. 维护最近 6 个温度值用于 HistoryBar 显示
4. 温度变化时才重绘（脏矩形优化）

## 配置参数

所有 NTC 参数集中在 `adc_ntc.rs` 顶部：

```rust
const NTC_B: f32 = 3950.0;      // B 常数
const NTC_R25: f32 = 10000.0;   // 25°C 时阻值 (Ω)
const PULL_R: f32 = 10000.0;    // 分压电阻 (Ω)
const VREF: f32 = 3.3;          // 参考电压 (V)
const TABLE_MIN: i16 = -10;     // 查表最低温度 (°C)
const TABLE_MAX: i16 = 100;     // 查表最高温度 (°C)
```

## 测试计划

1. **单元测试**: 验证 ADC 值到温度的转换
2. **硬件测试**: 使用已知温度源（如手握、冷水）验证
3. **UI 测试**: 验证温度显示和历史图表更新

## 参考资料

- STM32H7xx HAL ADC 文档
- NTC Thermistor Beta Equation
- 项目 CLAUDE.md 中的踩坑记录
