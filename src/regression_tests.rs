use crate::{
    application::acquisition::{Acquisition, SampleState},
    application::presentation,
    application::scheduling::{Periodic, RefreshRate},
    drivers::{
        aht20::{Aht20, Aht20Error},
        bmp280::{Bmp280, Bmp280Error},
    },
    ui::{FlowAnimation, FlushTarget, PressureCard, Screen, TempHumidCard},
};
use embedded_graphics::{pixelcolor::Rgb565, prelude::*};
use embedded_hal::blocking::i2c::{Read, Write, WriteRead};
use std::{vec, vec::Vec};

#[test]
fn acquisition_snapshots_map_failure_and_recovery_without_exposing_records() {
    use crate::ui::ValueStatus;
    let mut bus = SensorBus::new();
    let mut acquisition = Acquisition::default();
    let initial = acquisition.snapshot();
    assert_eq!(initial.sample_count, 0);
    assert_eq!(initial.temperature.state(), SampleState::Pending);
    assert_eq!(
        presentation::climate_card(&initial.temperature).status,
        ValueStatus::Waiting
    );
    assert_eq!(presentation::pressure_card(&initial.pressure).values, None);
    assert_eq!(crate::ui::sensor_format::pressure(None).as_str(), "----hPa");

    bus.aht_present = false;
    acquisition.sample(&mut bus, &mut |_| {}, &mut || 10);
    let failed = acquisition.snapshot();
    assert_eq!(failed.sample_count, 1);
    assert_eq!(
        presentation::climate_card(&failed.temperature).status,
        ValueStatus::Error
    );
    assert_eq!(failed.temperature.values(), None);
    assert_eq!(failed.pressure.state(), SampleState::Fresh);

    bus.aht_present = true;
    let mut timestamp = 20;
    acquisition.sample(&mut bus, &mut |_| {}, &mut || {
        timestamp += 1;
        timestamp
    });
    let valid = acquisition.snapshot();
    assert_eq!(valid.sample_count, 2);
    assert_eq!(valid.temperature.last_success_us(), Some(21));
    assert_eq!(valid.humidity.last_success_us(), Some(21));
    assert_eq!(valid.pressure.last_success_us(), Some(22));
    let card = presentation::climate_card(&valid.temperature);
    assert_eq!(card.status, ValueStatus::Normal);
    assert_eq!(card.values.unwrap().current, 25.0);

    bus.aht_present = false;
    acquisition.sample(&mut bus, &mut |_| {}, &mut || 30);
    let stale = acquisition.snapshot();
    assert_eq!(stale.sample_count, 3);
    assert_eq!(
        presentation::climate_card(&stale.temperature).status,
        ValueStatus::Stale
    );
    assert_eq!(
        presentation::climate_card(&stale.temperature).values,
        card.values
    );
    assert_eq!(stale.temperature.history(), &[25.0]);
    assert_eq!(stale.temperature.last_success_us(), Some(21));
    assert_eq!(stale.pressure.last_success_us(), Some(30));

    bus.aht_present = true;
    bus.bmp_present = false;
    acquisition.sample(&mut bus, &mut |_| {}, &mut || 40);
    let recovered = acquisition.snapshot();
    assert_eq!(recovered.sample_count, 4);
    assert_eq!(recovered.temperature.state(), SampleState::Fresh);
    assert_eq!(recovered.temperature.history(), &[25.0, 25.0]);
    assert_eq!(recovered.temperature.last_success_us(), Some(40));
    assert_eq!(
        presentation::pressure_card(&recovered.pressure).status,
        ValueStatus::Stale
    );
    assert_eq!(recovered.pressure.last_success_us(), Some(30));
    // 旧快照独立于组件，后续采集不会改变它。
    assert_eq!(initial.temperature.state(), SampleState::Pending);
    assert_eq!(valid.temperature.history(), &[25.0]);
    assert_eq!(failed.sample_count, 1);
}

