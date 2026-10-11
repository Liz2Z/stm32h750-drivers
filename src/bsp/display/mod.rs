//! STM32H750 SPI2 display adapter; owns all DMA1 resources.
#[cfg(feature = "display-bench")]
pub mod bench;
mod dma;
mod framebuffer;
mod spi2;

use crate::{
    bsp::clock::Clock,
    drivers::display::{DisplayOrientation, Error},
};
pub use spi2::Spi2Display;
use stm32h7xx_hal::{
    gpio::{Output, PushPull, PB1, PB12},
    pac::{DMA1, SPI2},
    rcc::rec::Dma1,
    spi::{Enabled, Spi},
};

pub type DisplaySpi = Spi<SPI2, Enabled, u8>;
pub type DisplayError = Error<stm32h7xx_hal::spi::Error>;
pub type DisplayDriver<'a> = crate::drivers::display::DisplayDriver<Spi2Display<'a>>;

/// Transfer peripheral ownership and claim uncached AXI SRAM buffers once.
pub fn new_display(
    spi: DisplaySpi,
    cs: PB12<Output<PushPull>>,
    dc: PB1<Output<PushPull>>,
    clock: &Clock,
    orientation: DisplayOrientation,
    dma: DMA1,
    dma_prec: Dma1,
) -> DisplayDriver<'_> {
    let interface = Spi2Display::new(spi, cs, dc, clock, dma, dma_prec);
    let (frame, transfer) = framebuffer::take_buffers();
    DisplayDriver::new(interface, frame, transfer, orientation)
}
