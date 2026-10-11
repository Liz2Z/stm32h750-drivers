//! Errors shared by display command and pixel transfers.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error<E> {
    Timeout,
    Spi(E),
    Dma,
    InvalidBuffer,
}

impl<E> From<core::convert::Infallible> for Error<E> {
    fn from(error: core::convert::Infallible) -> Self {
        match error {}
    }
}