#[test]
fn footer_mapping_preserves_labels_and_hides_disabled_connection() {
    use crate::{
        application::connection::{EspLinkState, UploadState},
        ui::status::StatusTone,
    };
    for (link, text, tone) in [
        (
            EspLinkState::SerialReady,
            "ESP: AT ready",
            StatusTone::Normal,
        ),
        (
            EspLinkState::WifiConnected,
            "WiFi: connected",
            StatusTone::Success,
        ),
        (EspLinkState::Failed, "ESP: failed", StatusTone::Failure),
    ] {
        for (upload, upload_text, upload_tone) in [
            (UploadState::Disabled, "Upload: disabled", StatusTone::Muted),
            (UploadState::Idle, "Upload: idle", StatusTone::Normal),
            (UploadState::Ok, "Upload: ok", StatusTone::Success),
            (UploadState::Failed, "Upload: failed", StatusTone::Failure),
        ] {
            let hidden = presentation::status_bar(false, link, upload, 42, Some(5920));
            assert_eq!(hidden.connection, None);
            assert_eq!(hidden.sample_count, 42);
            assert_eq!(hidden.refresh_centihz, Some(5920));
            let shown = presentation::status_bar(true, link, upload, 42, None);
            let labels = shown.connection.unwrap();
            assert_eq!((labels.link.text, labels.link.tone), (text, tone));
            assert_eq!(
                (labels.upload.text, labels.upload.tone),
                (upload_text, upload_tone)
            );
        }
    }
}

#[test]
fn deadlines_skip_missed_ticks_without_drifting() {
    let mut timer = Periodic::new(100, 5000);
    assert!(!timer.due(5099));
    assert!(timer.due(5150));
    assert!(!timer.due(10099));
    assert!(timer.due(16100));
    assert!(!timer.due(16100));
    assert!(timer.due(20100));
}

#[test]
fn deadlines_survive_counter_wrap() {
    let mut timer = Periodic::new(u32::MAX - 99, 200);
    assert!(!timer.due(99));
    assert!(timer.due(100));
    assert!(!timer.due(299));
    assert!(timer.due(300));
}

#[test]
fn refresh_rate_measures_completions_and_returns_zero_after_idle_window() {
    let mut rate = RefreshRate::new(0);
    rate.record_success();
    rate.poll(5_000_000);
    rate.record_success();
    assert_eq!(rate.centihz(), None);
    rate.poll(10_000_500);
    assert_eq!(rate.centihz(), Some(20));
    rate.poll(20_000_500);
    assert_eq!(rate.centihz(), Some(0));
}

#[test]
fn refresh_rate_uses_actual_elapsed_time_across_counter_wrap() {
    let start = u32::MAX - 5_000_000;
    let mut rate = RefreshRate::new(start);
    rate.record_success();
    rate.record_success();
    rate.record_success();
    rate.poll(start.wrapping_add(15_000_000));
    assert_eq!(rate.centihz(), Some(20));
}

struct SensorBus {
    aht_present: bool,
    bmp_present: bool,
    calibrated: bool,
    busy: bool,
    calibration: [u8; 24],
    pressure_reads: usize,
    measurement: [u8; 6],
}

impl SensorBus {
    fn new() -> Self {
        // BMP280 compensation example: T=25.08 C, P~=1006.53 hPa.
        let coefficients: [i32; 12] = [
            27504, 26435, -1000, 36477, -10685, 3024, 2855, 140, -7, 15500, -14600, 6000,
        ];
        let mut calibration = [0; 24];
        for (i, value) in coefficients.iter().enumerate() {
            calibration[i * 2..i * 2 + 2].copy_from_slice(&(*value as u16).to_le_bytes());
        }
        let pressure: u32 = 415148;
        let temp: u32 = 519888;
        Self {
            aht_present: true,
            bmp_present: true,
            calibrated: true,
            busy: false,
            calibration,
            pressure_reads: 0,
            measurement: [
                (pressure >> 12) as u8,
                (pressure >> 4) as u8,
                (pressure << 4) as u8,
                (temp >> 12) as u8,
                (temp >> 4) as u8,
                (temp << 4) as u8,
            ],
        }
    }
}

impl Write for SensorBus {
    type Error = ();
    fn write(&mut self, addr: u8, _: &[u8]) -> Result<(), ()> {
        if (addr == 0x38 && self.aht_present) || (addr == 0x76 && self.bmp_present) {
            Ok(())
        } else {
            Err(())
        }
    }
}
impl Read for SensorBus {
    type Error = ();
    fn read(&mut self, addr: u8, buffer: &mut [u8]) -> Result<(), ()> {
        if addr != 0x38 || !self.aht_present {
            return Err(());
        }
        // 50% humidity and 25 C; final CRC byte is unused by the existing protocol parser.
        buffer.copy_from_slice(&[0x08, 0x80, 0x00, 0x06, 0x00, 0x00, 0]);
        Ok(())
    }
}
impl WriteRead for SensorBus {
    type Error = ();
    fn write_read(&mut self, addr: u8, bytes: &[u8], buffer: &mut [u8]) -> Result<(), ()> {
        match (addr, bytes[0]) {
            (0x38, 0x71) if self.aht_present => {
                buffer[0] = if self.calibrated { 8 } else { 0 } | if self.busy { 0x80 } else { 0 };
            },
            (0x76, 0xd0) if self.bmp_present => buffer[0] = 0x58,
            (0x76, 0x88) if self.bmp_present => buffer.copy_from_slice(&self.calibration),
            (0x76, 0xf7) if self.bmp_present => {
                self.pressure_reads += 1;
                buffer.copy_from_slice(&self.measurement);
            },
            _ => return Err(()),
        }
        Ok(())
    }
}

