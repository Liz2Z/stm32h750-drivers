//! Clock, GPIO and peripheral assembly for this STM32H750 board.
use super::buttons::Buttons;
use super::rc522::Rc522Spi;
use super::{bounded_i2c::BoundedI2c, clock::Clock, display::DisplayDriver};
use crate::drivers::rc522::Rc522;
use crate::{
    bsp,
    drivers::{DisplayOrientation, Esp8266},
};
use core::convert::Infallible;
use stm32h7xx_hal::{
    gpio::{Output, PushPull, PA1},
    pac,
    prelude::*,
    serial::{Rx, SerialExt, Tx},
    spi,
};

pub struct Board<'a> {
    pub display: DisplayDriver<'a>,
    pub i2c: BoundedI2c<'a>,
    pub rc522: Rc522<Rc522Spi<'a>>,
    pub buttons: Buttons,
    pub esp8266: Esp8266<Tx<pac::USART3>, Rx<pac::USART3>>,
    pub led: PA1<Output<PushPull>>,
    #[cfg(feature = "display-bench")]
    pub spi_kernel_hz: u32,
}

/// Keep clock-backed resources in one stack frame for the application's lifetime.
pub fn with_board(
    run: impl for<'a> FnOnce(Board<'a>, &'a Clock) -> Infallible,
    uart_baud: u32,
) -> ! {
    let dp = pac::Peripherals::take().unwrap();
    let mut core = cortex_m::Peripherals::take().unwrap();
    core.SCB.enable_icache(); // instruction cache only; DMA buffers remain uncached

    // 配置电源和时钟
    let pwr = dp.PWR.constrain();
    let pwrcfg = pwr.vos1().freeze(); // VOS1 = 最高性能

    let rcc = dp.RCC.constrain();
    let ccdr = rcc
        .sys_ck(400.MHz())
        .pll1_q_ck(400.MHz())
        .freeze(pwrcfg, &dp.SYSCFG);

    let clock = Clock(
        dp.TIM2
            .tick_timer(1.MHz(), ccdr.peripheral.TIM2, &ccdr.clocks),
    );
    let sensor_delay = |ms| clock.delay_ms(ms);

    // GPIO 与外设仅在 BSP 中装配。
    let gpioa = dp.GPIOA.split(ccdr.peripheral.GPIOA);
    let gpiob = dp.GPIOB.split(ccdr.peripheral.GPIOB);
    let gpioc = dp.GPIOC.split(ccdr.peripheral.GPIOC);
    let gpioe = dp.GPIOE.split(ccdr.peripheral.GPIOE);
    let buttons = Buttons::new(
        gpioe.pe3.into_pull_up_input(),
        gpioc.pc5.into_pull_up_input(),
    );

    // LED 指示灯 - 用于显示系统状态
    // 正常运行时会闪烁，读取传感器成功时也会闪烁
    let mut led = gpioa.pa1.into_push_pull_output();

    // RC522 独占 SPI1：PA5=SCK、PA6=MISO、PA7=MOSI，MODE_0。
    // 请求 1 MHz；400 MHz 内核 / 256，实际为 1.5625 MHz（低于器件 10 MHz 上限）。
    let reader_spi = dp.SPI1.spi(
        (
            gpioa.pa5.into_alternate::<5>(),
            gpioa.pa6.into_alternate::<5>(),
            gpioa.pa7.into_alternate::<5>(),
        ),
        spi::Config::new(spi::MODE_0),
        1.MHz(),
        ccdr.peripheral.SPI1,
        &ccdr.clocks,
    );
    let rc522 = Rc522::new(Rc522Spi::new(
        reader_spi,
        gpioa.pa4.into_push_pull_output(),
        gpioa.pa3.into_push_pull_output(),
        &clock,
    ));

    // ESP8266-01S 串口（USART3）
    // PB10 -> ESP RX
    // PB11 <- ESP TX
    let esp_tx_pin = gpiob.pb10.into_alternate::<7>();
    let esp_rx_pin = gpiob.pb11.into_alternate::<7>();
    let esp_serial = dp
        .USART3
        .serial(
            (esp_tx_pin, esp_rx_pin),
            uart_baud.bps(),
            ccdr.peripheral.USART3,
            &ccdr.clocks,
        )
        .unwrap();
    let (esp_tx, esp_rx) = esp_serial.split();
    let esp8266 = Esp8266::new(esp_tx, esp_rx);
    // 屏幕控制引脚
    let mut disp_blk = gpiob.pb0.into_push_pull_output(); // 背光
    let disp_dc = gpiob.pb1.into_push_pull_output(); // 数据/命令
    let disp_cs = gpiob.pb12.into_push_pull_output(); // 片选

    // SPI 引脚配置
    // 当前 SPI2 实际 SCK 为 50MHz。
    let disp_sck = gpiob
        .pb13
        .into_alternate::<5>()
        .speed(stm32h7xx_hal::gpio::Speed::VeryHigh);
    let disp_miso = gpiob
        .pb14
        .into_alternate::<5>()
        .speed(stm32h7xx_hal::gpio::Speed::VeryHigh);
    let disp_mosi = gpiob
        .pb15
        .into_alternate::<5>()
        .speed(stm32h7xx_hal::gpio::Speed::VeryHigh);

    // 启动动画：LED 快闪表示正在初始化
    for _ in 0..10 {
        led.toggle();
        sensor_delay(50);
    }

    // 打开屏幕背光
    disp_blk.set_high();

    // 初始化 SPI2
    // 沿用当前屏幕的 MODE_3；80MHz 是请求值，实际时钟取决于内核时钟与分频。
    let spi = dp.SPI2.spi(
        (disp_sck, disp_miso, disp_mosi),
        spi::Config::new(spi::MODE_3).communication_mode(spi::CommunicationMode::Transmitter),
        80.MHz(),
        ccdr.peripheral.SPI2,
        &ccdr.clocks,
    );

    // The display owns and initializes its AXISRAM buffers once.
    let display = bsp::display::new_display(
        spi,
        disp_cs,
        disp_dc,
        &clock,
        DisplayOrientation::Landscape,
        dp.DMA1,
        ccdr.peripheral.DMA1,
    );
    // 第四阶段：传感器初始化
    // ============================================================

    // I2C1 引脚配置 (用于 AHT20 + BMP280 传感器)
    // PB6 = I2C1_SCL, PB7 = I2C1_SDA
    let i2c_scl = gpiob.pb6.into_alternate::<4>().set_open_drain();
    let i2c_sda = gpiob.pb7.into_alternate::<4>().set_open_drain();

    // 初始化 I2C1 (100kHz)
    let i2c = dp.I2C1.i2c(
        (i2c_scl, i2c_sda),
        100.kHz(),
        ccdr.peripheral.I2C1,
        &ccdr.clocks,
    );

    let i2c = BoundedI2c::new(i2c, &clock);

    let board = Board {
        display,
        i2c,
        rc522,
        buttons,
        esp8266,
        led,
        #[cfg(feature = "display-bench")]
        spi_kernel_hz: bsp::display::DisplaySpi::kernel_clk_unwrap(&ccdr.clocks).raw(),
    };
    match run(board, &clock) {}
}
