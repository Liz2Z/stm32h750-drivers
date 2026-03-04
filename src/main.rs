//! # DHT11 温湿度监控器
//!
//! 这是一个基于 STM32H750 的温湿度监控应用。
//! 它从 DHT11 传感器读取温湿度数据，并在 ILI9341 屏幕上实时显示。
//!
//! ## 应用场景
//!
//! - 室内环境监测
//! - 智能家居温湿度显示
//! - 温室大棚监控
//! - 电子设备散热监控
//!
//! ## 功能特点
//!
//! 1. **实时监测**：每 5 秒读取一次温湿度数据
//! 2. **可视化显示**：温度和湿度分别以卡片形式展示
//! 3. **状态指示**：LED 闪烁表示系统正常运行
//!
//! ## 硬件配置
//!
//! ### DHT11 温湿度传感器
//! | 引脚 | 连接 | 说明 |
//! |------|------|------|
//! | PA2 | DATA | 数据引脚（需要 4.7K 上拉电阻）|
//! | VCC | 3.3V/5V | 电源 |
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

/// 软件延时函数
///
/// 用于系统启动阶段和主循环中的帧率控制。
/// 这是一个粗略的延时，精度足够用于：
/// - DHT11 通信时序
/// - 屏幕初始化延时
/// - 帧率控制
fn delay_ms(ms: u32) {
    for _ in 0..ms {
        for _ in 0..8000 {
            cortex_m::asm::nop();
        }
    }
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

    // DHT11 数据引脚
    // 配置为开漏输出模式，因为 DHT11 的数据线是双向的：
    // - 主机发送启动信号时是输出
    // - 传感器返回数据时需要读取输入
    // 开漏模式配合上拉电阻可以实现双向通信
    let dht_pin = gpioa.pa2.into_open_drain_output();
    let mut dht11 = Dht11::new(dht_pin);

    // DHT11 上电后需要稳定时间
    // 这段时间传感器在进行内部校准
    delay_ms(2000);

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
    init_frame_buffer();

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
    // 第四阶段：创建 UI 界面
    // ============================================================
    //
    // UI 布局（横屏 320x240）：
    // ┌────────────────────────────────────┐
    // │         DHT11 MONITOR              │ 标题
    // ├───────────────┬────────────────────┤
    // │   温度卡片     │     湿度卡片       │
    // │   25.5°C      │      60%           │
    // │   hi:28°C     │      hi:65%        │
    // │   lo:22°C     │      lo:55%        │
    // └───────────────┴────────────────────┘

    // 创建屏幕容器
    let screen = Screen::new(display.width() as u32, display.height() as u32)
        .with_theme(GrayTheme::new());
    let mut screen = screen;

    // 创建传感器数据存储
    // 这些结构体会记录：
    // - 当前值
    // - 历史最高值
    // - 历史最低值
    let mut temp_sensor = TempHumidSensor::new();
    let mut humid_sensor = TempHumidSensor::new();

    // 添加温度卡片（左侧）
    let _ = screen.add_temp_humd_card(TempHumidCard::new(15, 50, true));

    // 添加湿度卡片（右侧）
    let _ = screen.add_temp_humd_card(TempHumidCard::new(170, 50, false));

    // 添加标题
    let _ = screen.add_label(Label::new(160, 15, "DHT11 MONITOR").centered());

    // 绘制初始界面
    screen.draw_with_dma(&mut display).unwrap();

    // ============================================================
    // 第五阶段：主循环
    // ============================================================
    //
    // 主循环负责：
    // 1. 定期读取 DHT11 传感器数据
    // 2. 更新 UI 显示
    // 3. LED 状态指示

    let mut frame_count: u32 = 0;
    let mut last_dht_read: u32 = 0;

    loop {
        frame_count += 1;

        // 每 5 秒读取一次传感器
        // 300 帧 × 16ms ≈ 4.8 秒
        //
        // 为什么是 5 秒？
        // - DHT11 最小读取间隔是 1-2 秒
        // - 温湿度变化较慢，5 秒更新一次足够
        // - 降低读取频率可以减少失败率
        if frame_count - last_dht_read > 300 {
            last_dht_read = frame_count;

            match dht11.read() {
                Ok(reading) => {
                    // 读取成功！
                    // LED 闪烁一次表示数据更新
                    let _ = led.toggle();

                    // 更新传感器数据
                    // update_temp/update_humid 会自动更新最高/最低值
                    temp_sensor.update_temp(reading.temperature);
                    humid_sensor.update_humid(reading.humidity);

                    // 重新创建卡片控件（带新数据）
                    let mut temp_card = TempHumidCard::new(15, 50, true);
                    temp_card.sensor = temp_sensor;
                    let mut humid_card = TempHumidCard::new(170, 50, false);
                    humid_card.sensor = humid_sensor;

                    // 更新屏幕显示
                    screen.widgets.clear();
                    let _ = screen.add_temp_humd_card(temp_card);
                    let _ = screen.add_temp_humd_card(humid_card);
                    let _ = screen.add_label(Label::new(160, 15, "DHT11 MONITOR").centered());
                    let _ = screen.draw_with_dma(&mut display);
                }
                Err(_) => {
                    // 读取失败
                    // 这在 DHT11 中很常见，可能原因：
                    // - 时序偏差
                    // - 电磁干扰
                    // - 传感器暂时忙碌
                    // 我们简单地忽略这次失败，下次继续尝试
                }
            }
        }

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
