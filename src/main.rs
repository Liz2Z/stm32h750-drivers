//! # 环境监测显示器
//!
//! 这是一个基于 STM32H750 的环境监测应用。
//! 它从 AHT20 传感器读取温湿度数据，从 BMP280 读取气压数据，并在 ILI9341 屏幕上实时显示。
//!
//! ## 应用场景
//!
//! - 室内环境监测
//! - 智能家居温湿度气压显示
//! - 温室大棚监控
//! - 电子设备散热监控
//! - 天气预报辅助
//!
//! ## 功能特点
//!
//! 1. **实时监测**：定期读取温湿度和气压数据
//! 2. **可视化显示**：温度、湿度、气压分别以卡片形式展示
//! 3. **状态指示**：LED 闪烁表示系统正常运行
//!
//! ## 硬件配置
//!
//! ### AHT20 温湿度传感器（I2C）
//! | 引脚 | 连接 | 说明 |
//! |------|------|------|
//! | PB7 | SDA | I2C 数据线 |
//! | PB8 | SCL | I2C 时钟线 |
//! | VCC | 3.3V | 电源 |
//! | GND | GND | 地 |
//!
//! ### BMP280 气压传感器（I2C，与 AHT20 共享总线）
//! | 引脚 | 连接 | 说明 |
//! |------|------|------|
//! | PB7 | SDA | I2C 数据线（共享）|
//! | PB8 | SCL | I2C 时钟线（共享）|
//! | VCC | 3.3V | 电源 |
//! | GND | GND | 地 |
//!
//! ### ILI9341 TFT 屏幕（320x240 横屏）
//! | 引脚 | 连接 | 说明 |
//! |------|------|------|
//! | PB15 | MOSI | SPI 数据输出 |
//! | PB13 | SCK | SPI 时钟 |
//! | PB12 | CS | 片选 |
//! | PB14 | MISO | SPI 数据输入 |
//! | PB1 | DC | 数据/命令选择 |
//! | PB0 | BLK | 背光控制 |
//!
//! ### LED 状态指示
//! | 引脚 | 说明 |
//! |------|------|
//! | PA1 | 状态 LED（读取成功时闪烁）|

#![no_std]
#![no_main]

mod drivers;
mod profiler;
mod ui;

use cortex_m_rt::entry;
use panic_halt as _;
use stm32h7xx_hal::pac;
use stm32h7xx_hal::prelude::*;
use stm32h7xx_hal::spi;

use drivers::{aht20, bmp280, display, DisplayDriver, DisplayOrientation};
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::*;

use ui::{GrayTheme, Label, PressureCard, PressureSensor, Screen, TempHumidCard, TempHumidSensor};

/// 软件延时函数
///
/// 用于系统启动阶段和主循环中的帧率控制。
/// STM32H750 主频 400MHz，1ms = 400,000 个 CPU 周期
/// 使用 cortex_m::asm::delay 精确消耗周期数
fn delay_ms(ms: u32) {
    cortex_m::asm::delay(ms * 400_000);
}

