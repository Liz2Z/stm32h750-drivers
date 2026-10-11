//! Bounded 8-bit writes for display commands and their parameters.
use super::Error;
use embedded_hal::spi::FullDuplex;

/// Bounded command writes for a transmit-only SPI bus; no RX FIFO is expected.
pub(crate) fn write_bytes<S: FullDuplex<u8>>(
    spi: &mut S,
    mut now_us: impl FnMut() -> u32,
    bytes: &[u8],
    timeout_us: u32,
) -> Result<(), Error<S::Error>> {
    let start = now_us();
    for &byte in bytes {
        loop {
            match spi.send(byte) {
                Ok(()) => break,
                Err(nb::Error::Other(e)) => return Err(Error::Spi(e)),
                Err(nb::Error::WouldBlock) => {
                    if now_us().wrapping_sub(start) >= timeout_us {
                        return Err(Error::Timeout);
                    }
                },
            }
        }
    }
    Ok(())
}
