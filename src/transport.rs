use embedded_hal::spi::FullDuplex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferError<E> {
    Timeout,
    Spi(E),
}

impl<E> From<core::convert::Infallible> for TransferError<E> {
    fn from(error: core::convert::Infallible) -> Self {
        match error {}
    }
}

/// FIFO-aware transmit with bounded stalls, discarding incoming display data.
pub fn write_spi<S: FullDuplex<u8>>(
    spi: &mut S,
    mut now_us: impl FnMut() -> u32,
    bytes: &[u8],
    timeout_us: u32,
) -> Result<(), TransferError<S::Error>> {
    let start = now_us();
    let mut sent = 0;
    let mut received = 0;
    while received < bytes.len() {
        let mut progress = false;
        if sent < bytes.len() && sent - received < 4 {
            match spi.send(bytes[sent]) {
                Ok(()) => {
                    sent += 1;
                    progress = true;
                },
                Err(nb::Error::WouldBlock) => {},
                Err(nb::Error::Other(e)) => return Err(TransferError::Spi(e)),
            }
        }
        if received < sent {
            match spi.read() {
                Ok(_) => {
                    received += 1;
                    progress = true;
                },
                Err(nb::Error::WouldBlock) => {},
                Err(nb::Error::Other(e)) => return Err(TransferError::Spi(e)),
            }
        }
        if !progress && now_us().wrapping_sub(start) >= timeout_us {
            return Err(TransferError::Timeout);
        }
    }
    Ok(())
}