/// 应用程序入口
///
/// 系统启动后从这里开始执行，主要流程：
/// 1. 初始化硬件（时钟、GPIO、SPI）
/// 2. 初始化屏幕显示
/// 3. 创建 UI 界面
/// 4. 进入主循环，定期读取传感器并更新显示
#[entry]
fn main() -> ! {
    // ============================================================
    // 第一阶段：系统初始化
    // ============================================================
    //
    // 这一步配置 MCU 的基本运行环境：
    // - 电源模式：VOS1（最高性能模式）
    // - 主频：400MHz（STM32H750 的最高频率）
    // - 时钟树：配置各个总线和外设的时钟

    let dp = pac::Peripherals::take().unwrap();

    // 配置电源和时钟
    let pwr = dp.PWR.constrain();
    let pwrcfg = pwr.vos1().freeze(); // VOS1 = 最高性能

    let rcc = dp.RCC.constrain();
    let ccdr = rcc
        .sys_ck(400.MHz())
        .pll1_q_ck(400.MHz())
        .freeze(pwrcfg, &dp.SYSCFG);

    // ============================================================
    // 第二阶段：GPIO 配置
    // ============================================================
    //
    // 配置各个引脚的功能：
    // - LED：状态指示
    // - DHT11：温湿度传感器数据线
    // - 屏幕：SPI 接口和控制信号

    let gpioa = dp.GPIOA.split(ccdr.peripheral.GPIOA);
    let gpiob = dp.GPIOB.split(ccdr.peripheral.GPIOB);

    // LED 指示灯 - 用于显示系统状态
    // 正常运行时会闪烁，读取传感器成功时也会闪烁
    let mut led = gpioa.pa1.into_push_pull_output();

    // ============================================================
    // 第三阶段：屏幕初始化
    // ============================================================
    //
    // 配置 ILI9341 TFT LCD 屏幕：
    // - SPI 接口：80MHz，用于高速数据传输
    // - 横屏模式：320x240 分辨率
    // - 帧缓冲：存储待显示的图像数据

    // 屏幕控制引脚
    let mut disp_blk = gpiob.pb0.into_push_pull_output(); // 背光
    let disp_dc = gpiob.pb1.into_push_pull_output();      // 数据/命令
    let disp_cs = gpiob.pb12.into_push_pull_output();     // 片选

    // SPI 引脚配置
    // 使用 VeryHigh 速度模式以支持 80MHz SPI 时钟
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
        let _ = led.toggle();
        delay_ms(50);
    }

    // 打开屏幕背光
    let _ = disp_blk.set_high();

    // 初始化 SPI2
    // 使用 MODE_3（CPOL=1, CPHA=1），这是 ILI9341 要求的模式
    // 80MHz 时钟可以实现约 10fps 的全屏刷新
    let spi = dp.SPI2.spi(
        (disp_sck, disp_miso, disp_mosi),
        spi::MODE_3,
        80.MHz(),
        ccdr.peripheral.SPI2,
        &ccdr.clocks,
    );

    // 初始化帧缓冲区
    // 帧缓冲位于 AXISRAM（D2 域的 SRAM），DMA 可以直接访问
    display::init_frame_buffer();

    // 创建并初始化显示驱动
    let mut display = DisplayDriver::new(spi, disp_cs, disp_dc);
    display.init(&mut delay_ms);

    // 设置横屏模式
    // 横屏更适合显示温湿度信息，可以并排显示两个卡片
    display.set_orientation(DisplayOrientation::Landscape);

    // 清屏为白色背景
    display.clear(Rgb565::WHITE).unwrap();
    display.flush();

    // ============================================================
    // 第四阶段：传感器初始化
    // ============================================================

    // I2C1 引脚配置 (用于 AHT20 + BMP280 传感器)
    // PB6 = I2C1_SCL, PB7 = I2C1_SDA
    let i2c_scl = gpiob
        .pb6
        .into_alternate::<4>()
        .set_open_drain();
    let i2c_sda = gpiob
        .pb7
        .into_alternate::<4>()
        .set_open_drain();

    // 初始化 I2C1 (100kHz)
    let i2c = dp.I2C1.i2c(
        (i2c_scl, i2c_sda),
        100.kHz(),
        ccdr.peripheral.I2C1,
        &ccdr.clocks,
    );

    // 初始化 AHT20 温湿度传感器
    let mut aht20 = aht20::Aht20::new(i2c);
    let mut aht20_ok = false;
    
    match aht20.init() {
        Ok(_) => {
               let screen = Screen::new(display.width() as u32, display.height() as u32)
                        .with_theme(GrayTheme::new());
                    let mut screen = screen;
                    
                    let _ = screen.add_label(Label::new(160, 80, "AHT20 INIT SUCCESS").centered());
                    let _ = screen.draw_with_dma(&mut
                         display);
                    delay_ms(1000);
            // AHT20 初始化成功，LED 闪烁一次
            aht20_ok = true;
            let _ = led.set_high();
            delay_ms(100);
            let _ = led.set_low();
        }
        Err(_) => {
            // AHT20 初始化失败，在屏幕上显示错误
            let screen = Screen::new(display.width() as u32, display.height() as u32)
                .with_theme(GrayTheme::new());
            let mut screen = screen;
            
            let _ = screen.add_label(Label::new(160, 100, "AHT20 INIT FAILED").centered());
            let _ = screen.add_label(Label::new(160, 130, "Check I2C Connection").centered());
            let _ = screen.draw_with_dma(&mut display);
            
            // LED 快闪 3 次
            for _ in 0..3 {
                let _ = led.set_high();
                delay_ms(100);
                let _ = led.set_low();
                delay_ms(100);
            }
        }
    }

    // 初始化 BMP280 气压传感器
    // BMP280 有两个可能的 I2C 地址：0x76 和 0x77
    let i2c = aht20.release();
    let mut bmp280 = bmp280::Bmp280::new(i2c);
    let mut bmp280_ok = false;
    
    match bmp280.init() {
        Ok(_) => {
            // BMP280 初始化成功
            bmp280_ok = true;
            for _ in 0..2 {
                let _ = led.set_high();
                delay_ms(100);
                let _ = led.set_low();
                delay_ms(100);
            }
        }
        Err(_) => {
            // BMP280 初始化失败，在屏幕上显示错误
            let screen = Screen::new(display.width() as u32, display.height() as u32)
                .with_theme(GrayTheme::new());
            let mut screen = screen;
            
            let _ = screen.add_label(Label::new(160, 80, "BMP280 INIT FAILED").centered());
            let _ = screen.add_label(Label::new(160, 110, "Check I2C Wiring").centered());
            let _ = screen.draw_with_dma(&mut display);
            
            // LED 快闪 4 次
            for _ in 0..4 {
                let _ = led.set_high();
                delay_ms(100);
                let _ = led.set_low();
                delay_ms(100);
            }
        }
    }

    // 释放 I2C 并重新创建 AHT20 实例用于后续读取
    // 注意：Bmp280 的 release() 现在不会销毁实例，校准数据得以保留
    let i2c = bmp280.release();
    aht20 = aht20::Aht20::new(i2c);

    // ============================================================
    // 第五阶段：创建 UI 界面
    // ============================================================
    //
    // UI 布局（横屏 320x240）：
    // ┌────────────────────────────────────┐
    // │         AHT20+BMP280               │ 标题
    // ├───────────────┬────────────────────┤
    // │   温度卡片     │     湿度卡片       │
    // │   25.5°C      │      60%           │
    // │   hi:28°C     │      hi:65%        │
    // │   lo:22°C     │      lo:55%        │
    // └───────────────┴────────────────────┘

    // 创建屏幕容器
    let mut screen = Screen::new(display.width() as u32, display.height() as u32)
        .with_theme(GrayTheme::new());

    // 创建传感器数据存储
    // 这些结构体会记录：
    // - 当前值
    // - 历史最高值
    // - 历史最低值
    let mut temp_sensor = TempHumidSensor::new();
    let mut humid_sensor = TempHumidSensor::new();
    let mut pressure_sensor = PressureSensor::new();

    // 添加温度卡片（左列）
    let temp_card = TempHumidCard::new(5, 20, true)
        .with_theme(GrayTheme::new());
    let _ = screen.add_temp_humd_card(temp_card);

    // 添加湿度卡片（中列）
    let humid_card = TempHumidCard::new(110, 20, false)
        .with_theme(GrayTheme::new());
    let _ = screen.add_temp_humd_card(humid_card);

    // 添加气压卡片（右列）
    let pressure_card = PressureCard::new(215, 20)
        .with_theme(GrayTheme::new());
    let _ = screen.add_pressure_card(pressure_card);


    // 绘制初始界面
    screen.draw_with_dma(&mut display).unwrap();

    // ============================================================
    // 第六阶段：主循环
    // ============================================================
    //
    // 主循环负责：
    // 1. 定期读取 AHT20 传感器数据
    // 2. 更新 UI 显示
    // 3. LED 状态指示

    let mut frame_count: u32 = 0;
    let mut last_sensor_read: u32 = 0;

    fn draw_frame_count<D>(display: &mut D, frame: u32)
    where
        D: embedded_graphics::prelude::DrawTarget<Color = embedded_graphics::pixelcolor::Rgb565>,
    {
        use embedded_graphics::{
            mono_font::{ascii::FONT_10X20, MonoTextStyle},
            prelude::*,
            text::{Baseline, Text},
        };
        
        let mut buf = [0u8; 12];
        let mut len = 0;
        
        if frame == 0 {
            buf[0] = b'0';
            len = 1;
        } else {
            let mut temp = frame;
            let mut temp_len = 0;
            while temp > 0 {
                temp_len += 1;
                temp /= 10;
            }
            
            len = temp_len;
            let mut n = frame;
            while n > 0 {
                len -= 1;
                buf[len] = b'0' + (n % 10) as u8;
                n /= 10;
            }
            len = temp_len;
        }
        
        let frame_str = unsafe {
            core::str::from_utf8_unchecked(&buf[..len])
        };
        
        let style = MonoTextStyle::new(&FONT_10X20, embedded_graphics::pixelcolor::Rgb565::BLACK);
        let text = Text::with_baseline(
            frame_str,
            Point::new(280, 210),
            style,
            Baseline::Top,
        );
        
        let _ = text.draw(display);
    }

    loop {
        frame_count += 1;

        // 每 5 秒读取一次传感器
        // 300 帧 × 16ms ≈ 4.8 秒
        if frame_count - last_sensor_read > 300 {
            last_sensor_read = frame_count;

            // 读取 AHT20 温湿度
            match aht20.read() {
                Ok(reading) => {
                    // 读取成功！
                    // LED 闪烁一次表示数据更新
                    let _ = led.toggle();

                    // 更新传感器数据
                    temp_sensor.update_temp(reading.temperature);
                    humid_sensor.update_humid(reading.humidity);

                    // 释放 AHT20 的 I2C 总线，重新挂载到依然存活的 bmp280 实例上
                    let i2c = aht20.release();
                    bmp280.attach(i2c);
                    
                    // 读取 BMP280 气压（此时它的 is_initialized 是 true，且包含全部校准数据）
                    match bmp280.read() {
                        Ok(bmp_reading) => {
                            // 更新气压数据
                            pressure_sensor.update(bmp_reading.pressure);
                        }
                        Err(_) => {
                            // BMP280 读取失败，保持上次的值
                        }
                    }

                    // 用完后释放出来，再交还给 AHT20
                    let i2c = bmp280.release();
                    aht20 = aht20::Aht20::new(i2c);

                    // 更新屏幕显示：先清空控件，然后重新添加带新数据的卡片
                    screen.widgets.clear();
                    
                    // 创建带新数据的卡片（3列布局）
                    let temp_card = TempHumidCard::new(5, 20, true)
                        .with_theme(GrayTheme::new());
                    let mut temp_card_with_data = temp_card;
                    temp_card_with_data.sensor = temp_sensor;
                    
                    let humid_card = TempHumidCard::new(110, 20, false)
                        .with_theme(GrayTheme::new());
                    let mut humid_card_with_data = humid_card;
                    humid_card_with_data.sensor = humid_sensor;
                    
                    let pressure_card = PressureCard::new(215, 20)
                        .with_theme(GrayTheme::new());
                    let mut pressure_card_with_data = pressure_card;
                    pressure_card_with_data.sensor = pressure_sensor;
                    
                    let _ = screen.add_temp_humd_card(temp_card_with_data);
                    let _ = screen.add_temp_humd_card(humid_card_with_data);
                    let _ = screen.add_pressure_card(pressure_card_with_data);
                    let _ = screen.draw_with_dma(&mut display);
                }
                Err(e) => {
                    // AHT20 读取失败，在屏幕上显示错误
                    screen.widgets.clear();
                    
                    // 显示具体的错误信息
                    let error_msg = match e {
                        aht20::Aht20Error::I2cError => "I2C ERROR",
                        aht20::Aht20Error::NotCalibrated => "NOT CALIBRATED",
                        aht20::Aht20Error::Busy => "SENSOR BUSY",
                        aht20::Aht20Error::InvalidData => "INVALID DATA",
                    };
                    
                    let _ = screen.add_label(Label::new(160, 80, "AHT20 READ ERROR").centered());
                    let _ = screen.add_label(Label::new(160, 110, error_msg).centered());
                    let _ = screen.draw_with_dma(&mut display);
                    
                    // LED 快闪表示错误
                    for _ in 0..3 {
                        let _ = led.toggle();
                        delay_ms(100);
                    }
                }
            }
        }

        // 在屏幕右下角显示 frame_count
        draw_frame_count(&mut display, frame_count);

        // 心跳指示
        // LED 每秒闪烁一次，表示系统正常运行
        if frame_count % 60 == 0 {
            let _ = led.toggle();
        }

        // 帧率控制
        // 约 60fps，提供流畅的视觉体验
        delay_ms(16);
    }
}
