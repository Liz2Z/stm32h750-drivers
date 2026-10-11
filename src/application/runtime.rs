//! 应用主循环：编排采集、显示与可选通信。
use super::{
    acquisition::Acquisition,
    config::*,
    connection::{EspLinkState, UploadState},
    navigation::{DemoPage, Navigation},
    presentation,
    rfid::CardReader,
    scheduling::{Periodic, RefreshRate},
};
use crate::{
    bsp::{
        board::Board,
        clock::{delay_ms, Clock},
        display::DisplayDriver,
        rc522::Rc522Spi,
    },
    drivers::{Esp8266Error, HttpEndpoint, SensorPayload},
    ui::{
        rfid_page::{draw_rfid_page, RfidPageData},
        status::{draw_status_with_message, StatusMessage},
        FlowAnimation, GrayTheme, PressureCard, Screen, TempHumidCard,
    },
};
use core::convert::Infallible;

pub fn run(board: Board<'_>, clock: &Clock) -> Infallible {
    let Board {
        mut display,
        mut i2c,
        mut esp8266,
        mut led,
        rc522,
        buttons,
        ..
    } = board;
    let mut sensor_delay = |ms| clock.delay_ms(ms);
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

    let mut display_ready = display.init(&mut sensor_delay).is_ok();
    #[cfg(feature = "display-bench")]
    crate::bsp::display::bench::run(&mut display, clock, board.spi_kernel_hz, display_ready);
    let mut acquisition = Acquisition::default();

    // 横屏三张传感器卡片、顶部动画和底栏状态。
    // 创建屏幕容器
    let mut screen =
        Screen::new(display.width() as u32, display.height() as u32).with_theme(GrayTheme::new());

    // 添加温度卡片（左列）
    let temp_card = TempHumidCard::new(5, 20, true).with_theme(GrayTheme::new());
    let _ = screen.add_temp_humd_card(temp_card);

    // 添加湿度卡片（中列）
    let humid_card = TempHumidCard::new(110, 20, false).with_theme(GrayTheme::new());
    let _ = screen.add_temp_humd_card(humid_card);

    // 添加气压卡片（右列）
    let pressure_card = PressureCard::new(215, 20).with_theme(GrayTheme::new());
    let _ = screen.add_pressure_card(pressure_card);

    let snapshot = acquisition.snapshot();
    screen.update_sensors(
        presentation::climate_card(&snapshot.temperature),
        presentation::climate_card(&snapshot.humidity),
        presentation::pressure_card(&snapshot.pressure),
    );
    if display_ready {
        display_ready = screen.draw_with_dma(&mut display).is_ok();
        screen.clear_dirty();
    }

    const SAMPLE_INTERVAL_US: u32 = 5_000_000;
    let now = clock.now_us();
    // 首次立即采样，之后按硬件时钟期限调度。
    let mut samples = Periodic::new(now.wrapping_sub(SAMPLE_INTERVAL_US), SAMPLE_INTERVAL_US);
    let mut heartbeat = Periodic::new(now, 1_000_000);
    let mut status_refresh = Periodic::new(now.wrapping_sub(1_000_000), 1_000_000);
    let mut refresh_rate = RefreshRate::new(now);
    let mut animation = FlowAnimation::new(now);
    let mut animation_tick = Periodic::new(now, 16_667);
    let mut reader = CardReader::new(rc522, now);
    let mut navigation = Navigation::default();

    loop {
        let page_changed = navigation.poll(buttons.pressed(), clock.now_us());
        let reader_changed = reader.poll(|| clock.now_us());
        refresh_rate.poll(clock.now_us());
        if samples.due(clock.now_us()) {
            let readings = acquisition.sample(&mut i2c, &mut sensor_delay, &mut || clock.now_us());
            let snapshot = acquisition.snapshot();
            screen.update_sensors(
                presentation::climate_card(&snapshot.temperature),
                presentation::climate_card(&snapshot.humidity),
                presentation::pressure_card(&snapshot.pressure),
            );
            if display_ready && navigation.page() == DemoPage::Sensors {
                let has_updates = !screen.dirty_rects.is_empty();
                display_ready = screen.draw_sensor_updates(&mut display).is_ok();
                if display_ready && has_updates {
                    refresh_rate.record_success();
                }
            }

            if ESP8266_ENABLED {
                if let Ok(reading) = readings.climate {
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
                            pressure: snapshot
                                .pressure
                                .values()
                                .map_or(0.0, |values| values.current),
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

        if display_ready
            && (page_changed || (reader_changed && navigation.page() == DemoPage::Rfid))
        {
            display_ready = draw_page(&mut display, &screen, navigation.page(), &reader);
            if display_ready {
                if navigation.page() == DemoPage::Sensors {
                    screen.clear_dirty();
                }
                refresh_rate.record_success();
            }
        }

        if display_ready && animation_tick.due(clock.now_us()) {
            display_ready = animation.refresh(&mut display, clock.now_us()).is_ok();
            if display_ready {
                refresh_rate.record_success();
            }
        }

        if status_refresh.due(clock.now_us()) || page_changed {
            if !display_ready {
                // 局部传输失败后，重新初始化并完整重绘。
                display_ready = display.init(&mut sensor_delay).is_ok()
                    && draw_page(&mut display, &screen, navigation.page(), &reader);
                if display_ready {
                    screen.clear_dirty();
                    refresh_rate.record_success();
                }
            }
            if display_ready {
                draw_status_with_message(
                    &mut display,
                    presentation::status_bar(
                        ESP8266_ENABLED,
                        esp_link_state,
                        esp_upload_state,
                        acquisition.snapshot().sample_count,
                        refresh_rate.centihz(),
                    ),
                    Some(StatusMessage {
                        text: "K1: Up   K2: Down",
                        tone: crate::ui::status::StatusTone::Muted,
                    }),
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

fn draw_page(
    display: &mut DisplayDriver<'_>,
    screen: &Screen,
    page: DemoPage,
    reader: &CardReader<Rc522Spi<'_>>,
) -> bool {
    match page {
        DemoPage::Sensors => screen.draw_with_dma(display).is_ok(),
        DemoPage::Rfid => {
            let status = presentation::reader_status(reader.state);
            let uid = presentation::uid_text(reader.last_card.as_ref().map(|card| &card.uid));
            let protocol = presentation::card_protocol(reader.last_card.as_ref());
            let memory = presentation::memory_progress(reader.memory.as_ref());
            let access = presentation::memory_access(reader.memory.as_ref());
            draw_rfid_page(
                display,
                RfidPageData {
                    status: StatusMessage {
                        text: status.text,
                        tone: status.tone,
                    },
                    uid: uid.as_str(),
                    uid_caption: if reader.state == super::rfid::ReaderState::Present {
                        "Card UID"
                    } else {
                        "Last UID"
                    },
                    card_type: reader
                        .last_card
                        .as_ref()
                        .map_or("Hold one card near the antenna", |card| card.kind()),
                    protocol: protocol.as_str(),
                    memory: memory.as_str(),
                    access: access.as_str(),
                },
            )
            .is_ok()
                && display.flush_rect(0, 20, 320, 200).is_ok()
        },
    }
}
