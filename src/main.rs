#![no_std]
#![no_main]

use panic_halt as _;
use cortex_m_rt::entry;
use stm32h7xx_hal::pac;
use stm32h7xx_hal::prelude::*;
use nb::block;

mod rc522;
mod types;
mod cli;

use rc522::{RC522, SpiDevice, PinDevice};
use types::CardType;
use cli::parse;

const DEFAULT_KEY: [u8; 6] = [0xFFu8; 6];

// 命令缓冲区
struct CmdBuffer {
    data: [u8; 64],
    idx: usize,
}

impl CmdBuffer {
    fn new() -> Self {
        Self {
            data: [0u8; 64],
            idx: 0,
        }
    }

    fn push(&mut self, byte: u8) {
        if self.idx < self.data.len() {
            self.data[self.idx] = byte;
            self.idx += 1;
        }
    }

    fn clear(&mut self) {
        self.idx = 0;
    }

    fn as_str(&self) -> &str {
        unsafe { core::str::from_utf8_unchecked(&self.data[..self.idx]) }
    }

    fn backspace(&mut self) {
        if self.idx > 0 {
            self.idx -= 1;
        }
    }
}

type SerialTx = stm32h7xx_hal::serial::Tx<pac::USART1>;
type SerialRx = stm32h7xx_hal::serial::Rx<pac::USART1>;

#[entry]
fn main() -> ! {
    let dp = pac::Peripherals::take().unwrap();

    // 配置电源和时钟
    let pwr = dp.PWR.constrain();
    let pwrcfg = pwr.freeze();

    let rcc = dp.RCC.constrain();
    // 使用 25MHz 外部晶振
    let ccdr = rcc
        .use_hse(25.MHz())
        .sysclk(480.MHz())
        .freeze(pwrcfg, &dp.SYSCFG);

    // 配置 GPIO
    let gpiob = dp.GPIOB.split(ccdr.peripheral.GPIOB);
    let gpioe = dp.GPIOE.split(ccdr.peripheral.GPIOE);
    let gpioa = dp.GPIOA.split(ccdr.peripheral.GPIOA);

    // SPI2 引脚 - RC522
    let sck = gpiob.pb13.into_alternate::<5>();
    let miso = gpiob.pb14.into_alternate::<5>();
    let mosi = gpiob.pb15.into_alternate::<5>();

    // NSS 和 RST
    let nss = gpiob.pb12.into_push_pull_output();
    let rst = gpioe.pe0.into_push_pull_output();

    // 配置 SPI2 (u8)
    let spi: stm32h7xx_hal::spi::Spi<_, _, u8> = dp.SPI2.spi(
        (sck, miso, mosi),
        stm32h7xx_hal::spi::MODE_0,
        1.MHz(),
        ccdr.peripheral.SPI2,
        &ccdr.clocks,
    );

    // USART1 引脚 - PA9(TX), PA10(RX) - AF7
    let tx = gpioa.pa9.into_alternate::<7>();
    let rx = gpioa.pa10.into_alternate::<7>();

    // 配置 USART1 - 115200 波特率
    let serial = dp.USART1.serial(
        (tx, rx),
        115200.bps(),
        ccdr.peripheral.USART1,
        &ccdr.clocks,
    ).unwrap();

    // 分离串口的发送和接收部分
    let (mut tx, mut rx) = serial.split();

    // 发送欢迎消息
    write_str(&mut tx, b"\r\n=== RFID-RC522 STM32H750 ===\r\n");
    write_str(&mut tx, b"Commands: SCAN, READ, WRITE, DUMP, STATUS, HELP\r\n");
    write_str(&mut tx, b"> ");

    // 创建 RC522 设备包装器
    let rc522_device = Rc522Device::new(spi, nss);
    let rst_device = PinWrapper::new(rst);

    // 初始化 RC522
    let mut rc522 = match RC522::new(rc522_device, rst_device) {
        Ok(r) => {
            write_str(&mut tx, b"RC522 initialized\r\n");
            r
        }
        Err(_) => {
            write_str(&mut tx, b"RC522 init failed\r\n");
            loop { cortex_m::asm::nop(); }
        }
    };

    // 命令缓冲区
    let mut cmd_buffer = CmdBuffer::new();

    // 主循环 - 命令处理
    loop {
        // 读取串口字符
        match block!(rx.read()) {
            Ok(byte) => {
                // 回显字符
                let _ = block!(tx.write(byte));

                match byte {
                    b'\r' | b'\n' => {
                        let _ = block!(tx.write(b'\n'));
                        if cmd_buffer.idx > 0 {
                            let cmd = cmd_buffer.as_str();
                            execute_command(&mut tx, &mut rc522, cmd);
                            cmd_buffer.clear();
                            write_str(&mut tx, b"> ");
                        }
                    }
                    8 | 127 => {
                        // 退格键
                        cmd_buffer.backspace();
                    }
                    32..=126 => {
                        // 可打印字符
                        cmd_buffer.push(byte);
                    }
                    _ => {}
                }
            }
            Err(_) => {
                // 继续循环
                cortex_m::asm::nop();
            }
        }
    }
}

