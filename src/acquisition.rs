use crate::{
    aht20::{Aht20, Aht20Error, Aht20Reading},
    bmp280::{Bmp280, Bmp280Error, Bmp280Reading},
};
use embedded_hal::blocking::i2c::{Read, Write, WriteRead};

/// Independent sensor lifecycles on a single borrowed bus.
#[derive(Default)]
pub struct Sensors {
    aht20: Aht20,
    bmp280: Bmp280,
    aht20_ready: bool,
    bmp280_ready: bool,
}

pub struct Readings {
    pub climate: Result<Aht20Reading, Aht20Error>,
    pub pressure: Result<Bmp280Reading, Bmp280Error>,
}

impl Sensors {
    pub fn sample<I2C: Read + Write + WriteRead>(
        &mut self,
        bus: &mut I2C,
        delay_ms: &mut impl FnMut(u32),
    ) -> Readings {
        let climate = (|| {
            if !self.aht20_ready {
                self.aht20.init(bus, delay_ms)?;
            }
            self.aht20.read(bus, delay_ms)
        })();
        self.aht20_ready = climate.is_ok();
        // Never gate pressure acquisition on the climate result.
        let pressure = (|| {
            if !self.bmp280_ready {
                self.bmp280.init(bus, delay_ms)?;
            }
            self.bmp280.read(bus)
        })();
        self.bmp280_ready = pressure.is_ok();
        Readings { climate, pressure }
    }
}
