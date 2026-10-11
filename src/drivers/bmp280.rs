//! BMP280 气压和温度传感器驱动
//!
//! BMP280 是一款高精度气压传感器
//! - 气压范围: 300-1100 hPa
//! - 温度范围: -40°C to 85°C
//! - I2C 地址: 0x76 (SDO=GND) 或 0x77 (SDO=VDDIO)

use embedded_hal::blocking::i2c::{Write, WriteRead};

const BMP280_ADDR_LOW: u8 = 0x76;
const BMP280_ADDR_HIGH: u8 = 0x77;

const REG_CALIBRATION: u8 = 0x88;
const REG_ID: u8 = 0xD0;
const REG_RESET: u8 = 0xE0;
#[allow(dead_code)]
const REG_STATUS: u8 = 0xF3;
const REG_CTRL_MEAS: u8 = 0xF4;
const REG_CONFIG: u8 = 0xF5;
const REG_PRESSURE_MSB: u8 = 0xF7;

const BMP280_ID: u8 = 0x58;

#[allow(dead_code)]
const MODE_SLEEP: u8 = 0x00;
#[allow(dead_code)]
const MODE_FORCED: u8 = 0x01;
const MODE_NORMAL: u8 = 0x03;

#[allow(dead_code)]
const OVERSAMPLING_SKIP: u8 = 0x00;
#[allow(dead_code)]
const OVERSAMPLING_1X: u8 = 0x01;
#[allow(dead_code)]
const OVERSAMPLING_2X: u8 = 0x02;
#[allow(dead_code)]
const OVERSAMPLING_4X: u8 = 0x03;
#[allow(dead_code)]
const OVERSAMPLING_8X: u8 = 0x04;
const OVERSAMPLING_16X: u8 = 0x05;

#[allow(dead_code)]
const FILTER_OFF: u8 = 0x00;
#[allow(dead_code)]
const FILTER_2: u8 = 0x01;
const FILTER_4: u8 = 0x02;
#[allow(dead_code)]
const FILTER_8: u8 = 0x03;
#[allow(dead_code)]
const FILTER_16: u8 = 0x04;

#[allow(dead_code)]
const STANDBY_0_5MS: u8 = 0x00;
#[allow(dead_code)]
const STANDBY_62_5MS: u8 = 0x01;
const STANDBY_125MS: u8 = 0x02;
#[allow(dead_code)]
const STANDBY_250MS: u8 = 0x03;
#[allow(dead_code)]
const STANDBY_500MS: u8 = 0x04;
#[allow(dead_code)]
const STANDBY_1000MS: u8 = 0x05;
#[allow(dead_code)]
const STANDBY_2000MS: u8 = 0x06;
#[allow(dead_code)]
const STANDBY_4000MS: u8 = 0x07;

#[derive(Debug, Clone, Copy)]
pub struct Bmp280Reading {
    #[allow(dead_code)]
    pub temperature: f32,
    pub pressure: f32,
}

#[derive(Debug, Clone, Copy)]
pub enum Bmp280Error {
    I2cError,
    DeviceNotFound,
    #[allow(dead_code)]
    InvalidCalibration,
    #[allow(dead_code)]
    MeasurementNotReady,
}

struct CalibrationData {
    dig_t1: u16,
    dig_t2: i16,
    dig_t3: i16,
    dig_p1: u16,
    dig_p2: i16,
    dig_p3: i16,
    dig_p4: i16,
    dig_p5: i16,
    dig_p6: i16,
    dig_p7: i16,
    dig_p8: i16,
    dig_p9: i16,
}

pub struct Bmp280 {
    addr: u8,
    calibration: CalibrationData,
    t_fine: i32,
    is_initialized: bool,
}

impl Bmp280 {
    pub fn new() -> Self {
        Self {
            addr: 0x76,
            calibration: CalibrationData {
                dig_t1: 0,
                dig_t2: 0,
                dig_t3: 0,
                dig_p1: 0,
                dig_p2: 0,
                dig_p3: 0,
                dig_p4: 0,
                dig_p5: 0,
                dig_p6: 0,
                dig_p7: 0,
                dig_p8: 0,
                dig_p9: 0,
            },
            t_fine: 0,
            is_initialized: false,
        }
    }

    pub fn init<I2C: Write + WriteRead>(
        &mut self,
        i2c: &mut I2C,
        delay_ms: &mut impl FnMut(u32),
    ) -> Result<(), Bmp280Error> {
        self.is_initialized = false;
        if !self.detect_device(i2c)? {
            return Err(Bmp280Error::DeviceNotFound);
        }
        self.soft_reset(i2c)?;
        delay_ms(100);
        self.read_calibration_data(i2c)?;
        if self.calibration.dig_p1 == 0
            || self.calibration.dig_t1 == 0
            || self.calibration.dig_p1 == u16::MAX
            || self.calibration.dig_t1 == u16::MAX
        {
            return Err(Bmp280Error::InvalidCalibration);
        }
        self.configure(i2c)?;
        // Allow the first 16x/16x conversion to complete before exposing data.
        delay_ms(100);
        self.is_initialized = true;
        Ok(())
    }

