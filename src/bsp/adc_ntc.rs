// src/adc_ntc.rs
//! NTC 热敏电阻温度传感器驱动模块
//!
//! 使用查表法将 ADC 值转换为温度
//!
//! 注意：此模块为预留功能，暂未在主程序中使用

#![allow(dead_code)]

use stm32h7xx_hal::gpio::{gpioa::PA3, Analog};

/// NTC 温度传感器驱动
pub struct NtcDriver {
    _pin: PA3<Analog>,
}

impl NtcDriver {
    /// 初始化 NTC 驱动
    pub fn new(pin: PA3<Analog>) -> Self {
        Self { _pin: pin }
    }
}
