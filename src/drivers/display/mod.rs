//! ILI9341 protocol, drawing and pixel layout, independent of the MCU.
mod error;
mod ili9341;
pub(crate) mod pixel_pack;
#[cfg(any(target_arch = "arm", test))]
pub(crate) mod spi_transfer;

pub use error::Error;
pub use ili9341::{DisplayDriver, DisplayOrientation, DMA_BUF_WORDS, MAX_FRAME_BUFFER_SIZE};

/// Owned transport port. Commands return idle; pixel sessions always finish.
/// Implementations must stop reading borrowed pixels before returning, including
/// errors, and release CS on begin failure or finish_pixels.
pub trait DisplayInterface {
    type Error: From<core::convert::Infallible>;
    fn write_command(&mut self, command: u8) -> Result<(), Self::Error>;
    fn write_data(&mut self, data: &[u8]) -> Result<(), Self::Error>;
    fn write_cmd_data(&mut self, command: u8, data: u8) -> Result<(), Self::Error>;
    fn begin_pixels(&mut self, command: u8) -> Result<(), Self::Error>;
    fn write_pixels(&mut self, pixels: &[u16]) -> Result<(), Self::Error>;
    fn finish_pixels(&mut self, result: Result<(), Self::Error>) -> Result<(), Self::Error>;
}

#[cfg(test)]
mod tests;
