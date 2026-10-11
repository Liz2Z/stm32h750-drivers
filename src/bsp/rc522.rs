//! RC522 板级连接：SPI1、PA4 片选、PA3 复位，独占资源、不使用 DMA。
use super::clock::Clock;
use crate::drivers::rc522::Interface;
use embedded_hal::spi::FullDuplex;
use stm32h7xx_hal::{
    gpio::{Output, PushPull, PA3, PA4},
    pac::SPI1,
    spi::{Enabled, HalSpi, Spi},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Timeout,
    Spi(stm32h7xx_hal::spi::Error),
}

pub struct Rc522Spi<'a> {
    spi: Spi<SPI1, Enabled, u8>,
    cs: PA4<Output<PushPull>>,
    reset: PA3<Output<PushPull>>,
    clock: &'a Clock,
}

impl<'a> Rc522Spi<'a> {
    pub fn new(
        spi: Spi<SPI1, Enabled, u8>,
        mut cs: PA4<Output<PushPull>>,
        mut reset: PA3<Output<PushPull>>,
        clock: &'a Clock,
    ) -> Self {
        cs.set_high();
        reset.set_high();
        Self {
            spi,
            cs,
            reset,
            clock,
        }
    }

    fn transfer(&mut self, bytes: &mut [u8; 2]) -> Result<(), Error> {
        let start = self.clock.now_us();
        for byte in bytes {
            loop {
                match self.spi.send(*byte) {
                    Ok(()) => break,
                    Err(nb::Error::Other(error)) => return Err(Error::Spi(error)),
                    Err(nb::Error::WouldBlock) => {
                        if self.clock.expired(start, 1_000) {
                            return Err(Error::Timeout);
                        }
                    },
                }
            }
            loop {
                match self.spi.read() {
                    Ok(value) => {
                        *byte = value;
                        break;
                    },
                    Err(nb::Error::Other(error)) => return Err(Error::Spi(error)),
                    Err(nb::Error::WouldBlock) => {
                        if self.clock.expired(start, 1_000) {
                            return Err(Error::Timeout);
                        }
                    },
                }
            }
        }
        // RX 到达后仍确认最后一位已离开线路，才允许释放 CS。
        while self.spi.inner().sr.read().txc().bit_is_clear() {
            if self.clock.expired(start, 1_000) {
                return Err(Error::Timeout);
            }
        }
        Ok(())
    }
}

impl Interface for Rc522Spi<'_> {
    type Error = Error;

    fn reset(&mut self) -> Result<(), Error> {
        self.cs.set_high();
        self.reset.set_low();
        self.clock.delay_ms(2);
        self.reset.set_high();
        self.clock.delay_ms(50);
        Ok(())
    }

    fn exchange(&mut self, bytes: &mut [u8; 2]) -> Result<(), Error> {
        self.cs.set_low();
        let result = self.transfer(bytes);
        self.cs.set_high();
        if result.is_err() {
            // 关闭 SPI1 清空 FIFO，清故障后启用；不影响 SPI2 或 DMA1。
            let regs = self.spi.inner();
            regs.cr1.write(|w| w.ssi().set_bit());
            regs.cr2.reset();
            regs.ifcr.write(|w| {
                w.ovrc()
                    .set_bit()
                    .modfc()
                    .set_bit()
                    .eotc()
                    .set_bit()
                    .txtfc()
                    .set_bit()
                    .udrc()
                    .set_bit()
                    .crcec()
                    .set_bit()
                    .suspc()
                    .set_bit()
            });
            regs.cr1.write(|w| w.ssi().set_bit().spe().set_bit());
        }
        result
    }
}