#[test]
fn bmp_missing_device_is_an_initialization_error_and_can_recover() {
    let mut bus = SensorBus::new();
    let mut bmp = Bmp280::new();
    bus.bmp_present = false;
    assert!(matches!(
        bmp.init(&mut bus, &mut |_| {}),
        Err(Bmp280Error::DeviceNotFound)
    ));
    bus.bmp_present = true;
    bmp.init(&mut bus, &mut |_| {}).unwrap();
    let reading = bmp.read(&mut bus).unwrap();
    assert!((reading.temperature - 25.08).abs() < 0.01);
    assert!((reading.pressure - 1006.53).abs() < 0.1);
    bus.bmp_present = false;
    assert!(bmp.init(&mut bus, &mut |_| {}).is_err());
    assert!(matches!(
        bmp.read(&mut bus),
        Err(Bmp280Error::DeviceNotFound)
    ));
}

#[test]
fn bmp_rejects_invalid_calibration_and_reset_measurements() {
    let mut bus = SensorBus::new();
    bus.calibration[6..8].fill(0);
    let mut bmp = Bmp280::new();
    assert!(matches!(
        bmp.init(&mut bus, &mut |_| {}),
        Err(Bmp280Error::InvalidCalibration)
    ));
    let mut bus = SensorBus::new();
    bmp.init(&mut bus, &mut |_| {}).unwrap();
    bus.measurement = [0x80, 0, 0, 0x80, 0, 0];
    assert!(matches!(
        bmp.read(&mut bus),
        Err(Bmp280Error::MeasurementNotReady)
    ));
}

#[test]
fn aht_failure_does_not_stop_pressure_and_is_retried() {
    let mut bus = SensorBus::new();
    bus.aht_present = false;
    let mut sensors = Acquisition::default();
    let readings = sensors.sample(&mut bus, &mut |_| {}, &mut || 42);
    assert!(readings.climate.is_err());
    assert!(readings.pressure.is_ok());
    assert_eq!(bus.pressure_reads, 1);
    bus.aht_present = true;
    let readings = sensors.sample(&mut bus, &mut |_| {}, &mut || 42);
    let climate = readings.climate.unwrap();
    assert_eq!(climate.temperature, 25.0);
    assert_eq!(climate.humidity, 50.0);
    assert!(readings.pressure.is_ok());
}

#[test]
fn aht_checks_calibration_and_bounds_busy_polling() {
    let mut bus = SensorBus::new();
    bus.calibrated = false;
    let mut sensor = Aht20::new();
    assert!(matches!(
        sensor.init(&mut bus, &mut |_| {}),
        Err(Aht20Error::NotCalibrated)
    ));
    bus.calibrated = true;
    bus.busy = true;
    let mut waited_ms = 0;
    assert!(matches!(
        sensor.read(&mut bus, &mut |ms| waited_ms += ms),
        Err(Aht20Error::Busy)
    ));
    assert_eq!(waited_ms, 180);
}

