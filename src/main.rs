#![no_std]
#![no_main]

mod display;

use core::fmt::Write;
use cortex_m_rt::entry;
use nb::block;
use panic_halt as _;
use stm32h7xx_hal::pac;
use stm32h7xx_hal::prelude::*;

use display::DisplaySpi;
use embedded_graphics::{
    mono_font::{ascii::FONT_10X20, MonoTextStyle},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::Rectangle,
    text::Text,
};

// 延时函数
fn delay_ms(ms: u32) {
    for _ in 0..ms {
        for _ in 0..8000 {
            cortex_m::asm::nop();
        }
    }
}

// 宏：写入字符串到串口
macro_rules! write_str {
    ($tx:expr, $s:expr) => {
        for b in $s.bytes() {
            let _ = block!($tx.write(b));
        }
    };
}

#[entry]
fn main() -> ! {
    let dp = pac::Peripherals::take().unwrap();

    let pwr = dp.PWR.constrain();
    let pwrcfg = pwr.vos1().freeze();

    let rcc = dp.RCC.constrain();
    let ccdr = rcc.sys_ck(96.MHz()).freeze(pwrcfg, &dp.SYSCFG);

    let gpioa = dp.GPIOA.split(ccdr.peripheral.GPIOA);
    let gpiob = dp.GPIOB.split(ccdr.peripheral.GPIOB);
    let mut led = gpioa.pa1.into_push_pull_output();

    // 屏幕引脚配置（根据开发板实际连接）
    // PB0 = BLK (背光)
    // PB1 = RS/D/C
    // PB12 = CS
    // PB13 = SCK
    // PB14 = MISO
    // PB15 = MOSI
    let mut disp_blk = gpiob.pb0.into_push_pull_output();
    let disp_dc = gpiob.pb1.into_push_pull_output();
    let disp_cs = gpiob.pb12.into_push_pull_output();
    let disp_sck = gpiob.pb13.into_push_pull_output();
    let disp_miso = gpiob.pb14.into_input();
    let disp_mosi = gpiob.pb15.into_push_pull_output();

    // LED 快闪 10 次
    for _ in 0..10 {
        let _ = led.toggle();
        delay_ms(50);
    }

    // 打开背光
    let _ = disp_blk.set_high();

    // USART2
    let tx = gpioa.pa2.into_alternate::<7>();
    let rx = gpioa.pa3.into_alternate::<7>();
    let serial = dp
        .USART2
        .serial((tx, rx), 9600.bps(), ccdr.peripheral.USART2, &ccdr.clocks)
        .unwrap();
    let (mut tx, _rx) = serial.split();

    // 初始化屏幕
    let mut display = DisplaySpi::new(
        disp_sck, disp_mosi, disp_miso, disp_cs, disp_dc,
    );
    display.init();

    // 清屏为黑色背景
    display.clear(Rgb565::BLACK).unwrap();

    // 画一个红色正方形
    display.fill_rect(10, 10, 100, 100, Rgb565::RED);

    // 画一个蓝色正方形
    display.fill_rect(120, 10, 50, 50, Rgb565::BLUE);

    // 画一个绿色正方形
    display.fill_rect(10, 120, 50, 50, Rgb565::GREEN);

    // LED 闪烁两次表示初始化完成
    let _ = led.toggle();
    delay_ms(100);
    let _ = led.toggle();

    // 串口欢迎消息
    write_str!(tx, "\r\n=== STM32H750 Display Demo ===\r\n");
    write_str!(tx, "Display: Hello World shown\r\n");

    // 主循环
    loop {
        // LED 慢闪表示运行中
        let _ = led.toggle();
        delay_ms(500);
    }
}
