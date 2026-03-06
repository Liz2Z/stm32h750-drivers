//! AHT20 温湿度传感器示例
//!
//! 本示例展示如何使用 I2C 接口读取 AHT20 温湿度传感器
//!
//! ## 硬件连接
//!
//! | AHT20 引脚 | STM32H750 引脚 | 说明 |
//! |-----------|---------------|------|
//! | VDD | 3.3V | 电源（必须 3.3V）|
//! | GND | GND | 地线 |
//! | SDA | PB7 | I2C1 数据线（需要 4.7kΩ 上拉电阻）|
//! | SCL | PB6 | I2C1 时钟线（需要 4.7kΩ 上拉电阻）|
//!
//! ## 重要提示
//!
//! 1. I2C 总线必须接上拉电阻（4.7kΩ ~ 10kΩ）
//! 2. 如果使用模块，通常模块上已经自带上拉电阻
//! 3. STM32H750 主频 400MHz，延时函数需要精确计算周期数

#![no_main]
#![no_std]

use cortex_m_rt::entry;
use panic_halt as _;
use stm32h7xx_hal::{pac, prelude::*};

// 从 src 目录引用模块
#[path = "../src/drivers/aht20.rs"]
mod aht20;

use aht20::Aht20;

/// 精确延时函数
///
/// STM32H750 主频 400MHz，1ms = 400,000 个 CPU 周期
/// 使用 cortex_m::asm::delay 精确消耗周期数
fn delay_ms(ms: u32) {
    cortex_m::asm::delay(ms * 400_000);
}

#[entry]
fn main() -> ! {
    // ============================================================
    // 系统初始化
    // ============================================================
    
    let dp = pac::Peripherals::take().unwrap();

    // 配置电源和时钟
    let pwr = dp.PWR.constrain();
    let pwrcfg = pwr.vos1().freeze(); // VOS1 = 最高性能模式

    let rcc = dp.RCC.constrain();
    let ccdr = rcc
        .sys_ck(400.MHz())
        .freeze(pwrcfg, &dp.SYSCFG);

    // ============================================================
    // I2C 配置
    // ============================================================
    
    let gpiob = dp.GPIOB.split(ccdr.peripheral.GPIOB);

    // I2C1 引脚配置
    // PB6 = I2C1_SCL, PB7 = I2C1_SDA
    // 必须设置为开漏输出，并确保硬件上有上拉电阻
    let i2c_scl = gpiob
        .pb6
        .into_alternate::<4>()
        .set_open_drain();
    let i2c_sda = gpiob
        .pb7
        .into_alternate::<4>()
        .set_open_drain();

    // 初始化 I2C1 (100kHz)
    let i2c = dp.I2C1.i2c(
        (i2c_scl, i2c_sda),
        100.kHz(),
        ccdr.peripheral.I2C1,
        &ccdr.clocks,
    );

    // ============================================================
    // AHT20 初始化
    // ============================================================
    
    let mut aht20 = Aht20::new(i2c);

    // 初始化传感器
    match aht20.init() {
        Ok(_) => {
            // 初始化成功
            // 可以在这里添加 LED 指示或其他反馈
        }
        Err(e) => {
            // 初始化失败，根据错误类型处理
            match e {
                aht20::Aht20Error::I2cError => {
                    // I2C 通信错误，检查硬件连接和上拉电阻
                }
                aht20::Aht20Error::NotCalibrated => {
                    // 传感器未校准
                }
                aht20::Aht20Error::Busy => {
                    // 传感器忙
                }
                aht20::Aht20Error::InvalidData => {
                    // 无效数据
                }
            }
            
            // 初始化失败，进入死循环
            loop {
                delay_ms(1000);
            }
        }
    }

    // ============================================================
    // 主循环 - 读取传感器数据
    // ============================================================
    
    loop {
        match aht20.read() {
            Ok(reading) => {
                // 读取成功
                let temperature = reading.temperature;
                let humidity = reading.humidity;
                
                // 在这里可以处理数据：
                // - 通过串口发送
                // - 显示在屏幕上
                // - 存储到缓冲区
                // - 触发其他操作
                
                // 示例：简单使用变量（避免编译器警告）
                let _ = temperature;
                let _ = humidity;
            }
            Err(e) => {
                // 读取失败，根据错误类型处理
                match e {
                    aht20::Aht20Error::I2cError => {
                        // I2C 通信错误
                    }
                    aht20::Aht20Error::NotCalibrated => {
                        // 传感器未校准
                    }
                    aht20::Aht20Error::Busy => {
                        // 传感器忙，可能延时不够
                    }
                    aht20::Aht20Error::InvalidData => {
                        // 无效数据，可能是传感器故障
                    }
                }
            }
        }

        // 每 2 秒读取一次
        // AHT20 测量时间约 80ms，建议间隔 > 1 秒
        delay_ms(2000);
    }
}