fn write_str(tx: &mut SerialTx, s: &[u8]) {
    for &b in s {
        let _ = block!(tx.write(b));
    }
}

fn write_hex(tx: &mut SerialTx, byte: u8) {
    let high = (byte >> 4) & 0x0F;
    let low = byte & 0x0F;
    let _ = block!(tx.write(to_hex(high)));
    let _ = block!(tx.write(to_hex(low)));
}

fn to_hex(n: u8) -> u8 {
    if n < 10 { b'0' + n } else { b'A' + n - 10 }
}

fn execute_command<SPI, SPIE, RST, RSTE>(
    tx: &mut SerialTx,
    rc522: &mut RC522<SPI, RST>,
    cmd: &str,
) where
    SPI: SpiDevice<Error = SPIE>,
    RST: PinDevice<Error = RSTE>,
{
    match parse(cmd) {
        cli::Command::Scan => {
            write_str(tx, b"Scanning...\r\n");
            match rc522.request() {
                Ok(CardType::Mifare1K) => {
                    write_str(tx, b"Card: Mifare1K\r\n");
                    match rc522.anticoll() {
                        Ok(uid) => {
                            write_str(tx, b"UID: ");
                            for b in &uid {
                                write_hex(tx, *b);
                            }
                            write_str(tx, b"\r\n");
                        }
                        Err(_) => {
                            write_str(tx, b"Anticoll error\r\n");
                        }
                    }
                }
                Ok(CardType::Unknown) => {
                    write_str(tx, b"Card: Unknown\r\n");
                }
                Err(_) => {
                    write_str(tx, b"No card detected\r\n");
                }
                _ => {}
            }
        }
        cli::Command::Read => {
            write_str(tx, b"Reading block 4...\r\n");
            match rc522.request() {
                Ok(CardType::Mifare1K) => {
                    if let Ok(uid) = rc522.anticoll() {
                        write_str(tx, b"UID: ");
                        for b in &uid {
                            write_hex(tx, *b);
                        }
                        write_str(tx, b"\r\n");

                        if rc522.authenticate(4, types::KeyType::KeyA, &DEFAULT_KEY, &uid).is_ok() {
                            match rc522.read(4) {
                                Ok(data) => {
                                    write_str(tx, b"Data: ");
                                    for (i, b) in data.iter().enumerate() {
                                        write_hex(tx, *b);
                                        if i < 15 {
                                            write_str(tx, b" ");
                                        }
                                    }
                                    write_str(tx, b"\r\n");
                                }
                                Err(_) => {
                                    write_str(tx, b"Read error\r\n");
                                }
                            }
                        } else {
                            write_str(tx, b"Auth error\r\n");
                        }
                    }
                }
                Err(_) => {
                    write_str(tx, b"No card detected\r\n");
                }
                _ => {}
            }
        }
        cli::Command::Write => {
            write_str(tx, b"Not implemented yet\r\n");
        }
        cli::Command::Dump => {
            write_str(tx, b"Not implemented yet\r\n");
        }
        cli::Command::Status => {
            write_str(tx, b"Status: Ready\r\n");
        }
        cli::Command::Help => {
            write_str(tx, b"Commands:\r\n");
            write_str(tx, b"  SCAN   - Scan for card\r\n");
            write_str(tx, b"  READ   - Read block 4\r\n");
            write_str(tx, b"  WRITE  - Write block 4\r\n");
            write_str(tx, b"  STATUS - System status\r\n");
            write_str(tx, b"  HELP   - Show help\r\n");
        }
        cli::Command::Unknown => {
            write_str(tx, b"Unknown command\r\n");
        }
    }
}

// RC522 SPI 设备包装器
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
    PIN: stm32h7xx_hal::hal::digital::v2::OutputPin<Error = E>,
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
    NSS: stm32h7xx_hal::hal::digital::v2::OutputPin,
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
