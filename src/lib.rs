#![no_std]

// Hardware-independent logic is also compiled by the host regression tests.
pub mod acquisition;
#[path = "drivers/aht20.rs"]
pub mod aht20;
#[path = "drivers/bmp280.rs"]
pub mod bmp280;
pub mod framebuffer;
pub mod scheduling;
#[path = "ui/sensor.rs"]
pub mod sensor;
pub mod transport;
pub mod ui;

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod regression_tests;

/// Clip a refresh window without overflowing coordinates or sending empty areas.
pub fn clip_region(
    x: u16,
    y: u16,
    w: u16,
    h: u16,
    width: u16,
    height: u16,
) -> Option<(u16, u16, u16, u16)> {
    if w == 0 || h == 0 || x >= width || y >= height {
        return None;
    }
    Some((x, y, w.min(width - x), h.min(height - y)))
}
