//! STM32H750 firmware entry; board assembly and application have separate owners.
#![no_std]
#![no_main]
use cortex_m_rt::entry;
use panic_halt as _;
use stm32h750_drivers::{application, bsp};

#[entry]
fn main() -> ! {
    bsp::board::with_board(application::run, application::config::ESP8266_UART_BAUD)
}
