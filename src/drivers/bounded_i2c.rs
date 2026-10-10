//! Bounded I2C1 transactions. HAL 0.16's blocking methods have no deadline.
use crate::clock::Clock;
use embedded_hal::blocking::i2c::{Read, Write, WriteRead};
use stm32h7xx_hal::{
    i2c::I2c,
    pac::{i2c1, I2C1},
};

#[derive(Debug, Clone, Copy)]
pub enum BusError {
    Timeout,
    Nack,
    Bus,
    Arbitration,
    InvalidTransfer,
}

pub struct BoundedI2c<'a> {
    bus: I2c<I2C1>,
    clock: &'a Clock,
}

impl<'a> BoundedI2c<'a> {
    const TIMEOUT_US: u32 = 10_000;

    pub fn new(bus: I2c<I2C1>, clock: &'a Clock) -> Self {
        Self { bus, clock }
    }

    fn wait(&self, start: u32, ready: impl Fn(&i2c1::isr::R) -> bool) -> Result<(), BusError> {
        loop {
            let status = self.bus.inner().isr.read();
            if status.berr().bit_is_set() || status.ovr().bit_is_set() {
                return Err(BusError::Bus);
            }
            if status.arlo().bit_is_set() {
                return Err(BusError::Arbitration);
            }
            if status.nackf().bit_is_set() {
                return Err(BusError::Nack);
            }
            if ready(&status) {
                return Ok(());
            }
            if self.clock.expired(start, Self::TIMEOUT_US) {
                return Err(BusError::Timeout);
            }
        }
    }

    fn begin(
        &self,
        addr: u8,
        length: usize,
        read: bool,
        autoend: bool,
        start: u32,
    ) -> Result<(), BusError> {
        if addr > 0x7f || length == 0 || length > 255 {
            return Err(BusError::InvalidTransfer);
        }
        while self.bus.inner().cr2.read().start().bit_is_set() {
            if self.clock.expired(start, Self::TIMEOUT_US) {
                return Err(BusError::Timeout);
            }
        }
        self.bus.inner().cr2.write(|w| {
            w.sadd()
                .bits(u16::from(addr) << 1)
                .rd_wrn()
                .bit(read)
                .nbytes()
                .bits(length as u8)
                .autoend()
                .bit(autoend)
                .start()
                .set_bit()
        });
        Ok(())
    }

    fn finish(&self, result: Result<(), BusError>) -> Result<(), BusError> {
        let regs = self.bus.inner();
        if result.is_err() {
            // Request STOP only while we still own the bus, then bound its wait.
            if !regs.isr.read().arlo().bit_is_set() {
                regs.cr2.modify(|_, w| w.stop().set_bit());
                let start = self.clock.now_us();
                while regs.isr.read().busy().bit_is_set() && !self.clock.expired(start, 1_000) {}
            }
            // Reset the controller state while retaining HAL's timing/filter setup.
            regs.cr1.modify(|_, w| w.pe().clear_bit());
            regs.cr2.reset();
        }
        regs.icr.write(|w| {
            w.stopcf()
                .set_bit()
                .nackcf()
                .set_bit()
                .berrcf()
                .set_bit()
                .arlocf()
                .set_bit()
                .ovrcf()
                .set_bit()
                .timoutcf()
                .set_bit()
        });
        if result.is_err() {
            regs.isr.write(|w| w.txe().set_bit());
            regs.cr1.modify(|_, w| w.pe().set_bit());
        }
        result
    }

    fn transfer(&mut self, addr: u8, bytes: &[u8], buffer: &mut [u8]) -> Result<(), BusError> {
        if bytes.is_empty() && buffer.is_empty() {
            return Ok(());
        }
        let start = self.clock.now_us();
        let result = (|| {
            self.wait(start, |s| s.busy().bit_is_clear())?;
            if !bytes.is_empty() {
                self.begin(addr, bytes.len(), false, buffer.is_empty(), start)?;
                for &byte in bytes {
                    self.wait(start, |s| s.txis().bit_is_set())?;
                    self.bus.inner().txdr.write(|w| w.txdata().bits(byte));
                }
                if !buffer.is_empty() {
                    self.wait(start, |s| s.tc().bit_is_set())?;
                }
            }
            if !buffer.is_empty() {
                self.begin(addr, buffer.len(), true, true, start)?;
                for byte in buffer {
                    self.wait(start, |s| s.rxne().bit_is_set())?;
                    *byte = self.bus.inner().rxdr.read().rxdata().bits();
                }
            }
            self.wait(start, |s| s.stopf().bit_is_set())?;
            self.wait(start, |s| s.busy().bit_is_clear())
        })();
        self.finish(result)
    }
}

impl Write for BoundedI2c<'_> {
    type Error = BusError;
    fn write(&mut self, addr: u8, bytes: &[u8]) -> Result<(), Self::Error> {
        self.transfer(addr, bytes, &mut [])
    }
}
impl Read for BoundedI2c<'_> {
    type Error = BusError;
    fn read(&mut self, addr: u8, buffer: &mut [u8]) -> Result<(), Self::Error> {
        self.transfer(addr, &[], buffer)
    }
}
impl WriteRead for BoundedI2c<'_> {
    type Error = BusError;
    fn write_read(&mut self, addr: u8, bytes: &[u8], buffer: &mut [u8]) -> Result<(), Self::Error> {
        self.transfer(addr, bytes, buffer)
    }
}
