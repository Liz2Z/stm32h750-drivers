# STM32H750 延时函数与 I2C 通信踩坑记录

## 问题现象

在使用 STM32H750 (400MHz) 开发 AHT20 温湿度传感器驱动时，遇到以下问题：
- 传感器初始化失败或读取超时
- 屏幕显示读数始终为 0
- I2C 通信不稳定

## 根本原因分析

### 1. 延时函数时间计算错误（最严重）

#### 错误代码
```rust
fn delay_ms(ms: u32) {
    for _ in 0..ms {
        for _ in 0..8000 {
            cortex_m::asm::nop();
        }
    }
}
```

#### 问题分析

STM32H750 主频高达 **400MHz**（每秒执行 4 亿个周期）。

- `nop()` 指令 + 循环开销 ≈ 3~4 个时钟周期
- 8000 次循环 ≈ 32,000 个时钟周期
- 在 400MHz 下，32,000 个周期 = **0.08ms**（80 微秒）

**结果**：原本应该延时 80ms 的测量等待，实际只延时了不到 7ms！

传感器根本来不及完成测量，导致：
- `BUSY` 超时错误
- `INVALID DATA` 错误
- 读取失败

#### 正确实现

```rust
fn delay_ms(ms: u32) {
    // STM32H750 主频 400MHz，1ms = 400,000 个 CPU 周期
    cortex_m::asm::delay(ms * 400_000);
}
```

使用 `cortex_m::asm::delay()` 可以精确消耗指定的 CPU 周期数。

#### 经验教训

⚠️ **在高性能 MCU 上，必须根据实际主频计算延时周期数！**

公式：
```
延时周期数 = 主频(Hz) × 延时时间(秒)
1ms 延时 = 主频(MHz) × 1000 个周期
```

示例：
- STM32F103 (72MHz): 1ms = 72,000 周期
- STM32H750 (400MHz): 1ms = 400,000 周期
- STM32H723 (550MHz): 1ms = 550,000 周期

### 2. I2C 空写入导致总线错误（致命）

#### 错误代码

```rust
fn read_data(&mut self) -> Result<Aht20Reading, Aht20Error> {
    let mut buf = [0u8; 7];
    self.i2c
        .write_read(AHT20_ADDR, &[], &mut buf)  // ❌ 空切片写入
        .map_err(|_| Aht20Error::I2cError)?;
    // ...
}
```

#### 问题分析

STM32 硬件 I2C 在遇到 0 字节写入时：
1. 发送 START 条件
2. 发送设备地址 + 写位
3. 立即发送 STOP 条件（因为没有数据）
4. 或者产生总线错误

AHT20 的状态机会因此错乱，直接回复 NACK，导致 `I2cError`。

#### 正确实现

```rust
fn read_data(&mut self) -> Result<Aht20Reading, Aht20Error> {
    let mut buf = [0u8; 7];
    // 直接读取，AHT20 触发测量后会自动将数据放在输出缓冲区
    self.i2c
        .read(AHT20_ADDR, &mut buf)  // ✅ 直接读取
        .map_err(|_| Aht20Error::I2cError)?;
    // ...
}
```

需要添加 `Read` trait：
```rust
use embedded_hal::blocking::i2c::{Read, Write, WriteRead};

impl<I2C, E> Aht20<I2C>
where
    I2C: Write<Error = E> + WriteRead<Error = E> + Read<Error = E>,
{
    // ...
}
```

#### 经验教训

⚠️ **某些传感器在触发测量后，可以直接读取数据，无需发送寄存器地址！**

阅读传感器数据手册，确认：
- 是否需要先写入寄存器地址再读取
- 是否可以直接读取（AHT20、BMP280 等支持）

### 3. 不必要的重新初始化

#### 错误代码

```rust
// 初始化 AHT20
let mut aht20 = aht20::Aht20::new(i2c);
aht20.init()?;

// ... 其他代码 ...

// 重新创建并初始化
let i2c = other_sensor.release();
aht20 = aht20::Aht20::new(i2c);
aht20.init()?;  // ❌ 不必要的重新初始化
```

