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
//! | PB6 | SCL | I2C 时钟线 |
//! | VCC | 3.3V | 电源 |
//! | GND | GND | 地 |
//!
//! ### BMP280 气压传感器（I2C，与 AHT20 共享总线）
//! | 引脚 | 连接 | 说明 |
//! |------|------|------|
//! | PB7 | SDA | I2C 数据线（共享）|
//! | PB6 | SCL | I2C 时钟线（共享）|
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

mod clock;
mod drivers;
mod profiler;
use stm32h750_drivers::ui;

use cortex_m_rt::entry;
use panic_halt as _;
use stm32h7xx_hal::pac;
use stm32h7xx_hal::prelude::*;
use stm32h7xx_hal::serial::SerialExt;
use stm32h7xx_hal::spi;

use clock::Clock;
use drivers::bounded_i2c::BoundedI2c;
use drivers::{
    DisplayDriver, DisplayOrientation, Esp8266, Esp8266Error, HttpEndpoint, SensorPayload,
};
use embedded_graphics::pixelcolor::Rgb565;
use heapless::String;
use stm32h750_drivers::{
    acquisition::Sensors,
    scheduling::{Periodic, RefreshRate},
};

use ui::{GrayTheme, PressureCard, PressureSensor, Screen, TempHumidCard, TempHumidSensor};

// 当前只验证显示和采集；避免未配置的 ESP 阻塞主循环。
const ESP8266_ENABLED: bool = false;
const ESP8266_UART_BAUD: u32 = 115_200;
const ESP8266_WIFI_SSID: &str = "quantum-wifi";
const ESP8266_WIFI_PASSWORD: &str = "qwerzxc123!@#";
const ESP8266_HTTP_HOST: &str = "";
const ESP8266_HTTP_PORT: u16 = 80;
const ESP8266_HTTP_PATH: &str = "/sensor";

#[derive(Clone, Copy)]
enum UploadState {
    Disabled,
    Idle,
    Ok,
    Failed,
}

#[derive(Clone, Copy)]
enum EspLinkState {
    SerialReady,
    WifiConnected,
    Failed,
}

/// 软件延时函数
///
/// 用于现有的系统启动和 ESP8266 阻塞流程。
/// STM32H750 主频 400MHz，1ms = 400,000 个 CPU 周期
/// 启动与 ESP 代码沿用周期延时；传感器和调度使用 TIM2。
fn delay_ms(ms: u32) {
    cortex_m::asm::delay(ms * 400_000);
}

