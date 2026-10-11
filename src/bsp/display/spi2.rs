//! STM32H750 SPI2/CS/DC adapter for the generic ILI9341 driver.
use super::{dma::DisplayDma, DisplayError, DisplaySpi};
use crate::{
    bsp::clock::Clock,
    drivers::display::{spi_transfer, DisplayInterface},
};
use stm32h7xx_hal::{
    gpio::{Output, PushPull, PB1, PB12},
    pac::DMA1,
    rcc::rec::Dma1,
    spi::HalSpi,
};

pub struct Spi2Display<'a> {
    spi: DisplaySpi,
    clock: &'a Clock,
    dma: DisplayDma<'a>,
    cs: PB12<Output<PushPull>>,
    dc: PB1<Output<PushPull>>,
}

impl<'a> Spi2Display<'a> {
    pub(super) fn new(
        spi: DisplaySpi,
        mut cs: PB12<Output<PushPull>>,
        mut dc: PB1<Output<PushPull>>,
        clock: &'a Clock,
        dma: DMA1,
        prec: Dma1,
    ) -> Self {
        cs.set_high();
        dc.set_high();
        Self {
            spi,
            clock,
            dma: DisplayDma::new(dma, prec, clock),
            cs,
            dc,
        }
    }
    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), DisplayError> {
        let start = self.clock.now_us();
        spi_transfer::write_bytes(&mut self.spi, || self.clock.now_us(), bytes, 100_000)?;
        // CS and DC may change only after the last bit has left the wire.
        while self.spi.inner().sr.read().txc().bit_is_clear() {
            if self.clock.expired(start, 100_000) {
                return Err(DisplayError::Timeout);
            }
        }
        Ok(())
    }

    /// All transaction exits release CS, including errors.
    fn finish(&mut self, result: Result<(), DisplayError>) -> Result<(), DisplayError> {
        self.cs.set_high();
        if result.is_err() {
            self.dma.abort();
            // Disable flushes FIFOs; always return to the command word size.
            self.spi.inner().cr1.write(|w| w.ssi().set_bit());
            self.spi
                .inner()
                .cfg1
                .modify(|_, w| w.dsize().bits(7).txdmaen().clear_bit());
            self.spi.inner().cr2.reset();
            self.spi.inner().ifcr.write(|w| {
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
            self.spi
                .inner()
                .cr1
                .write(|w| w.ssi().set_bit().spe().set_bit());
        }
        result
    }

    fn write_command(&mut self, cmd: u8) -> Result<(), DisplayError> {
        self.cs.set_low();
        self.dc.set_low();
        let result = self.write_bytes(&[cmd]);
        self.finish(result)
    }

    fn write_cmd_data(&mut self, cmd: u8, data: u8) -> Result<(), DisplayError> {
        self.cs.set_low();
        self.dc.set_low();
        let result = (|| {
            self.write_bytes(&[cmd])?;
            self.dc.set_high();
            self.write_bytes(&[data])
        })();
        self.finish(result)
    }

    fn write_data_bytes(&mut self, bytes: &[u8]) -> Result<(), DisplayError> {
        self.cs.set_low();
        self.dc.set_high();
        let result = self.write_bytes(bytes);
        self.finish(result)
    }

    /// Change only while idle: commands use 8 bits, DMA pixels use 16 bits.
    fn pixel_mode(&mut self, enabled: bool) -> Result<(), DisplayError> {
        let regs = self.spi.inner();
        if regs.cr1.read().cstart().bit_is_set() {
            regs.cr1.modify(|_, w| w.csusp().set_bit());
            let start = self.clock.now_us();
            while regs.sr.read().susp().bit_is_clear() {
                if self.clock.expired(start, 1_000) {
                    return Err(DisplayError::Timeout);
                }
            }
        }
        regs.cr1.write(|w| w.ssi().set_bit());
        regs.ifcr
            .write(|w| w.suspc().set_bit().eotc().set_bit().txtfc().set_bit());
        regs.cfg1.modify(|_, w| {
            w.dsize()
                .bits(if enabled { 15 } else { 7 })
                .txdmaen()
                .bit(enabled)
        });
        regs.cr2.reset(); // unlimited-length master transaction
        regs.cr1.write(|w| w.ssi().set_bit().spe().set_bit());
        Ok(())
    }
}
impl DisplayInterface for Spi2Display<'_> {
    type Error = DisplayError;
    fn write_command(&mut self, command: u8) -> Result<(), Self::Error> {
        self.write_command(command)
    }
    fn write_data(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        self.write_data_bytes(data)
    }
    fn write_cmd_data(&mut self, command: u8, data: u8) -> Result<(), Self::Error> {
        self.write_cmd_data(command, data)
    }
    fn begin_pixels(&mut self, command: u8) -> Result<(), Self::Error> {
        self.cs.set_low();
        self.dc.set_low();
        let result = (|| {
            self.write_bytes(&[command])?;
            self.dc.set_high();
            self.pixel_mode(true)
        })();
        if result.is_err() {
            self.finish(result)
        } else {
            result
        }
    }
    fn write_pixels(&mut self, pixels: &[u16]) -> Result<(), Self::Error> {
        self.dma.send(self.spi.inner(), pixels)
    }
    fn finish_pixels(&mut self, transfer: Result<(), Self::Error>) -> Result<(), Self::Error> {
        let restore = self.pixel_mode(false);
        self.finish(transfer.and(restore))
    }
}