    fn detect_device<I2C: WriteRead>(&mut self, i2c: &mut I2C) -> Result<bool, Bmp280Error> {
        for addr in [BMP280_ADDR_LOW, BMP280_ADDR_HIGH] {
            let mut buf = [0u8; 1];
            if i2c.write_read(addr, &[REG_ID], &mut buf).is_ok() && buf[0] == BMP280_ID {
                self.addr = addr;
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn soft_reset<I2C: Write>(&mut self, i2c: &mut I2C) -> Result<(), Bmp280Error> {
        i2c.write(self.addr, &[REG_RESET, 0xB6])
            .map_err(|_| Bmp280Error::I2cError)?;

        Ok(())
    }

    fn read_calibration_data<I2C: WriteRead>(&mut self, i2c: &mut I2C) -> Result<(), Bmp280Error> {
        let mut buf = [0u8; 24];
        i2c.write_read(self.addr, &[REG_CALIBRATION], &mut buf)
            .map_err(|_| Bmp280Error::I2cError)?;

        self.calibration.dig_t1 = u16::from_le_bytes([buf[0], buf[1]]);
        self.calibration.dig_t2 = i16::from_le_bytes([buf[2], buf[3]]);
        self.calibration.dig_t3 = i16::from_le_bytes([buf[4], buf[5]]);

        self.calibration.dig_p1 = u16::from_le_bytes([buf[6], buf[7]]);
        self.calibration.dig_p2 = i16::from_le_bytes([buf[8], buf[9]]);
        self.calibration.dig_p3 = i16::from_le_bytes([buf[10], buf[11]]);
        self.calibration.dig_p4 = i16::from_le_bytes([buf[12], buf[13]]);
        self.calibration.dig_p5 = i16::from_le_bytes([buf[14], buf[15]]);
        self.calibration.dig_p6 = i16::from_le_bytes([buf[16], buf[17]]);
        self.calibration.dig_p7 = i16::from_le_bytes([buf[18], buf[19]]);
        self.calibration.dig_p8 = i16::from_le_bytes([buf[20], buf[21]]);
        self.calibration.dig_p9 = i16::from_le_bytes([buf[22], buf[23]]);

        Ok(())
    }

    fn configure<I2C: Write>(&mut self, i2c: &mut I2C) -> Result<(), Bmp280Error> {
        let config = (STANDBY_125MS << 5) | (FILTER_4 << 2);
        i2c.write(self.addr, &[REG_CONFIG, config])
            .map_err(|_| Bmp280Error::I2cError)?;

        let ctrl_meas = (OVERSAMPLING_16X << 5) | (OVERSAMPLING_16X << 2) | MODE_NORMAL;
        i2c.write(self.addr, &[REG_CTRL_MEAS, ctrl_meas])
            .map_err(|_| Bmp280Error::I2cError)?;

        Ok(())
    }

    pub fn read<I2C: WriteRead>(&mut self, i2c: &mut I2C) -> Result<Bmp280Reading, Bmp280Error> {
        if !self.is_initialized {
            return Err(Bmp280Error::DeviceNotFound);
        }

        let mut buf = [0u8; 6];
        i2c.write_read(self.addr, &[REG_PRESSURE_MSB], &mut buf)
            .map_err(|_| Bmp280Error::I2cError)?;

        let pressure_raw =
            ((buf[0] as u32) << 12) | ((buf[1] as u32) << 4) | ((buf[2] as u32) >> 4);
        let temperature_raw =
            ((buf[3] as u32) << 12) | ((buf[4] as u32) << 4) | ((buf[5] as u32) >> 4);

        if pressure_raw == 0x80000 || temperature_raw == 0x80000 {
            return Err(Bmp280Error::MeasurementNotReady);
        }
        let temperature = self.compensate_temperature(temperature_raw);
        let pressure = self.compensate_pressure(pressure_raw);

        Ok(Bmp280Reading {
            temperature,
            pressure,
        })
    }

    fn compensate_temperature(&mut self, raw: u32) -> f32 {
        let var1 = ((((raw as i32) >> 3) - ((self.calibration.dig_t1 as i32) << 1))
            * (self.calibration.dig_t2 as i32))
            >> 11;

        let var2 = ((((((raw as i32) >> 4) - (self.calibration.dig_t1 as i32))
            * (((raw as i32) >> 4) - (self.calibration.dig_t1 as i32)))
            >> 12)
            * (self.calibration.dig_t3 as i32))
            >> 14;

        self.t_fine = var1 + var2;

        ((self.t_fine * 5 + 128) >> 8) as f32 / 100.0
    }

    fn compensate_pressure(&self, raw: u32) -> f32 {
        let var1 = (self.t_fine as f32 / 2.0) - 64000.0;
        let var2 = var1 * var1 * (self.calibration.dig_p6 as f32) / 32768.0;
        let var2 = var2 + var1 * (self.calibration.dig_p5 as f32) * 2.0;
        let var2 = (var2 / 4.0) + ((self.calibration.dig_p4 as f32) * 65536.0);
        let var1 = ((self.calibration.dig_p3 as f32) * var1 * var1 / 524288.0
            + (self.calibration.dig_p2 as f32) * var1)
            / 524288.0;
        let var1 = (1.0 + var1 / 32768.0) * (self.calibration.dig_p1 as f32);

        if var1 == 0.0 {
            return 0.0;
        }

        let pressure = 1048576.0 - (raw as f32);
        let pressure = (pressure - (var2 / 4096.0)) * 6250.0 / var1;
        let var1 = (self.calibration.dig_p9 as f32) * pressure * pressure / 2147483648.0;
        let var2 = pressure * (self.calibration.dig_p8 as f32) / 32768.0;

        let pressure = pressure + (var1 + var2 + (self.calibration.dig_p7 as f32)) / 16.0;

        pressure / 100.0
    }
}

impl Default for Bmp280 {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // BMP280 补偿示例：温度 25.08°C，气压约 1006.53hPa。
    struct SensorBus {
        bmp_present: bool,
        calibration: [u8; 24],
        measurement: [u8; 6],
    }
    impl SensorBus {
        fn new() -> Self {
            let coefficients: [i32; 12] = [
                27504, 26435, -1000, 36477, -10685, 3024, 2855, 140, -7, 15500, -14600, 6000,
            ];
            let mut calibration = [0; 24];
            for (i, value) in coefficients.iter().enumerate() {
                calibration[i * 2..i * 2 + 2].copy_from_slice(&(*value as u16).to_le_bytes());
            }
            let pressure: u32 = 415148;
            let temperature: u32 = 519888;
            Self {
                bmp_present: true,
                calibration,
                measurement: [
                    (pressure >> 12) as u8,
                    (pressure >> 4) as u8,
                    (pressure << 4) as u8,
                    (temperature >> 12) as u8,
                    (temperature >> 4) as u8,
                    (temperature << 4) as u8,
                ],
            }
        }
    }
    impl Write for SensorBus {
        type Error = ();
        fn write(&mut self, addr: u8, _: &[u8]) -> Result<(), ()> {
            if addr == 0x76 && self.bmp_present {
                Ok(())
            } else {
                Err(())
            }
        }
    }
    impl WriteRead for SensorBus {
        type Error = ();
        fn write_read(&mut self, addr: u8, bytes: &[u8], buffer: &mut [u8]) -> Result<(), ()> {
            if addr != 0x76 || !self.bmp_present {
                return Err(());
            }
            match bytes {
                [0xd0] => buffer[0] = 0x58,
                [0x88] => buffer.copy_from_slice(&self.calibration),
                [0xf7] => buffer.copy_from_slice(&self.measurement),
                _ => return Err(()),
            }
            Ok(())
        }
    }

    #[test]
    fn bmp_missing_device_is_an_initialization_error_and_can_recover() {
        let mut bus = SensorBus::new();
        let mut bmp = Bmp280::new();
        bus.bmp_present = false;
        assert!(matches!(
            bmp.init(&mut bus, &mut |_| {}),
            Err(Bmp280Error::DeviceNotFound)
        ));
        bus.bmp_present = true;
        bmp.init(&mut bus, &mut |_| {}).unwrap();
        let reading = bmp.read(&mut bus).unwrap();
        assert!((reading.temperature - 25.08).abs() < 0.01);
        assert!((reading.pressure - 1006.53).abs() < 0.1);
        bus.bmp_present = false;
        assert!(bmp.init(&mut bus, &mut |_| {}).is_err());
        assert!(matches!(
            bmp.read(&mut bus),
            Err(Bmp280Error::DeviceNotFound)
        ));
    }

    #[test]
    fn bmp_rejects_invalid_calibration_and_reset_measurements() {
        let mut bus = SensorBus::new();
        bus.calibration[6..8].fill(0);
        let mut bmp = Bmp280::new();
        assert!(matches!(
            bmp.init(&mut bus, &mut |_| {}),
            Err(Bmp280Error::InvalidCalibration)
        ));
        let mut bus = SensorBus::new();
        bmp.init(&mut bus, &mut |_| {}).unwrap();
        bus.measurement = [0x80, 0, 0, 0x80, 0, 0];
        assert!(matches!(
            bmp.read(&mut bus),
            Err(Bmp280Error::MeasurementNotReady)
        ));
    }
}
