#![no_std]
#![no_main]

mod display;
mod ui;

use cortex_m::peripheral::DWT;
use cortex_m_rt::entry;
use nb::block;
use panic_halt as _;
use stm32h7xx_hal::pac;
use stm32h7xx_hal::prelude::*;
use stm32h7xx_hal::spi;

use display::{DisplayDriver, init_frame_buffer};
use embedded_graphics::prelude::*;
use embedded_graphics::pixelcolor::Rgb565;

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

// 宏：写入数字到串口
macro_rules! write_num {
    ($tx:expr, $n:expr) => {{
        let mut n = $n;
        let mut buf = [0u8; 12];
        let mut i = 0;
        if n == 0 {
            buf[i] = b'0';
            i = 1;
        } else {
            let mut temp = n;
            let mut len = 0;
            while temp > 0 {
                len += 1;
                temp /= 10;
            }
            i = len;
            while n > 0 {
                i -= 1;
                buf[i] = b'0' + (n % 10) as u8;
                n /= 10;
            }
            i = len;
        }
        for j in 0..i {
            let _ = block!($tx.write(buf[j]));
        }
    }};
}

// 宏：测量并输出耗时
macro_rules! measure_time {
    ($tx:expr, $name:expr, $operation:block) => {
        {
            let start = DWT::cycle_count();
            $operation
            let end = DWT::cycle_count();
            let cycles = end.wrapping_sub(start);
            let time_us = cycles / 400; // 400MHz = 400 cycles/us
            write_str!($tx, $name);
            write_str!($tx, " took: ");
            write_num!($tx, time_us / 1000);
            write_str!($tx, ".");
            write_num!($tx, (time_us % 1000) / 100);
            write_str!($tx, " ms\r\n");
            time_us
        }
    };
}

#[entry]
fn main() -> ! {
    let cp = cortex_m::Peripherals::take().unwrap();
    let dp = pac::Peripherals::take().unwrap();

    let pwr = dp.PWR.constrain();
    let pwrcfg = pwr.vos1().freeze();

    let rcc = dp.RCC.constrain();
    let ccdr = rcc
        .sys_ck(400.MHz())
        .pll1_q_ck(400.MHz())
        .freeze(pwrcfg, &dp.SYSCFG);

    // 启用 DWT 周期计数器
    let mut dwt = cp.DWT;
    dwt.enable_cycle_counter();

    let gpioa = dp.GPIOA.split(ccdr.peripheral.GPIOA);
    let gpiob = dp.GPIOB.split(ccdr.peripheral.GPIOB);
    let mut led = gpioa.pa1.into_push_pull_output();

    // 屏幕引脚配置
    let mut disp_blk = gpiob.pb0.into_push_pull_output();
    let disp_dc = gpiob.pb1.into_push_pull_output();
    let disp_cs = gpiob.pb12.into_push_pull_output();
    // SPI 引脚需要 VeryHigh 速度才能支持 80MHz
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

    // LED 快闪 10 次
    for _ in 0..10 {
        let _ = led.toggle();
        delay_ms(50);
    }

    // 打开背光
    let _ = disp_blk.set_high();

    // USART1 (PA9/PA10)
    let tx = gpioa.pa9.into_alternate::<7>();
    let rx = gpioa.pa10.into_alternate::<7>();
    let serial = dp
        .USART1
        .serial((tx, rx), 9600.bps(), ccdr.peripheral.USART1, &ccdr.clocks)
        .unwrap();
    let (mut tx, _) = serial.split();

    // 初始化 SPI2
    let spi = dp.SPI2.spi(
        (disp_sck, disp_miso, disp_mosi),
        spi::MODE_3,
        80.MHz(),
        ccdr.peripheral.SPI2,
        &ccdr.clocks,
    );
    write_str!(tx, "SPI2 initialized at 80MHz!\r\n");

    // 初始化帧缓冲
    init_frame_buffer();
    write_str!(tx, "Frame buffer initialized!\r\n");

    // 初始化屏幕
    let mut display = DisplayDriver::new(spi, disp_cs, disp_dc);
    write_str!(tx, "Display init...\r\n");
    display.init(&mut delay_ms);
    write_str!(tx, "Display initialized!\r\n");

    // 使用 DMA 清屏（黑色背景）
    write_str!(tx, "Clearing screen with DMA...\r\n");
    measure_time!(tx, "Clear screen (DMA)", {
        display.clear(Rgb565::BLACK).unwrap();
        display.flush();
    });

    write_str!(tx, "\r\n=== STM32H750 UI Demo with DMA ===\r\n");
    write_str!(tx, "Creating UI screen...\r\n");

    // 创建 UI 屏幕（240x320）
    let screen = Screen::new(240, 320).with_theme(Theme::dark());

    // 添加标题标签
    let title = Label::new(120, 20, "DMA UI Demo").centered();
    let mut screen = screen;
    let _ = screen.add_label(title);

    // 添加按钮
    let btn1 = Button::new(1, 20, 60, 90, 40, "Button 1");
    let _ = screen.add_button(btn1);

    let btn2 = Button::new(2, 130, 60, 90, 40, "Button 2");
    let _ = screen.add_button(btn2);

    // 添加进度条
    let progress = ProgressBar::new(1, 20, 130, 200, 25).with_range(0, 100);
    let _ = screen.add_progress(progress);

    // 添加状态标签
    let status = Label::new(120, 180, "Status: DMA Ready").centered();
    let _ = screen.add_label(status);

    write_str!(tx, "UI screen created!\r\n");

    // 测量初始绘制耗时（使用 DMA 批量传输）
    measure_time!(tx, "Initial full screen draw with DMA", {
        screen.draw_with_dma(&mut display).unwrap();
    });

    write_str!(tx, "\r\n=== Benchmark End ===\r\n\r\n");
    write_str!(tx, "Starting main loop...\r\n");

    // 动画状态
    let mut anim_value: i32 = 0;
    let mut anim_direction: i32 = 1;
    let mut frame_count: u32 = 0;
    let mut last_val1: i32 = -1;

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

        // 只在值变化时更新
        let changed1 = anim_value != last_val1;

        if changed1 {
            if let Some(pb) = screen.get_progress_bar(1) {
                pb.set_value(anim_value);
            }
            last_val1 = anim_value;
            // 使用 DMA 更新进度条
            measure_time!(tx, "Progress bar update (DMA)", {
                let _ = screen.update_progress_bar_with_dma(&mut display, 1);
            });
        }

        frame_count += 1;

        // LED 慢闪表示运行中
        if frame_count % 60 == 0 {
            let _ = led.toggle();
        }

        delay_ms(100); // 约 10fps
    }
}