#[derive(Debug)]
enum FlushError {
    Failed,
}
impl From<core::convert::Infallible> for FlushError {
    fn from(e: core::convert::Infallible) -> Self {
        match e {}
    }
}
struct Panel {
    framebuffer: Vec<Rgb565>,
    visible: Vec<Rgb565>,
    windows: Vec<(u16, u16, u16, u16)>,
    fail: bool,
}
impl Panel {
    fn new() -> Self {
        Self {
            framebuffer: vec![Rgb565::BLACK; 320 * 240],
            visible: vec![Rgb565::BLACK; 320 * 240],
            windows: Vec::new(),
            fail: false,
        }
    }
}
impl OriginDimensions for Panel {
    fn size(&self) -> Size {
        Size::new(320, 240)
    }
}
impl DrawTarget for Panel {
    type Color = Rgb565;
    type Error = core::convert::Infallible;
    fn draw_iter<I: IntoIterator<Item = Pixel<Rgb565>>>(
        &mut self,
        pixels: I,
    ) -> Result<(), Self::Error> {
        for Pixel(p, c) in pixels {
            if p.x >= 0 && p.x < 320 && p.y >= 0 && p.y < 240 {
                self.framebuffer[p.y as usize * 320 + p.x as usize] = c;
            }
        }
        Ok(())
    }
}
impl FlushTarget for Panel {
    type FlushError = FlushError;
    fn flush(&mut self) -> Result<(), FlushError> {
        self.flush_rect(0, 0, 320, 240)
    }
    fn flush_rect(&mut self, x: u16, y: u16, w: u16, h: u16) -> Result<(), FlushError> {
        if self.fail {
            self.fail = false;
            return Err(FlushError::Failed);
        }
        self.windows.push((x, y, w, h));
        for row in y..y + h {
            let start = row as usize * 320 + x as usize;
            self.visible[start..start + w as usize]
                .copy_from_slice(&self.framebuffer[start..start + w as usize]);
        }
        Ok(())
    }
}
fn screen() -> Screen {
    let mut screen = Screen::new(320, 240);
    screen
        .add_temp_humd_card(TempHumidCard::new(5, 20, true))
        .unwrap();
    screen
        .add_temp_humd_card(TempHumidCard::new(110, 20, false))
        .unwrap();
    screen
        .add_pressure_card(PressureCard::new(215, 20))
        .unwrap();
    screen
}

#[test]
fn widget_owned_inputs_preserve_original_card_and_footer_pixels() {
    use crate::ui::{
        status::{draw_status, ConnectionLabels, StatusData, StatusLabel, StatusTone},
        PressureCardData, TempHumidCardData, ValueRange, ValueStatus,
    };
    // 改动前实现生成的 RGB565 小端像素 FNV-1a 快照，覆盖等待、失败、正常、旧值和恢复。
    // 控件测试只构造 UI 输入，不创建任何领域模型。
    let expected = [
        [0xf87b538557828dcb, 0x8878aeb1e1a8000f],
        [0xced7583177dcd7fb, 0x6000d3cb60bcf9ff],
        [0x23b4ba5ef6ac94fb, 0xb4de35f8df8cb6ff],
        [0x2594c85cdbc365eb, 0x97735c712a85daef],
        [0xbc7ef32c19d6b795, 0x58b1da95008da0a9],
    ];
    let range = |current, high, low| Some(ValueRange { current, high, low });
    for (phase, expected) in expected.into_iter().enumerate() {
        let status = match phase {
            0 => ValueStatus::Waiting,
            1 => ValueStatus::Error,
            3 => ValueStatus::Stale,
            _ => ValueStatus::Normal,
        };
        let (temp, humid, pressure) = if phase < 2 {
            (None, None, None)
        } else {
            (
                if phase == 4 {
                    range(0.0, 0.0, -12.3)
                } else {
                    range(-12.3, -12.3, -12.3)
                },
                range(55.0, 55.0, 55.0),
                range(1000.0, 1000.0, 1000.0),
            )
        };
        let mut view = screen();
        view.update_sensors(
            TempHumidCardData {
                values: temp,
                status,
            },
            TempHumidCardData {
                values: humid,
                status,
            },
            PressureCardData {
                values: pressure,
                status,
            },
        );
        let mut panel = Panel::new();
        view.draw_with_dma(&mut panel).unwrap();
        for (enabled, expected) in [false, true].into_iter().zip(expected) {
            draw_status(
                &mut panel,
                StatusData {
                    connection: enabled.then_some(ConnectionLabels {
                        link: StatusLabel {
                            text: "ESP: failed",
                            tone: StatusTone::Failure,
                        },
                        upload: StatusLabel {
                            text: "Upload: disabled",
                            tone: StatusTone::Muted,
                        },
                    }),
                    sample_count: 42,
                    refresh_centihz: Some(5920),
                },
            );
            let hash = panel
                .framebuffer
                .iter()
                .fold(0xcbf29ce484222325u64, |mut hash, pixel| {
                    for byte in pixel.into_storage().to_le_bytes() {
                        hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
                    }
                    hash
                });
            assert_eq!(hash, expected, "phase={phase}, connection={enabled}");
        }
    }
}

