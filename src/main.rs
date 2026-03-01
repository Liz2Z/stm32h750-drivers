#![no_std]
#![no_main]

mod adc_ntc;
mod display;
mod profiler;
mod serial;
mod ui;

use cortex_m_rt::entry;
use panic_halt as _;
use stm32h7xx_hal::pac;
use stm32h7xx_hal::prelude::*;
use stm32h7xx_hal::spi;

use display::{init_frame_buffer, DisplayDriver};
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::*;

use ui::{GrayTheme, HistoryBar, Label, Screen, TempHumidCard};

// 延时函数
fn delay_ms(ms: u32) {
    for _ in 0..ms {
        for _ in 0..8000 {
            cortex_m::asm::nop();
        }
    }
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

    // 初始化 SPI2
    let spi = dp.SPI2.spi(
        (disp_sck, disp_miso, disp_mosi),
        spi::MODE_3,
        80.MHz(),
        ccdr.peripheral.SPI2,
        &ccdr.clocks,
    );

    // 初始化帧缓冲
    init_frame_buffer();

    // 初始化屏幕
    let mut display = DisplayDriver::new(spi, disp_cs, disp_dc);
    display.init(&mut delay_ms);

    // 使用 DMA 清屏（白色背景）
    display.clear(Rgb565::WHITE).unwrap();
    display.flush();

    // 创建 UI 屏幕（320x240）
    let screen = Screen::new(320, 240).with_theme(GrayTheme::new());
    let mut screen = screen;

    // ===== 温湿度传感器测试场景 =====

    // 1. 创建温度卡片
    let mut temp_card = TempHumidCard::new(20, 40, true); // show_temp = true
    temp_card.sensor.update_temp(25.5);
    temp_card.sensor.temp_high = 28.0;
    temp_card.sensor.temp_low = 22.0;
    let _ = screen.add_temp_humd_card(temp_card);

    // 2. 创建湿度卡片
    let mut humid_card = TempHumidCard::new(170, 40, false); // show_temp = false
    humid_card.sensor.update_humid(60.0);
    humid_card.sensor.humid_high = 65.0;
    humid_card.sensor.humid_low = 55.0;
    let _ = screen.add_temp_humd_card(humid_card);

    // 3. 创建历史记录条
    let mut history = HistoryBar::new(20, 180);
    history.update(&[24.0, 23.0, 25.0, 26.0, 25.5, 25.0]);
    let _ = screen.add_history_bar(history);

    // 4. 添加标题
    let title = Label::new(160, 10, "TEMP/HUMID DASHBOARD").centered();
    let _ = screen.add_label(title);

    // 初始绘制（使用 DMA 批量传输）
    screen.draw_with_dma(&mut display).unwrap();

    // 动画状态
    let mut frame_count: u32 = 0;

    // 主循环
    loop {
        frame_count += 1;

        // LED 慢闪表示运行中
        if frame_count % 60 == 0 {
            let _ = led.toggle();
        }

        delay_ms(16); // 约 60fps
    }
}
