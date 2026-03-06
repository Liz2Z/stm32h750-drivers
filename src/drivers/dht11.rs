//! DHT11 温湿度传感器驱动
//!
//! DHT11 是一款低成本温湿度传感器，广泛用于智能家居、环境监测等场景。
//! 它使用单总线协议通信，只需要一根数据线就能同时传输温度和湿度数据。
//!
//! ## 为什么选择 DHT11？
//!
//! - 价格便宜（约 5 元人民币）
//! - 接口简单（仅需一根数据线）
//! - 精度足够日常使用（温度 ±2°C，湿度 ±5%RH）
//!
//! ## 工作原理
//!
//! DHT11 内部有一个湿度敏感元件和一个 NTC 测温元件。
//! 当主机发送启动信号后，DHT11 会返回 40 位数据：
//! - 前 16 位：湿度（整数 + 小数）
//! - 中间 16 位：温度（整数 + 小数）
//! - 最后 8 位：校验和（用于验证数据完整性）
//!
//! ## 硬件连接
//!
//! | STM32 | DHT11 | 说明 |
//! |-------|-------|------|
//! | PA2 | DATA | 数据引脚（必须配置为开漏输出）|
//! | VCC | VCC | 3.3V 或 5V 电源 |
//! | GND | GND | 地 |
//!
//! ⚠️ **重要**：DATA 引脚必须接 4.7K~10K 上拉电阻，否则无法正常工作！
//!
//! ## 使用限制
//!
//! - 两次读取间隔至少 1~2 秒（传感器需要时间进行模数转换）
//! - 测量范围：温度 0-50°C，湿度 20-90%RH
//! - 不适合高精度测量场景
//!
//! 注意：此模块为预留功能，暂未在主程序中使用

#![allow(dead_code)]

use embedded_hal::digital::v2::{InputPin, OutputPin};

/// 读取传感器时可能发生的错误
///
/// 在实际使用中，DHT11 可能因为以下原因读取失败：
/// - 传感器未连接或接线错误
/// - 上拉电阻缺失或阻值不对
/// - 读取间隔太短（传感器还没准备好）
/// - 环境干扰导致数据传输错误
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DhtError {
    /// 传感器无响应或响应超时
    /// 通常意味着硬件连接问题
    Timeout,
    /// 数据校验失败
    /// 说明传输过程中发生了错误，数据不可信
    ChecksumMismatch,
}

/// 温湿度测量结果
///
/// 包含从 DHT11 读取的温度和湿度值。
/// 这两个值已经过校验和验证，可以放心使用。
#[derive(Debug, Clone, Copy, Default)]
pub struct DhtReading {
    /// 温度，单位：摄氏度（°C）
    /// 例如：25.5 表示 25.5°C
    pub temperature: f32,
    /// 相对湿度，单位：百分比（%RH）
    /// 例如：60.0 表示 60% 相对湿度
    pub humidity: f32,
}

/// DHT11 传感器驱动
///
/// 这个驱动封装了与 DHT11 通信的所有细节，用户只需调用 `read()` 方法
/// 即可获得温湿度数据。
///
/// # 使用示例
///
/// ```no_run
/// // 1. 将 GPIO 配置为开漏输出（这是必须的，因为 DHT11 的数据线是双向的）
/// let dht_pin = gpioa.pa2.into_open_drain_output();
///
/// // 2. 创建驱动实例
/// let mut dht11 = Dht11::new(dht_pin);
///
/// // 3. 等待传感器稳定（上电后需要约 1 秒）
/// delay_ms(1000);
///
/// // 4. 读取数据
/// loop {
///     match dht11.read() {
///         Ok(reading) => {
///             // 读取成功，可以使用数据
///             println!("温度: {}°C, 湿度: {}%", reading.temperature, reading.humidity);
///         }
///         Err(_) => {
///             // 读取失败，等待后重试
///             // 失败是正常的，DHT11 不是每次都能成功
///         }
///     }
///     delay_ms(2000); // 至少等待 2 秒再读取
/// }
/// ```
pub struct Dht11<Pin> {
    /// 数据引脚
    /// 配置为开漏输出模式，既能输出信号也能读取传感器的响应
    pin: Pin,
}

