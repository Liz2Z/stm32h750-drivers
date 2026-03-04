#![no_std]
#![no_main]

mod adc_ntc;
mod dht11;
mod display;
mod profiler;
mod serial;
mod ui;

use cortex_m_rt::entry;
use panic_halt as _;
use stm32h7xx_hal::pac;
use stm32h7xx_hal::prelude::*;
use stm32h7xx_hal::spi;

use display::{init_frame_buffer, DisplayDriver, DisplayOrientation};
use dht11::Dht11;
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::*;

use ui::{GrayTheme, Label, Screen, TempHumidCard, TempHumidSensor};

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

    let dht_pin = gpioa.pa2.into_open_drain_output();

    let mut dht11 = Dht11::new(dht_pin);

    delay_ms(2000);

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

    // 设置横屏模式
    display.set_orientation(DisplayOrientation::Landscape);

    // 使用 DMA 清屏（白色背景）
    display.clear(Rgb565::WHITE).unwrap();
    display.flush();

    // 创建 UI 屏幕（根据当前方向动态获取尺寸）
    let screen = Screen::new(display.width() as u32, display.height() as u32).with_theme(GrayTheme::new());
    let mut screen = screen;

    // ===== 温湿度传感器测试场景 =====

    let mut temp_sensor = TempHumidSensor::new();
    let mut humid_sensor = TempHumidSensor::new();

    let _ = screen.add_temp_humd_card(TempHumidCard::new(15, 50, true));
    let _ = screen.add_temp_humd_card(TempHumidCard::new(170, 50, false));
    let _ = screen.add_label(Label::new(160, 15, "DHT11 MONITOR").centered());

    screen.draw_with_dma(&mut display).unwrap();

    let mut frame_count: u32 = 0;
    let mut last_dht_read: u32 = 0;

    loop {
        frame_count += 1;

        if frame_count - last_dht_read > 300 {
            last_dht_read = frame_count;

            match dht11.read() {
                Ok(reading) => {
                    let _ = led.toggle();

                    temp_sensor.update_temp(reading.temperature);
                    humid_sensor.update_humid(reading.humidity);

                    let mut temp_card = TempHumidCard::new(15, 50, true);
                    temp_card.sensor = temp_sensor;
                    let mut humid_card = TempHumidCard::new(170, 50, false);
                    humid_card.sensor = humid_sensor;

                    screen.widgets.clear();
                    let _ = screen.add_temp_humd_card(temp_card);
                    let _ = screen.add_temp_humd_card(humid_card);
                    let _ = screen.add_label(Label::new(160, 15, "DHT11 MONITOR").centered());
                    let _ = screen.draw_with_dma(&mut display);
                }
                Err(_) => {
                }
            }
        }

        if frame_count % 60 == 0 {
            let _ = led.toggle();
        }

        delay_ms(16);
    }
}
