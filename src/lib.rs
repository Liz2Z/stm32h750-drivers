#![no_std]

// Hardware-independent logic is also compiled by the host regression tests.
pub mod application;
#[cfg(target_arch = "arm")]
pub mod bsp;
pub mod drivers;
pub mod ui;

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod regression_tests;
