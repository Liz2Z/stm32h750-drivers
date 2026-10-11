//! STM32H750-specific clocks, peripherals and DMA memory.
pub mod adc_ntc;
pub mod board;
pub mod bounded_i2c;
pub mod clock;
pub mod display;
#[cfg(feature = "profiler")]
pub mod profiler;
