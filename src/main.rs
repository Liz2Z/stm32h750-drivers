#![no_std]
#![no_main]

mod display;

use cortex_m_rt::entry;
use nb::block;
use panic_halt as _;
use stm32h7xx_hal::pac;
use stm32h7xx_hal::prelude::*;
use stm32h7xx_hal::spi;

use display::DisplaySpi;
use embedded_graphics::{pixelcolor::Rgb565, prelude::*};

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

    // 屏幕引脚配置（根据开发板实际连接）
    // PB0 = BLK (背光)
    // PB1 = RS/D/C
    // PB12 = CS (软件控制)
    // PB13 = SCK (SPI2 AF5)
    // PB14 = MISO (SPI2 AF5)
    // PB15 = MOSI (SPI2 AF5)
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

    // 初始化 SPI2 - ILI9341 通常使用 Mode 3 更稳定
    let spi = dp.SPI2.spi(
        (disp_sck, disp_miso, disp_mosi),
        spi::MODE_3,  // CPOL=1, CPHA=1
        1.MHz(),
        ccdr.peripheral.SPI2,
        &ccdr.clocks,
    );
    write_str!(tx, "SPI2 initialized!\r\n");

    // 初始化屏幕
    let mut display = DisplaySpi::new(spi, disp_cs, disp_dc);
    write_str!(tx, "Display created, starting init...\r\n");
    display.init(&mut tx);
    write_str!(tx, "Display init complete!\r\n");

    // LED 快闪 5 次表示进入主循环
    for _ in 0..5 {
        let _ = led.toggle();
        delay_ms(50);
    }

    // 串口欢迎消息
    write_str!(tx, "\r\n=== STM32H750 Display Animation ===\r\n");

    // 动画参数
    let mut x: i32 = 0;
    let mut y: i32 = 0;
    let mut dx: i32 = 4;
    let mut dy: i32 = 3;
    let box_size: u16 = 40;
    let max_x = 240 - box_size as i32;
    let max_y = 320 - box_size as i32;

    // 颜色数组
    let colors = [
        Rgb565::RED,
        Rgb565::GREEN,
        Rgb565::BLUE,
        Rgb565::YELLOW,
        Rgb565::CYAN,
        Rgb565::MAGENTA,
    ];
    let mut color_idx = 0;
    let mut frame_count: u32 = 0;

    write_str!(tx, "Starting animation loop...\r\n");

    // 先清屏一次
    display.clear(Rgb565::BLACK).unwrap();

    // 主循环 - 弹跳方块动画
    loop {
        // 清除上一帧的方块（用黑色覆盖）
        display.fill_rect(x as u16, y as u16, box_size, box_size, Rgb565::BLACK);

        // 更新位置
        x += dx;
        y += dy;

        // 碰撞检测 - X 方向
        if x <= 0 || x >= max_x {
            dx = -dx;
            x = x.clamp(0, max_x);
            color_idx = (color_idx + 1) % colors.len();
        }

        // 碰撞检测 - Y 方向
        if y <= 0 || y >= max_y {
            dy = -dy;
            y = y.clamp(0, max_y);
            color_idx = (color_idx + 1) % colors.len();
        }

        // 绘制新位置的方块
        display.fill_rect(x as u16, y as u16, box_size, box_size, colors[color_idx]);

        // 每 60 帧输出一次状态
        frame_count += 1;
        if frame_count % 60 == 0 {
            write_str!(tx, "Frame: ");
            let c = b'0' + (frame_count / 60 % 10) as u8;
            let _ = block!(tx.write(c));
            write_str!(tx, "\r\n");
        }

        // LED 慢闪表示运行中
        let _ = led.toggle();
        delay_ms(16); // 约 60fps
    }
}