fn draw_status<D>(
    display: &mut D,
    link_state: EspLinkState,
    upload_state: UploadState,
    sample_count: u32,
    refresh_centihz: Option<u32>,
) where
    D: embedded_graphics::prelude::DrawTarget<Color = embedded_graphics::pixelcolor::Rgb565>,
{
    use embedded_graphics::{
        mono_font::{ascii::FONT_6X10, MonoTextStyle},
        prelude::*,
        primitives::{PrimitiveStyle, Rectangle},
        text::{Baseline, Text},
    };

    let status_bg = Rgb565::new(26, 58, 74);
    let wifi_color = match link_state {
        EspLinkState::WifiConnected => Rgb565::new(0, 40, 0),
        EspLinkState::Failed => Rgb565::new(31, 8, 8),
        _ => Rgb565::BLACK,
    };
    let upload_color = match upload_state {
        UploadState::Ok => Rgb565::new(0, 40, 0),
        UploadState::Failed => Rgb565::new(31, 8, 8),
        UploadState::Disabled => Rgb565::new(14, 18, 20),
        UploadState::Idle => Rgb565::BLACK,
    };

    let _ = Rectangle::new(Point::new(0, 220), Size::new(320, 20))
        .into_styled(PrimitiveStyle::with_fill(status_bg))
        .draw(display);

    let wifi_style = MonoTextStyle::new(&FONT_6X10, wifi_color);
    let upload_style = MonoTextStyle::new(&FONT_6X10, upload_color);
    let frame_style = MonoTextStyle::new(&FONT_6X10, Rgb565::BLACK);

    if !ESP8266_ENABLED {
        let mut text: String<24> = String::new();
        let _ = core::fmt::write(&mut text, format_args!("Samples: {}", sample_count));
        let _ = Text::with_baseline(
            text.as_str(),
            Point::new(4, 225),
            frame_style,
            Baseline::Top,
        )
        .draw(display);
        let mut rate_text: String<32> = String::new();
        if let Some(rate) = refresh_centihz {
            let _ = core::fmt::write(
                &mut rate_text,
                format_args!("Refresh: {}.{:02} Hz", rate / 100, rate % 100),
            );
        } else {
            let _ = rate_text.push_str("Refresh: -- Hz");
        }
        let _ = Text::with_baseline(
            rate_text.as_str(),
            Point::new(190, 225),
            frame_style,
            Baseline::Top,
        )
        .draw(display);
        return;
    }

    let wifi_text = match link_state {
        EspLinkState::SerialReady => "ESP: AT ready",
        EspLinkState::WifiConnected => "WiFi: connected",
        EspLinkState::Failed => "ESP: failed",
    };
    let upload_text = match upload_state {
        UploadState::Disabled => "Upload: disabled",
        UploadState::Idle => "Upload: idle",
        UploadState::Ok => "Upload: ok",
        UploadState::Failed => "Upload: failed",
    };

    let mut frame_buf: String<24> = String::new();
    let _ = core::fmt::write(&mut frame_buf, format_args!("Samples: {}", sample_count));

    let _ =
        Text::with_baseline(wifi_text, Point::new(4, 225), wifi_style, Baseline::Top).draw(display);
    let _ = Text::with_baseline(
        upload_text,
        Point::new(110, 225),
        upload_style,
        Baseline::Top,
    )
    .draw(display);
    let _ = Text::with_baseline(
        frame_buf.as_str(),
        Point::new(210, 225),
        frame_style,
        Baseline::Top,
    )
    .draw(display);
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

    let clock = Clock(
        dp.TIM2
            .tick_timer(1.MHz(), ccdr.peripheral.TIM2, &ccdr.clocks),
    );
    let mut sensor_delay = |ms| clock.delay_ms(ms);

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

    // ESP8266-01S 串口（USART3）
    // PB10 -> ESP RX
    // PB11 <- ESP TX
    let esp_tx_pin = gpiob.pb10.into_alternate::<7>();
    let esp_rx_pin = gpiob.pb11.into_alternate::<7>();
    let esp_serial = dp
        .USART3
        .serial(
            (esp_tx_pin, esp_rx_pin),
            ESP8266_UART_BAUD.bps(),
            ccdr.peripheral.USART3,
            &ccdr.clocks,
        )
        .unwrap();
    let (esp_tx, esp_rx) = esp_serial.split();
    let mut esp8266 = Esp8266::new(esp_tx, esp_rx);
    let mut esp_ready = false;
    let mut esp_wifi_connected = false;
    let upload_endpoint = HttpEndpoint {
        host: ESP8266_HTTP_HOST,
        port: ESP8266_HTTP_PORT,
        path: ESP8266_HTTP_PATH,
    };
    let mut esp_upload_enabled =
        !upload_endpoint.host.is_empty() && !upload_endpoint.path.is_empty();
    let mut esp_upload_state = if esp_upload_enabled {
        UploadState::Idle
    } else {
        UploadState::Disabled
    };

    let mut esp_link_state = if ESP8266_ENABLED {
        match esp8266.initialize(&mut delay_ms, ESP8266_WIFI_SSID, ESP8266_WIFI_PASSWORD) {
            Ok(status) => {
                esp_ready = true;
                esp_wifi_connected = status.connected;
                if status.connected {
                    EspLinkState::WifiConnected
                } else {
                    EspLinkState::SerialReady
                }
            },
            Err(_) => {
                // 允许系统先跑起来，后续在主循环中继续重试连接 ESP
                EspLinkState::Failed
            },
        }
    } else {
        EspLinkState::SerialReady
    };

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
    let disp_dc = gpiob.pb1.into_push_pull_output(); // 数据/命令
    let disp_cs = gpiob.pb12.into_push_pull_output(); // 片选

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
        led.toggle();
        sensor_delay(50);
    }

    // 打开屏幕背光
    disp_blk.set_high();

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

    // The display owns and initializes its AXISRAM buffers once.
    let mut display = DisplayDriver::with_orientation(
        spi,
        disp_cs,
        disp_dc,
        &clock,
        DisplayOrientation::Landscape,
    );
    let mut display_ready = display.init(&mut sensor_delay).is_ok();

    // ============================================================
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

    let mut i2c = BoundedI2c::new(i2c, &clock);
    let mut sensors = Sensors::default();

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
    let mut screen =
        Screen::new(display.width() as u32, display.height() as u32).with_theme(GrayTheme::new());

    // 创建传感器数据存储
    // 这些结构体会记录：
    // - 当前值
    // - 历史最高值
    // - 历史最低值
    let mut temp_sensor = TempHumidSensor::new();
    let mut humid_sensor = TempHumidSensor::new();
    let mut pressure_sensor = PressureSensor::new();

    // 添加温度卡片（左列）
    let temp_card = TempHumidCard::new(5, 20, true).with_theme(GrayTheme::new());
    let _ = screen.add_temp_humd_card(temp_card);

    // 添加湿度卡片（中列）
    let humid_card = TempHumidCard::new(110, 20, false).with_theme(GrayTheme::new());
    let _ = screen.add_temp_humd_card(humid_card);

    // 添加气压卡片（右列）
    let pressure_card = PressureCard::new(215, 20).with_theme(GrayTheme::new());
    let _ = screen.add_pressure_card(pressure_card);

    screen.update_sensors(temp_sensor, humid_sensor, pressure_sensor);
    if display_ready {
        display_ready = screen.draw_with_dma(&mut display).is_ok();
        screen.clear_dirty();
    }

    const SAMPLE_INTERVAL_US: u32 = 5_000_000;
    let now = clock.now_us();
    // Take the first sample immediately, then keep hardware-clock deadlines.
    let mut samples = Periodic::new(now.wrapping_sub(SAMPLE_INTERVAL_US), SAMPLE_INTERVAL_US);
    let mut heartbeat = Periodic::new(now, 1_000_000);
    let mut status_refresh = Periodic::new(now.wrapping_sub(1_000_000), 1_000_000);
    let mut sample_count: u32 = 0;
    let mut refresh_rate = RefreshRate::new(now);

    loop {
        refresh_rate.poll(clock.now_us());
        if samples.due(clock.now_us()) {
            sample_count = sample_count.wrapping_add(1);
            let readings = sensors.sample(&mut i2c, &mut sensor_delay);
            let aht_result = readings.climate;
            match aht_result {
                Ok(reading) => {
                    temp_sensor.update_temp(reading.temperature);
                    humid_sensor.update_humid(reading.humidity);
                    let now = clock.now_us();
                    temp_sensor.mark_updated(now);
                    humid_sensor.mark_updated(now);
                },
                Err(_) => {
                    temp_sensor.mark_failed();
                    humid_sensor.mark_failed();
                },
            }

            let bmp_result = readings.pressure;
            match bmp_result {
                Ok(reading) => {
                    pressure_sensor.update(reading.pressure);
                    pressure_sensor.mark_updated(clock.now_us());
                },
                Err(_) => {
                    pressure_sensor.mark_failed();
                },
            }
            screen.update_sensors(temp_sensor, humid_sensor, pressure_sensor);
            if display_ready {
                let has_updates = !screen.dirty_rects.is_empty();
                display_ready = screen.draw_sensor_updates(&mut display).is_ok();
                if display_ready && has_updates {
                    refresh_rate.record_success();
                }
            }

            if ESP8266_ENABLED {
                if let Ok(reading) = aht_result {
                    if !esp_ready {
                        if let Ok(status) = esp8266.initialize(
                            &mut delay_ms,
                            ESP8266_WIFI_SSID,
                            ESP8266_WIFI_PASSWORD,
                        ) {
                            esp_ready = true;
                            esp_wifi_connected = status.connected;
                            esp_link_state = if status.connected {
                                EspLinkState::WifiConnected
                            } else {
                                EspLinkState::SerialReady
                            };
                            esp_upload_state = if esp_upload_enabled {
                                UploadState::Idle
                            } else {
                                UploadState::Disabled
                            };
                        } else {
                            esp_link_state = EspLinkState::Failed;
                        }
                    }

                    if esp_ready && esp_upload_enabled {
                        let payload = SensorPayload {
                            temperature: reading.temperature,
                            humidity: reading.humidity,
                            pressure: pressure_sensor.pressure_current,
                        };

                        match esp8266.send_sensor_data(&mut delay_ms, upload_endpoint, payload) {
                            Ok(_) => {
                                esp_upload_state = UploadState::Ok;
                                if let Ok(status) = esp8266.query_wifi(&mut delay_ms) {
                                    esp_wifi_connected = status.connected;
                                    esp_link_state = if status.connected {
                                        EspLinkState::WifiConnected
                                    } else {
                                        EspLinkState::SerialReady
                                    };
                                }
                            },
                            Err(Esp8266Error::InvalidConfig) => {
                                esp_upload_enabled = false;
                                esp_upload_state = UploadState::Disabled;
                            },
                            Err(_) => {
                                esp_ready = false;
                                esp_wifi_connected = false;
                                esp_link_state = EspLinkState::Failed;
                                esp_upload_state = UploadState::Failed;
                            },
                        }
                    }
                }
            }
        }

        if status_refresh.due(clock.now_us()) {
            if !display_ready {
                // A failed partial transfer is retried as a full initialization/frame.
                display_ready = display.init(&mut sensor_delay).is_ok()
                    && screen.draw_with_dma(&mut display).is_ok();
                if display_ready {
                    screen.clear_dirty();
                    refresh_rate.record_success();
                }
            }
            if display_ready {
                draw_status(
                    &mut display,
                    esp_link_state,
                    esp_upload_state,
                    sample_count,
                    refresh_rate.centihz(),
                );
                display_ready = display.flush_rect(0, 220, 320, 20).is_ok();
            }
        }
        if heartbeat.due(clock.now_us()) {
            if !ESP8266_ENABLED || esp_wifi_connected {
                led.toggle();
            } else {
                led.set_low();
            }
        }
        sensor_delay(1);
    }
}
