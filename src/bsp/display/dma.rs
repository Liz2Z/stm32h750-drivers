//! Exclusive DMA1 stream 0 owner for synchronous SPI2 pixel transfers.
use super::{DisplayError, Error};
use crate::bsp::clock::Clock;
use core::sync::atomic::{compiler_fence, Ordering};
use stm32h7xx_hal::{
    pac::{DMA1, DMAMUX1, RCC, SPI2},
    rcc::rec::{Dma1, ResetEnable},
};

pub(super) struct DisplayDma<'a> {
    dma: DMA1,
    clock: &'a Clock,
}

impl<'a> DisplayDma<'a> {
    pub(super) fn new(dma: DMA1, prec: Dma1, clock: &'a Clock) -> Self {
        let _ = prec.enable().reset();
        // The application enables I-cache only. DMA reads AXI SRAM directly.
        assert!(
            !cortex_m::peripheral::SCB::dcache_enabled(),
            "Display DMA requires non-cached buffers"
        );
        Self { dma, clock }
    }

    fn clear_flags(&self) {
        self.dma.lifcr.write(|w| {
            w.ctcif0()
                .set_bit()
                .chtif0()
                .set_bit()
                .cteif0()
                .set_bit()
                .cdmeif0()
                .set_bit()
                .cfeif0()
                .set_bit()
        });
    }

    /// Stop all memory reads before returning a buffer to its mutable owner.
    pub(super) fn abort(&mut self) {
        let stream = &self.dma.st[0];
        stream.cr.modify(|_, w| w.en().clear_bit());
        let start = self.clock.now_us();
        while stream.cr.read().en().bit_is_set() {
            if self.clock.expired(start, 1_000) {
                // This driver owns the entire DMA1 peripheral, so resetting it
                // cannot invalidate another user's stream or buffer.
                // SAFETY: this adapter owns DMA1 exclusively; RCC reset stops all its streams.
                unsafe {
                    let rcc = &*RCC::ptr();
                    rcc.ahb1rstr.modify(|_, w| w.dma1rst().set_bit());
                    cortex_m::asm::dsb();
                    rcc.ahb1rstr.modify(|_, w| w.dma1rst().clear_bit());
                    let _ = rcc.ahb1rstr.read();
                }
                break;
            }
        }
        cortex_m::asm::dsb();
        compiler_fence(Ordering::SeqCst);
        self.clear_flags();
    }

    /// SPI must be configured for 16-bit, MSB-first, transmit-only operation.
    /// This call retains the immutable borrow until DMA and the wire are idle.
    pub(super) fn send(&mut self, spi: &SPI2, pixels: &[u16]) -> Result<(), DisplayError> {
        if pixels.is_empty() {
            return Ok(());
        }
        let address = pixels.as_ptr() as usize;
        let fits = pixels.len() <= u16::MAX as usize
            && address >= 0x2400_0000
            && address
                .checked_add(pixels.len() * 2)
                .is_some_and(|end| end <= 0x2408_0000);
        if !fits {
            return Err(Error::InvalidBuffer);
        }
        spi.cfg1.modify(|_, w| w.txdmaen().clear_bit());
        self.abort();
        let stream = &self.dma.st[0];
        // Reapply the request line on every transfer, including after recovery.
        // SAFETY: stream 0 is stopped and exclusively owned here.
        unsafe {
            (*DMAMUX1::ptr()).ccr[0].write(|w| w.dmareq_id().spi2_tx_dma());
        }
        // SAFETY: half-word TXDR and AXI SRAM addresses are validated above;
        // the immutable pixel borrow lasts until abort confirms DMA has stopped.
        stream
            .par
            .write(|w| unsafe { w.bits(spi.txdr.as_ptr() as u32) });
        stream.m0ar.write(|w| unsafe { w.bits(address as u32) });
        stream.ndtr.write(|w| w.ndt().bits(pixels.len() as u16));
        // Buffer memory reads before servicing SPI requests.
        stream.fcr.write(|w| w.dmdis().set_bit().fth().full());
        stream.cr.write(|w| {
            w.dir()
                .memory_to_peripheral()
                .minc()
                .incremented()
                .psize()
                .bits16()
                .msize()
                .bits16()
                .pl()
                .high()
        });
        compiler_fence(Ordering::SeqCst);
        cortex_m::asm::dmb();
        stream.cr.modify(|_, w| w.en().set_bit());
        spi.cfg1.modify(|_, w| w.txdmaen().set_bit());
        spi.cr1.modify(|_, w| w.cstart().set_bit());
        let start = self.clock.now_us();
        let result = loop {
            let flags = self.dma.lisr.read();
            if flags.teif0().bit_is_set() || flags.dmeif0().bit_is_set() {
                break Err(Error::Dma);
            }
            // RM0433 section 15.3.11: FIFO starvation doesn't acknowledge the
            // peripheral request and loses no data. This master simply pauses
            // SCK until data arrives. A bad FIFO configuration disables EN;
            // detect that separately, and still require TC plus wire-idle.
            if stream.cr.read().en().bit_is_clear() && self.dma.lisr.read().tcif0().bit_is_clear() {
                break Err(Error::Dma);
            }
            let status = spi.sr.read();
            if status.modf().bit_is_set() {
                break Err(Error::Spi(stm32h7xx_hal::spi::Error::ModeFault));
            }
            if flags.tcif0().bit_is_set() && status.txc().bit_is_set() {
                break Ok(());
            }
            if self.clock.expired(start, 100_000) {
                break Err(Error::Timeout);
            }
        };
        self.abort();
        result
    }
}