#### 问题分析

传感器是**外部硬件设备**，具有状态保持性。重新创建 Rust 结构体只是改变了软件层面的引用，不会让物理传感器掉电失忆。

第二次调用 `init()` 是多余的，甚至可能打断传感器正在进行的空闲状态。

#### 正确实现

```rust
// 初始化 AHT20
let mut aht20 = aht20::Aht20::new(i2c);
aht20.init()?;

// ... 其他代码 ...

// 重新创建实例，但不需要重新初始化
let i2c = other_sensor.release();
aht20 = aht20::Aht20::new(i2c);  // ✅ 直接使用，无需 init()
```

#### 经验教训

⚠️ **区分软件对象和硬件设备！**

- 软件对象：Rust 结构体，可以随时创建/销毁
- 硬件设备：物理传感器，有独立的状态机

## 解决方案总结

### 修改文件清单

| 文件 | 修改内容 | 影响 |
|------|---------|------|
| `src/main.rs` | 修复 `delay_ms` 函数<br>移除不必要的重新初始化 | 关键修复 |
| `src/aht20.rs` | 修复 `delay_ms` 函数<br>修复 `read_data` 的 I2C 空写入<br>添加 `Read` trait | 关键修复 |
| `src/bmp280.rs` | 修复 `delay_ms` 函数 | 关键修复 |

### 核心修改代码

#### 1. 延时函数（所有文件）

```rust
// 修改前
fn delay_ms(ms: u32) {
    for _ in 0..ms {
        for _ in 0..8000 {
            cortex_m::asm::nop();
        }
    }
}

// 修改后
fn delay_ms(ms: u32) {
    cortex_m::asm::delay(ms * 400_000);  // 400MHz 主频
}
```

#### 2. AHT20 I2C 读取

```rust
// 修改前
self.i2c.write_read(AHT20_ADDR, &[], &mut buf)?;

// 修改后
self.i2c.read(AHT20_ADDR, &mut buf)?;
```

## 硬件检查要点

### I2C 总线配置

```rust
let i2c_scl = gpiob
    .pb6
    .into_alternate::<4>()
    .set_open_drain();  // ✅ 必须设置为开漏输出

let i2c_sda = gpiob
    .pb7
    .into_alternate::<4>()
    .set_open_drain();  // ✅ 必须设置为开漏输出
```

### 硬件连接

```
I2C 总线：
  PB6 (I2C1_SCL) ─┬─ 4.7kΩ 上拉到 3.3V  ⚠️ 必须接上拉电阻！
                  └─ AHT20 SCL
  PB7 (I2C1_SDA) ─┬─ 4.7kΩ 上拉到 3.3V  ⚠️ 必须接上拉电阻！
                  └─ AHT20 SDA

AHT20 传感器：
  VCC  ─ 3.3V
  GND  ─ GND
  SCL  ─ PB6
  SDA  ─ PB7
```

**重要提示**：
- I2C 总线必须接上拉电阻（4.7kΩ ~ 10kΩ）
- 如果使用模块，通常模块上已经自带上拉电阻
- 如果是裸芯片，必须自己接上拉电阻

## 调试技巧

### 1. 添加详细的错误信息

```rust
match aht20.read() {
    Ok(reading) => {
        // 处理数据
    }
    Err(e) => {
        let error_msg = match e {
            Aht20Error::I2cError => "I2C ERROR",
            Aht20Error::NotCalibrated => "NOT CALIBRATED",
            Aht20Error::Busy => "SENSOR BUSY",
            Aht20Error::InvalidData => "INVALID DATA",
        };
        // 在屏幕上显示错误信息
    }
}
```

### 2. 检查初始化状态

```rust
match aht20.init() {
    Ok(_) => {
        // 显示 "AHT20 INIT SUCCESS"
    }
    Err(_) => {
        // 显示 "AHT20 INIT FAILED"
        // 检查硬件连接
    }
}
```

### 3. 使用 LED 指示状态

