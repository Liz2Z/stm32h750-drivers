#![no_std]
#![no_main]

use panic_halt as _;
use cortex_m_rt::entry;
use cortex_m::delay::Delay;
use stm32h7xx_hal::{pac, prelude::*, spi};
use stm32h7xx_hal::hal::digital::v2::OutputPin;

mod rc522;
mod types;

use rc522::{RC522, SpiDevice, PinDevice};
use types::CardType;

const DEFAULT_KEY: [u8; 6] = [0xFFu8; 6];

#[entry]
fn main() -> ! {
    let cp = cortex_m::Peripherals::take().unwrap();
    let dp = pac::Peripherals::take().unwrap();

    let mut delay = Delay::new(cp.SYST, 120_000_000);

    // 配置电源和时钟
    let pwr = dp.PWR.constrain();
    let pwrcfg = pwr.freeze();

    let rcc = dp.RCC.constrain();
    let ccdr = rcc.freeze(pwrcfg, &dp.SYSCFG);

    // 配置 GPIO
    let gpiob = dp.GPIOB.split(ccdr.peripheral.GPIOB);
    let gpioe = dp.GPIOE.split(ccdr.peripheral.GPIOE);

    // SPI 引脚
    let sck = gpiob.pb13.into_alternate::<5>();
    let miso = gpiob.pb14.into_alternate::<5>();
    let mosi = gpiob.pb15.into_alternate::<5>();

    // NSS 和 RST
    let nss = gpiob.pb12.into_push_pull_output();
    let rst = gpioe.pe0.into_push_pull_output();

    // 配置 SPI
    let spi = dp.SPI2.spi(
        (sck, miso, mosi),
        spi::MODE_0,
        1.MHz(),
        ccdr.peripheral.SPI2,
        &ccdr.clocks,
    );

    // 创建 RC522 设备包装器，处理 NSS 片选信号
    let rc522_device = Rc522Device::new(spi, nss);

    // 包装 RST 引脚
    let rst_device = PinWrapper::new(rst);

    // 初始化 RC522
    let mut rc522 = RC522::new(rc522_device, rst_device).unwrap();

    loop {
        match rc522.request() {
            Ok(CardType::Mifare1K) => {
                if let Ok(uid) = rc522.anticoll() {
                    if rc522.authenticate(4, types::KeyType::KeyA, &DEFAULT_KEY, &uid).is_ok() {
                        if let Ok(_data) = rc522.read(4) {
                            // 成功读取
                        }
                    }
                }
            }
            _ => {}
        }

        delay.delay_ms(500u32);
    }
}

// RC522 SPI 设备包装器
// 处理 NSS 片选信号，将 stm32h7xx-hal 的 SPI 适配为 RC522 需要的接口
struct Rc522Device<SPI, NSS> {
    spi: SPI,
    nss: NSS,
}

impl<SPI, NSS> Rc522Device<SPI, NSS> {
    fn new(spi: SPI, nss: NSS) -> Self {
        Self { spi, nss }
    }
}

// GPIO Pin 包装器
struct PinWrapper<PIN> {
    pin: PIN,
}

impl<PIN> PinWrapper<PIN> {
    fn new(pin: PIN) -> Self {
        Self { pin }
    }
}

impl<PIN, E> PinDevice for PinWrapper<PIN>
where
    PIN: OutputPin<Error = E>,
{
    type Error = E;

    fn set_low(&mut self) -> Result<(), Self::Error> {
        self.pin.set_low()
    }

    fn set_high(&mut self) -> Result<(), Self::Error> {
        self.pin.set_high()
    }
}

// 实现 SPI 设备 trait
impl<SPI, NSS, SE> SpiDevice for Rc522Device<SPI, NSS>
where
    SPI: stm32h7xx_hal::hal::spi::FullDuplex<u8, Error = SE>,
    NSS: OutputPin,
{
    type Error = SE;

    fn read(&mut self) -> nb::Result<u8, Self::Error> {
        let _ = self.nss.set_low();
        let result = self.spi.read();
        let _ = self.nss.set_high();
        result
    }

    fn send(&mut self, byte: u8) -> nb::Result<(), Self::Error> {
        let _ = self.nss.set_low();
        let result = self.spi.send(byte);
        let _ = self.nss.set_high();
        result
    }
}