#[test]
fn partial_render_matches_full_frame_after_shorter_numbers_and_errors() {
    let mut screen = screen();
    let mut partial = Panel::new();
    screen.draw_with_dma(&mut partial).unwrap();
    use crate::ui::{PressureCardData, TempHumidCardData, ValueRange, ValueStatus};
    for value in [-12.3, 0.0, 5.0, 100.0] {
        let status = if value == 0.0 {
            ValueStatus::Stale
        } else {
            ValueStatus::Normal
        };
        screen.update_sensors(
            TempHumidCardData {
                values: Some(ValueRange {
                    current: value,
                    high: value.max(-12.3),
                    low: -12.3,
                }),
                status,
            },
            TempHumidCardData {
                values: Some(ValueRange {
                    current: 50.0,
                    high: 50.0,
                    low: 50.0,
                }),
                status: ValueStatus::Normal,
            },
            PressureCardData {
                values: Some(ValueRange {
                    current: 1000.0,
                    high: 1000.0,
                    low: 1000.0,
                }),
                status,
            },
        );
        screen.draw_sensor_updates(&mut partial).unwrap();
        let mut full = Panel::new();
        screen.draw_with_dma(&mut full).unwrap();
        assert_eq!(partial.visible, full.visible);
    }
}

#[test]
fn animation_only_changes_its_strip_and_skips_late_frames_across_wrap() {
    let mut panel = Panel::new();
    let mut animation = FlowAnimation::new(u32::MAX - 49_999);
    animation.refresh(&mut panel, 0).unwrap();
    let first = panel.visible.clone();
    animation.refresh(&mut panel, 750_000).unwrap();
    assert_ne!(first, panel.visible);
    assert!(panel
        .windows
        .iter()
        .all(|&window| window == (5, 3, 310, 14)));
    for y in 0..240 {
        for x in 0..320 {
            if !(5..315).contains(&x) || !(3..17).contains(&y) {
                assert_eq!(panel.framebuffer[y * 320 + x], Rgb565::BLACK);
            }
        }
    }
    let mut reference = Panel::new();
    let mut from_zero = FlowAnimation::new(0);
    from_zero.refresh(&mut reference, 800_000).unwrap();
    assert_eq!(panel.visible, reference.visible);
    animation.refresh(&mut panel, 6_750_000).unwrap();
    assert_eq!(panel.visible, reference.visible);
}

#[test]
fn unchanged_samples_skip_transfer_and_failed_refresh_keeps_dirty_regions() {
    let mut screen = screen();
    let mut panel = Panel::new();
    screen.draw_with_dma(&mut panel).unwrap();
    panel.windows.clear();
    let mut acquisition = Acquisition::default();
    let mut bus = SensorBus::new();
    acquisition.sample(&mut bus, &mut |_| {}, &mut || 0);
    let before = acquisition.snapshot();
    screen.update_sensors(
        presentation::climate_card(&before.temperature),
        Default::default(),
        Default::default(),
    );
    panel.fail = true;
    assert!(screen.draw_sensor_updates(&mut panel).is_err());
    assert_eq!(screen.dirty_rects.len(), 1);
    screen.draw_sensor_updates(&mut panel).unwrap();
    assert_eq!(panel.windows, [(6, 95, 98, 115)]);
    assert!(screen.dirty_rects.is_empty());
    acquisition.sample(&mut bus, &mut |_| {}, &mut || 100);
    let after = acquisition.snapshot();
    assert_ne!(
        before.temperature.last_success_us(),
        after.temperature.last_success_us()
    );
    assert_ne!(before.temperature.history(), after.temperature.history());
    screen.update_sensors(
        presentation::climate_card(&after.temperature),
        Default::default(),
        Default::default(),
    );
    screen.draw_sensor_updates(&mut panel).unwrap();
    assert_eq!(panel.windows.len(), 1);
}

#[test]
fn pressure_failure_does_not_stop_climate_sampling() {
    let mut bus = SensorBus::new();
    bus.bmp_present = false;
    let mut sensors = Acquisition::default();
    let readings = sensors.sample(&mut bus, &mut |_| {}, &mut || 42);
    assert!(readings.climate.is_ok());
    assert!(matches!(
        readings.pressure,
        Err(Bmp280Error::DeviceNotFound)
    ));
    bus.bmp_present = true;
    let readings = sensors.sample(&mut bus, &mut |_| {}, &mut || 42);
    assert!(readings.climate.is_ok());
    assert!(readings.pressure.is_ok());
}
