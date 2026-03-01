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
