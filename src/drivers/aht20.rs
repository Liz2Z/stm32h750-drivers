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

pub struct Aht20<I2C> {
    i2c: I2C,
}

impl<I2C, E> Aht20<I2C>
where
    I2C: Write<Error = E> + WriteRead<Error = E> + Read<Error = E>,
{
    pub fn new(i2c: I2C) -> Self {
        Self { i2c }
    }

    pub fn init(&mut self) -> Result<(), Aht20Error> {
        self.soft_reset()?;

        self.delay_ms(20);

        self.check_calibration()
    }

    fn soft_reset(&mut self) -> Result<(), Aht20Error> {
        self.i2c
            .write(AHT20_ADDR, &[CMD_SOFT_RESET])
            .map_err(|_| Aht20Error::I2cError)?;

        self.delay_ms(20);

        Ok(())
    }

    fn check_calibration(&mut self) -> Result<(), Aht20Error> {
        let status = self.read_status()?;

        if status & STATUS_CALIBRATED_MASK == 0 {
            self.initialize()?;
        }

        Ok(())
    }

    fn initialize(&mut self) -> Result<(), Aht20Error> {
        self.i2c
            .write(AHT20_ADDR, &[CMD_INITIALIZE, 0x08, 0x00])
            .map_err(|_| Aht20Error::I2cError)?;

        self.delay_ms(10);

        Ok(())
    }

    fn read_status(&mut self) -> Result<u8, Aht20Error> {
        let mut buf = [0u8; 1];
        self.i2c
            .write_read(AHT20_ADDR, &[CMD_STATUS], &mut buf)
            .map_err(|_| Aht20Error::I2cError)?;

        Ok(buf[0])
    }

    pub fn read(&mut self) -> Result<Aht20Reading, Aht20Error> {
        self.trigger_measurement()?;

        self.delay_ms(80);

        self.wait_for_completion()?;

        self.read_data()
    }

    fn trigger_measurement(&mut self) -> Result<(), Aht20Error> {
        self.i2c
            .write(AHT20_ADDR, &[CMD_TRIGGER_MEASUREMENT, 0x33, 0x00])
            .map_err(|_| Aht20Error::I2cError)?;

        Ok(())
    }

    fn wait_for_completion(&mut self) -> Result<(), Aht20Error> {
        let mut attempts = 0;
        loop {
            let status = self.read_status()?;

            if status & STATUS_BUSY_MASK == 0 {
                return Ok(());
            }

            attempts += 1;
            if attempts > 100 {
                return Err(Aht20Error::Busy);
            }

            self.delay_ms(1);
        }
    }

    fn read_data(&mut self) -> Result<Aht20Reading, Aht20Error> {
        let mut buf = [0u8; 7];
        // 直接读取数据，无需发送寄存器地址
        // AHT20 在触发测量后会自动将数据放在输出缓冲区
        self.i2c
            .read(AHT20_ADDR, &mut buf)
            .map_err(|_| Aht20Error::I2cError)?;

        let humidity_raw = ((buf[1] as u32) << 12)
            | ((buf[2] as u32) << 4)
            | ((buf[3] as u32) >> 4);

        let temperature_raw = (((buf[3] & 0x0F) as u32) << 16)
            | ((buf[4] as u32) << 8)
            | (buf[5] as u32);

        let humidity = (humidity_raw as f32 / 1048576.0) * 100.0;
        let temperature = (temperature_raw as f32 / 1048576.0) * 200.0 - 50.0;

        if humidity > 100.0 || humidity < 0.0 || temperature > 100.0 || temperature < -50.0 {
            return Err(Aht20Error::InvalidData);
        }

        Ok(Aht20Reading {
            temperature,
            humidity,
        })
    }

    fn delay_ms(&mut self, ms: u32) {
        // STM32H750 主频 400MHz，1ms = 400,000 个 CPU 周期
        cortex_m::asm::delay(ms * 400_000);
    }

    pub fn release(self) -> I2C {
        self.i2c
    }
}
