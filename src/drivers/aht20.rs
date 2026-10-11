//! AHT20 温湿度传感器驱动
//!
//! AHT20 是一款高精度 I2C 温湿度传感器
//! - 温度精度: ±0.3°C
//! - 湿度精度: ±2% RH
//! - I2C 地址: 0x38

use embedded_hal::blocking::i2c::{Read, Write, WriteRead};

const AHT20_ADDR: u8 = 0x38;

const CMD_INITIALIZE: u8 = 0xBE;
const CMD_TRIGGER_MEASUREMENT: u8 = 0xAC;
const CMD_SOFT_RESET: u8 = 0xBA;
const CMD_STATUS: u8 = 0x71;

const STATUS_BUSY_MASK: u8 = 0x80;
const STATUS_CALIBRATED_MASK: u8 = 0x08;

#[derive(Debug, Clone, Copy)]
pub struct Aht20Reading {
    pub temperature: f32,
    pub humidity: f32,
}

#[derive(Debug, Clone, Copy)]
pub enum Aht20Error {
    I2cError,
    NotCalibrated,
    Busy,
    InvalidData,
}

/// The bus is borrowed per operation, so another sensor can use it without
/// destroying either driver's state.
#[derive(Default)]
pub struct Aht20;

impl Aht20 {
    pub fn new() -> Self {
        Self
    }

    pub fn init<I2C: Write + WriteRead>(
        &mut self,
        i2c: &mut I2C,
        delay_ms: &mut impl FnMut(u32),
    ) -> Result<(), Aht20Error> {
        i2c.write(AHT20_ADDR, &[CMD_SOFT_RESET])
            .map_err(|_| Aht20Error::I2cError)?;
        delay_ms(40);
        if Self::read_status(i2c)? & STATUS_CALIBRATED_MASK == 0 {
            i2c.write(AHT20_ADDR, &[CMD_INITIALIZE, 0x08, 0x00])
                .map_err(|_| Aht20Error::I2cError)?;
            delay_ms(10);
            if Self::read_status(i2c)? & STATUS_CALIBRATED_MASK == 0 {
                return Err(Aht20Error::NotCalibrated);
            }
        }
        Ok(())
    }

    fn read_status<I2C: WriteRead>(i2c: &mut I2C) -> Result<u8, Aht20Error> {
        let mut buf = [0u8; 1];
        i2c.write_read(AHT20_ADDR, &[CMD_STATUS], &mut buf)
            .map_err(|_| Aht20Error::I2cError)?;
        Ok(buf[0])
    }

    pub fn read<I2C: Write + WriteRead + Read>(
        &mut self,
        i2c: &mut I2C,
        delay_ms: &mut impl FnMut(u32),
    ) -> Result<Aht20Reading, Aht20Error> {
        i2c.write(AHT20_ADDR, &[CMD_TRIGGER_MEASUREMENT, 0x33, 0x00])
            .map_err(|_| Aht20Error::I2cError)?;
        delay_ms(80);
        for _ in 0..100 {
            if Self::read_status(i2c)? & STATUS_BUSY_MASK == 0 {
                let mut buf = [0u8; 7];
                // AHT20 measurements are read directly, without a register write.
                i2c.read(AHT20_ADDR, &mut buf)
                    .map_err(|_| Aht20Error::I2cError)?;
                if buf[0] & STATUS_BUSY_MASK != 0 {
                    return Err(Aht20Error::Busy);
                }
                if buf[0] & STATUS_CALIBRATED_MASK == 0 {
                    return Err(Aht20Error::NotCalibrated);
                }
                let humidity_raw =
                    ((buf[1] as u32) << 12) | ((buf[2] as u32) << 4) | ((buf[3] as u32) >> 4);
                let temperature_raw =
                    (((buf[3] & 0x0F) as u32) << 16) | ((buf[4] as u32) << 8) | (buf[5] as u32);
                let humidity = humidity_raw as f32 * (100.0 / 1048576.0);
                let temperature = temperature_raw as f32 * (200.0 / 1048576.0) - 50.0;
                if !(0.0..=100.0).contains(&humidity) || !(-50.0..=100.0).contains(&temperature) {
                    return Err(Aht20Error::InvalidData);
                }
                return Ok(Aht20Reading {
                    temperature,
                    humidity,
                });
            }
            delay_ms(1);
        }
        Err(Aht20Error::Busy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 这里只需模拟状态寄存器；忙状态超时前不应读取测量帧。
    struct SensorBus {
        calibrated: bool,
        busy: bool,
    }
    impl SensorBus {
        fn new() -> Self {
            Self {
                calibrated: true,
                busy: false,
            }
        }
    }
    impl Write for SensorBus {
        type Error = ();
        fn write(&mut self, addr: u8, _: &[u8]) -> Result<(), ()> {
            assert_eq!(addr, 0x38);
            Ok(())
        }
    }
    impl WriteRead for SensorBus {
        type Error = ();
        fn write_read(&mut self, addr: u8, bytes: &[u8], buffer: &mut [u8]) -> Result<(), ()> {
            assert_eq!(addr, 0x38);
            assert_eq!(bytes, &[0x71]);
            buffer[0] = if self.calibrated { 8 } else { 0 } | if self.busy { 0x80 } else { 0 };
            Ok(())
        }
    }
    impl Read for SensorBus {
        type Error = ();
        fn read(&mut self, _: u8, _: &mut [u8]) -> Result<(), ()> {
            panic!("忙状态超时不应读取测量帧");
        }
    }

    #[test]
    fn aht_checks_calibration_and_bounds_busy_polling() {
        let mut bus = SensorBus::new();
        bus.calibrated = false;
        let mut sensor = Aht20::new();
        assert!(matches!(
            sensor.init(&mut bus, &mut |_| {}),
            Err(Aht20Error::NotCalibrated)
        ));
        bus.calibrated = true;
        bus.busy = true;
        let mut waited_ms = 0;
        assert!(matches!(
            sensor.read(&mut bus, &mut |ms| waited_ms += ms),
            Err(Aht20Error::Busy)
        ));
        assert_eq!(waited_ms, 180);
    }
}