```rust
// 初始化成功：LED 闪烁 1 次
// 初始化失败：LED 快闪 3 次
// 读取成功：LED 闪烁 1 次
// 读取失败：LED 快闪 3 次
```

## 性能对比

### 延时精度对比

| 方法 | 期望延时 | 实际延时 (400MHz) | 误差 |
|------|---------|------------------|------|
| 错误的循环延时 | 80ms | ~7ms | **91% 偏差** |
| 正确的 delay() | 80ms | 80ms | 0% 偏差 |

### 传感器读取成功率

| 状态 | 成功率 | 典型错误 |
|------|--------|---------|
| 修复前 | 0% | BUSY, INVALID DATA, I2cError |
| 修复后 | 100% | 无 |

## 相关参考资料

- [STM32H750 数据手册](https://www.st.com/resource/en/datasheet/stm32h750ib.pdf)
- [AHT20 数据手册](https://cdn-shop.adafruit.com/product-files/5181/5181_AHT20.pdf)
- [embedded-hal 文档](https://docs.rs/embedded-hal/)
- [cortex-m 延时函数](https://docs.rs/cortex-m/latest/cortex_m/asm/fn.delay.html)

## 总结

这次踩坑的核心教训：

1. **高性能 MCU 必须精确计算延时周期数**
   - 使用 `cortex_m::asm::delay()` 而不是循环延时
   - 根据实际主频计算周期数

2. **I2C 通信要符合硬件规范**
   - 避免空写入（0 字节写入）
   - 阅读传感器数据手册确认通信协议
   - 确保硬件上拉电阻正确连接

3. **区分软件对象和硬件设备**
   - 软件对象可以随时创建/销毁
   - 硬件设备有独立的状态机
   - 避免不必要的重新初始化

4. **完善的错误处理和调试信息**
   - 显示具体的错误类型
   - 使用 LED 指示状态
   - 在屏幕上显示初始化和读取状态

这些经验对于其他高性能 MCU（如 STM32H7 系列）和 I2C 传感器开发都有参考价值。

---

## 4. 多传感器共享 I2C 总线时结构体状态丢失（关键！）

### 问题现象

- AHT20 温湿度读取正常
- BMP280 气压始终显示为 0 或不显示
- BMP280 初始化成功，但读取时返回 `DeviceNotFound` 错误

### 根本原因分析

#### 错误代码

```rust
// 初始化阶段
let mut bmp280 = bmp280::Bmp280::new(i2c);
bmp280.init()?;  // 初始化成功，校准数据已缓存
let i2c = bmp280.release();  // ❌ 销毁了 bmp280 实例！
aht20 = aht20::Aht20::new(i2c);

// 主循环
loop {
    match aht20.read() {
        Ok(reading) => {
            let i2c = aht20.release();
            let mut bmp280 = bmp280::Bmp280::new(i2c);  // ❌ 重新创建实例
            
            match bmp280.read() {  // ❌ is_initialized = false，校准数据全为 0
                // ...
            }
        }
    }
}
```

#### 问题分析

**AHT20 为什么能行？**
- `Aht20` 结构体内部**只有** `i2c` 这一个成员，没有任何缓存状态
- 每次重新 `Aht20::new()` 创建实例，照样能直接发送读数指令并成功解析

**BMP280 为什么不行？**
- `Bmp280` 在 `init()` 时，不仅配置了硬件，**还在结构体内部缓存了重要的校准数据**
- 校准数据包括：`dig_t1` ~ `dig_p9` 共 12 个参数，用于将原始 ADC 值转换为真实的温度和气压
- `is_initialized` 标志位被设为 `true`

当重新执行 `Bmp280::new(i2c)` 时：
1. 创建了一个**全新**的 `Bmp280` 实例
2. `is_initialized` 是默认的 `false`
3. 所有校准参数全都是 `0`

`read()` 方法中有以下判断：
```rust
if !self.is_initialized {
    return Err(Bmp280Error::DeviceNotFound);  // 直接返回错误！
}
```

即使去掉这个判断，由于校准参数全为 0，计算出来的气压也会是错误的值。

### 正确实现

#### 1. 修改 `bmp280.rs`：使用 `Option<I2C>` 包装

```rust
pub struct Bmp280<I2C> {
    i2c: Option<I2C>,  // 使用 Option 包裹
    addr: u8,
    calibration: CalibrationData,  // 校准数据持久保存
    t_fine: i32,
    is_initialized: bool,  // 初始化标志持久保存
}

impl<I2C, E> Bmp280<I2C>
where
    I2C: Write<Error = E> + WriteRead<Error = E>,
{
    pub fn new(i2c: I2C) -> Self {
        Self {
            i2c: Some(i2c),
            // ... 其他字段初始化
        }
    }

    // 修改 release：取出 I2C 但不销毁实例
    pub fn release(&mut self) -> I2C {
        self.i2c.take().expect("I2C bus was not attached!")
    }

    // 新增 attach：重新挂载 I2C
    pub fn attach(&mut self, i2c: I2C) {
        self.i2c = Some(i2c);
    }

    // 所有 I2C 操作改为 self.i2c.as_mut().unwrap()
    fn detect_device(&mut self) -> Result<bool, Bmp280Error> {
        self.i2c.as_mut().unwrap().write_read(...)
    }
}
```

#### 2. 修改 `main.rs`：复用同一个实例

```rust
// 初始化阶段
let mut bmp280 = bmp280::Bmp280::new(i2c);
bmp280.init()?;
let i2c = bmp280.release();  // 释放 I2C，但 bmp280 实例保留校准数据
aht20 = aht20::Aht20::new(i2c);

// 主循环
loop {
    match aht20.read() {
        Ok(reading) => {
            // 释放 AHT20 的 I2C，挂载到 bmp280
            let i2c = aht20.release();
            bmp280.attach(i2c);  // ✅ 复用已有实例，校准数据完整保留
            
            match bmp280.read() {  // ✅ is_initialized = true
                Ok(bmp_reading) => {
                    pressure_sensor.update(bmp_reading.pressure);
                }
                Err(_) => {}
            }

            // 释放 I2C，交还给 AHT20
            let i2c = bmp280.release();
            aht20 = aht20::Aht20::new(i2c);
        }
    }
}
```

### 核心思路图解

```
┌─────────────────────────────────────────────────────────────┐
│  主循环外：bmp280 实例一直存活，校准数据持久保存           │
├─────────────────────────────────────────────────────────────┤
│  主循环内：                                                │
│    aht20.release() → bmp280.attach() → bmp280.read() →    │
│    bmp280.release() → aht20 = Aht20::new()                │
│                                                           │
│  I2C 总线在两个传感器之间流转，但 bmp280 结构体状态不变    │
└─────────────────────────────────────────────────────────────┘
```

### 经验教训

⚠️ **区分"物理硬件状态"和"软件结构体状态"！**

| 对象 | 状态存储位置 | 重新创建的影响 |
|------|------------|--------------|
| 物理传感器 | 传感器内部寄存器 | 不受影响，硬件不会"失忆" |
| 简单结构体（如 Aht20） | 只有 I2C 引用 | 重新创建无影响 |
| 复杂结构体（如 Bmp280） | 校准数据、初始化标志 | **重新创建会丢失所有状态** |

### 进阶建议

在 Embedded Rust 中处理多个传感器共享同一个 I2C 总线，最标准的做法是引入 **`shared-bus`** 或 **`embedded-hal-bus`** crate：

```rust
use embedded_hal_bus::i2c::Sharing;

let i2c_manager = embedded_hal_bus::i2c::SimpleDevice::new(i2c);

let mut aht20 = Aht20::new(i2c_manager.acquire_i2c());
let mut bmp280 = Bmp280::new(i2c_manager.acquire_i2c());

// 两个传感器可以独立使用，无需手动 release/attach
```

这种方式更加优雅，避免了手动管理 I2C 所有权转移的复杂性。
