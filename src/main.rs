#![no_std]
#![no_main]

mod display;
mod ui;

use cortex_m_rt::entry;
use nb::block;
use panic_halt as _;
use stm32h7xx_hal::pac;
use stm32h7xx_hal::prelude::*;
use stm32h7xx_hal::spi;

use display::DisplaySpi;
use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::*,
};

use ui::{Button, Label, ProgressBar, Screen, Theme};

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
    let ccdr = rcc
        .sys_ck(400.MHz())
        .pll1_q_ck(400.MHz())
        .freeze(pwrcfg, &dp.SYSCFG);

    let gpioa = dp.GPIOA.split(ccdr.peripheral.GPIOA);
    let gpiob = dp.GPIOB.split(ccdr.peripheral.GPIOB);
    let mut led = gpioa.pa1.into_push_pull_output();

    // 屏幕引脚配置
    let mut disp_blk = gpiob.pb0.into_push_pull_output();
    let disp_dc = gpiob.pb1.into_push_pull_output();
    let disp_cs = gpiob.pb12.into_push_pull_output();
    let disp_sck = gpiob.pb13.into_alternate::<5>();
    let disp_miso = gpiob.pb14.into_alternate::<5>();
    let disp_mosi = gpiob.pb15.into_alternate::<5>();

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

    // 初始化 SPI2
    let spi = dp.SPI2.spi(
        (disp_sck, disp_miso, disp_mosi),
        spi::MODE_3,
        48.MHz(),
        ccdr.peripheral.SPI2,
        &ccdr.clocks,
    );
    write_str!(tx, "SPI2 initialized!\r\n");

    // 初始化屏幕
    let mut display = DisplaySpi::new(spi, disp_cs, disp_dc);
    write_str!(tx, "Display created, starting init...\r\n");
    display.init(&mut tx);
    write_str!(tx, "Display init complete!\r\n");

    // 清屏
    display.clear(Rgb565::BLACK).unwrap();

    // LED 快闪 5 次表示进入主循环
    for _ in 0..5 {
        let _ = led.toggle();
        delay_ms(100);
    }

    write_str!(tx, "\r\n=== STM32H750 UI Demo ===\r\n");
    write_str!(tx, "Creating UI screen...\r\n");

    // 创建 UI 屏幕（240x320）
    let screen = Screen::new(240, 320).with_theme(Theme::dark());

    // 添加标题标签
    let title = Label::new(120, 20, "UI Demo").centered();
    let mut screen = screen;
    let _ = screen.add_label(title);

    // 添加按钮
    let btn1 = Button::new(1, 20, 60, 90, 40, "Button 1");
    let _ = screen.add_button(btn1);

    let btn2 = Button::new(2, 130, 60, 90, 40, "Button 2");
    let _ = screen.add_button(btn2);

    // 添加进度条
    let progress = ProgressBar::new(1, 20, 130, 200, 25)
        .with_range(0, 100);
    let _ = screen.add_progress(progress);

    // 添加状态标签
    let status = Label::new(120, 180, "Status: Ready").centered();
    let _ = screen.add_label(status);

    write_str!(tx, "UI screen created!\r\n");

    // 初始绘制
    screen.draw(&mut display).unwrap();

    write_str!(tx, "Starting main loop...\r\n");

    // 动画状态
    let mut anim_value: i32 = 0;
    let mut anim_direction: i32 = 1;
    let mut frame_count: u32 = 0;

    // 主循环
    loop {
        // 更新进度条动画
        anim_value += anim_direction * 2;
        if anim_value >= 100 {
            anim_value = 100;
            anim_direction = -1;
        } else if anim_value <= 0 {
            anim_value = 0;
            anim_direction = 1;
        }

        // 模拟更新进度条值
        // 注意：这里需要修改 Screen 的实现来支持修改控件
        // 暂时重新绘制整个屏幕
        screen.draw(&mut display).unwrap();

        frame_count += 1;

        // LED 慢闪表示运行中
        if frame_count % 60 == 0 {
            let _ = led.toggle();
        }

        delay_ms(16); // 约 60fps
    }
}
