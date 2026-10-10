//! # 硬件驱动模块
//!
//! 本模块包含所有硬件驱动程序，包括传感器、显示器和通信接口。
//!
//! ## 模块列表
//!
//! - [`aht20`] - AHT20 温湿度传感器驱动
//! - [`bmp280`] - BMP280 气压和温度传感器驱动
//! - [`dht11`] - DHT11 温湿度传感器驱动
//! - [`adc_ntc`] - ADC NTC 热敏电阻驱动
//! - [`display`] - ILI9341 TFT 显示屏驱动
//! - [`serial`] - 串口通信驱动
//! - [`esp8266`] - ESP8266 UART 协处理器驱动

pub mod adc_ntc;
pub use stm32h750_drivers::{aht20, bmp280};
pub mod bounded_i2c;
pub mod dht11;
pub mod display;
pub mod esp8266;
pub mod serial;

// 重新导出常用类型
#[allow(unused_imports)]
pub use aht20::{Aht20, Aht20Error, Aht20Reading};
#[allow(unused_imports)]
pub use bmp280::{Bmp280, Bmp280Error, Bmp280Reading};
#[allow(unused_imports)]
pub use dht11::{Dht11, DhtError, DhtReading};
pub use display::{DisplayDriver, DisplayOrientation};
#[allow(unused_imports)]
pub use esp8266::{Esp8266, Esp8266Error, HttpEndpoint, SensorPayload, WifiStatus};
#[allow(unused_imports)]
pub use serial::SerialTx;