impl<Pin, E> Dht11<Pin>
where
    Pin: OutputPin<Error = E> + InputPin<Error = E>,
{
    /// 创建 DHT11 驱动
    ///
    /// 创建后建议等待 1~2 秒再进行第一次读取，
    /// 让传感器有足够时间完成内部初始化。
    pub fn new(pin: Pin) -> Self {
        Self { pin }
    }

    /// 读取温湿度
    ///
    /// 这是主要的使用接口。一次读取大约需要 5ms。
    ///
    /// # 返回值
    ///
    /// - `Ok(DhtReading)`: 读取成功
    /// - `Err(DhtError::Timeout)`: 传感器无响应，检查接线
    /// - `Err(DhtError::ChecksumMismatch)`: 数据校验失败，建议重试
    ///
    /// # 注意
    ///
    /// 不要频繁调用此方法，建议间隔至少 2 秒。
    /// DHT11 内部需要时间进行模数转换，频繁读取会失败。
    pub fn read(&mut self) -> Result<DhtReading, DhtError> {
        let data = self.read_raw()?;

        // DHT11 返回的数据格式：
        // byte[0]: 湿度整数部分（例如 60 表示 60%）
        // byte[1]: 湿度小数部分（例如 5 表示 0.5%，DHT11 通常为 0）
        // byte[2]: 温度整数部分（例如 25 表示 25°C）
        // byte[3]: 温度小数部分（例如 5 表示 0.5°C，DHT11 通常为 0）
        Ok(DhtReading {
            temperature: data[2] as f32 + data[3] as f32 * 0.1,
            humidity: data[0] as f32 + data[1] as f32 * 0.1,
        })
    }

    /// 读取原始 5 字节数据
    ///
    /// 这是内部方法，完成完整的通信流程：
    /// 1. 发送启动信号（告诉传感器"我要读取数据了"）
    /// 2. 等待传感器响应
    /// 3. 接收 40 位数据（5 个字节）
    /// 4. 验证校验和
    fn read_raw(&mut self) -> Result<[u8; 5], DhtError> {
        // 步骤 1：发送启动信号
        self.send_start_signal()?;

        // 步骤 2：等待传感器响应
        self.wait_for_response()?;

        // 步骤 3：读取 5 个字节的数据
        let mut data = [0u8; 5];
        for byte in data.iter_mut() {
            *byte = self.read_byte()?;
        }

        // 步骤 4：验证校验和
        // 校验和 = 前 4 个字节相加的低 8 位
        // 如果校验失败，说明传输过程中有干扰，数据不可信
        let checksum = data[0]
            .wrapping_add(data[1])
            .wrapping_add(data[2])
            .wrapping_add(data[3]);

        if checksum != data[4] {
            return Err(DhtError::ChecksumMismatch);
        }

        Ok(data)
    }

    /// 发送启动信号
    ///
    /// 启动信号的作用是"唤醒" DHT11，告诉它主机要读取数据了。
    ///
    /// 时序：
    /// 1. 主机拉低数据线至少 18ms（让传感器检测到启动信号）
    /// 2. 主机释放数据线（拉高），等待传感器响应
    fn send_start_signal(&mut self) -> Result<(), DhtError> {
        // 拉低 20ms，超过 DHT11 要求的 18ms 最小值
        let _ = self.pin.set_low();
        delay_ms(20);

        // 释放数据线，准备接收响应
        let _ = self.pin.set_high();
        delay_us(40);

        Ok(())
    }

    /// 等待 DHT11 响应
    ///
    /// DHT11 收到启动信号后，会发送响应信号表示"我准备好了"：
    /// 1. 拉低 80μs（表示开始响应）
    /// 2. 拉高 80μs（表示准备发送数据）
    ///
    /// 如果这个阶段超时，通常意味着：
    /// - 传感器未连接
    /// - 上拉电阻缺失
    /// - 传感器损坏
    fn wait_for_response(&mut self) -> Result<(), DhtError> {
        // 等待 DHT11 拉低（开始响应）
        let mut timeout = 1000;
        while self.pin.is_high().unwrap_or(true) {
            timeout -= 1;
            if timeout == 0 {
                return Err(DhtError::Timeout);
            }
            delay_us(1);
        }

        // 等待 DHT11 拉高（响应结束）
        timeout = 1000;
        while self.pin.is_low().unwrap_or(true) {
            timeout -= 1;
            if timeout == 0 {
                return Err(DhtError::Timeout);
            }
            delay_us(1);
        }

        // 等待 DHT11 再次拉低（开始发送数据）
        timeout = 1000;
        while self.pin.is_high().unwrap_or(true) {
            timeout -= 1;
            if timeout == 0 {
                return Err(DhtError::Timeout);
            }
            delay_us(1);
        }

        Ok(())
    }

    /// 读取一个字节
    ///
    /// DHT11 发送数据的编码方式：
    /// - 每个位都以 50μs 低电平开始
    /// - 然后是高电平：
    ///   - 26-28μs 高电平 = "0"
    ///   - 70μs 高电平 = "1"
    ///
    /// 通过测量高电平的持续时间，我们就能区分 0 和 1。
    fn read_byte(&mut self) -> Result<u8, DhtError> {
        let mut byte = 0u8;

        for _ in 0..8 {
            // 等待低电平结束（50μs 的起始信号）
            let mut timeout = 1000;
            while self.pin.is_low().unwrap_or(true) {
                timeout -= 1;
                if timeout == 0 {
                    return Err(DhtError::Timeout);
                }
                delay_us(1);
            }

            // 等待 28μs 后检测电平
            // 如果是 "0"，此时已经是低电平
            // 如果是 "1"，此时还是高电平
            delay_us(28);

            if self.pin.is_high().unwrap_or(false) {
                // 高电平持续时间超过 28μs，说明是 "1"
                byte = (byte << 1) | 1;

                // 等待这个位的高电平结束
                timeout = 1000;
                while self.pin.is_high().unwrap_or(false) {
                    timeout -= 1;
                    if timeout == 0 {
                        return Err(DhtError::Timeout);
                    }
                    delay_us(1);
                }
            } else {
                // 高电平持续时间不足 28μs，说明是 "0"
                byte <<= 1;
            }
        }

        Ok(byte)
    }
}

/// 毫秒级延时
///
/// 这是一个粗略的软件延时，用于 DHT11 通信中不需要精确计时的场景。
/// 基于 400MHz 主频估算：约 8000 个 NOP 指令 ≈ 1ms
fn delay_ms(ms: u32) {
    for _ in 0..ms {
        for _ in 0..8000 {
            cortex_m::asm::nop();
        }
    }
}

/// 微秒级延时
///
/// 用于 DHT11 通信中需要精确计时的场景，比如区分数据位的 0 和 1。
/// 基于 400MHz 主频估算：约 8 个 NOP 指令 ≈ 1μs
///
/// 注意：这是软件延时，实际精度受编译优化和中断影响。
/// 对于 DHT11 这种对时序要求不高的传感器来说足够了。
fn delay_us(us: u32) {
    for _ in 0..us {
        for _ in 0..8 {
            cortex_m::asm::nop();
        }
    }
}
