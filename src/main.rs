#![no_std]
#![no_main]

use panic_halt as _;
use cortex_m_rt::entry;
use stm32h7xx_hal::{pac, prelude::*, spi};

mod rc522;
mod types;

#[entry]
fn main() -> ! {
    let dp = pac::Peripherals::take().unwrap();

    // 配置电源和时钟
    let pwr = dp.PWR.constrain();
    let pwrcfg = pwr.freeze();

    let rcc = dp.RCC.constrain();
    let ccdr = rcc.freeze(pwrcfg, &dp.SYSCFG);

    // 配置 GPIO
    let gpiob = dp.GPIOB.split(ccdr.peripheral.GPIOB);
    let gpioe = dp.GPIOE.split(ccdr.peripheral.GPIOE);

    // SPI_SCK (PB13), SPI_MISO (PB14), SPI_MOSI (PB15) - 复用功能
    let sck = gpiob.pb13.into_alternate::<5>();
    let miso = gpiob.pb14.into_alternate::<5>();
    let mosi = gpiob.pb15.into_alternate::<5>();

    // SPI_NSS (PB12) - 普通输出
    let mut nss = gpiob.pb12.into_push_pull_output();
    nss.set_high();

    // RC522_RST (PE0) - 推挽输出
    let mut rst = gpioe.pe0.into_push_pull_output();
    rst.set_high();

    // 配置 SPI2 (Mode 0, 1MHz for RC522 initialization)
    let _spi: spi::Spi<_, _, u8> = dp.SPI2.spi(
        (sck, miso, mosi),
        spi::MODE_0,
        1.MHz(),
        ccdr.peripheral.SPI2,
        &ccdr.clocks,
    );

    loop {}
}
