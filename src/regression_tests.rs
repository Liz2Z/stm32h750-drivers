use crate::{
    acquisition::Sensors,
    aht20::{Aht20, Aht20Error},
    bmp280::{Bmp280, Bmp280Error},
    clip_region,
    scheduling::{Periodic, RefreshRate},
    sensor::{PressureSensor, SampleState, TempHumidSensor},
    transport::{write_spi, TransferError},
    ui::{FlushTarget, PressureCard, Screen, TempHumidCard},
};
use embedded_graphics::{pixelcolor::Rgb565, prelude::*};
use embedded_hal::{
    blocking::i2c::{Read, Write, WriteRead},
    spi::FullDuplex,
};
use std::{collections::VecDeque, vec, vec::Vec};

#[test]
fn zero_and_negative_temperatures_preserve_extrema() {
    let mut data = TempHumidSensor::new();
    for temp in [0.0, 5.0, -3.0, 0.0, 2.0] {
        data.update_temp(temp);
    }
    assert_eq!((data.temp_low, data.temp_high), (-3.0, 5.0));
    let mut data = TempHumidSensor::new();
    for temp in [0.0, -5.0] {
        data.update_temp(temp);
    }
    assert_eq!(data.temp_high, 0.0);
    let mut data = TempHumidSensor::new();
    for humid in [0.0, 60.0] {
        data.update_humid(humid);
    }
    assert_eq!(data.humid_low, 0.0);
}

#[test]
fn missing_stale_and_recovered_samples_are_distinct() {
    let mut data = PressureSensor::new();
    assert_eq!(data.pressure_str().as_str(), "----hPa");
    data.mark_failed();
    assert_eq!(data.state, SampleState::Error);
    data.update(1000.0);
    data.mark_updated(42);
    data.mark_failed();
    assert_eq!(data.state, SampleState::Stale);
    assert_eq!(data.pressure_current, 1000.0);
    assert_eq!(data.last_success_us, Some(42));
    data.update(1001.0);
    data.mark_updated(50);
    assert_eq!(data.state, SampleState::Fresh);
    assert_eq!(data.last_success_us, Some(50));
}

#[test]
fn nonfinite_samples_do_not_corrupt_history() {
    let mut data = TempHumidSensor::new();
    data.update_temp(20.0);
    data.update_temp(f32::NAN);
    assert_eq!(data.history_count, 1);
    assert_eq!(data.temp_current, 20.0);
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

#[test]
fn refresh_clips_empty_and_overflowing_regions() {
    assert_eq!(clip_region(5, 5, 0, 10, 320, 240), None);
    assert_eq!(clip_region(5, 5, 10, 0, 320, 240), None);
    assert_eq!(clip_region(320, 0, 1, 1, 320, 240), None);
    assert_eq!(
        clip_region(319, 239, u16::MAX, u16::MAX, 320, 240),
        Some((319, 239, 1, 1))
    );
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
    let mut sensors = Sensors::default();
    let readings = sensors.sample(&mut bus, &mut |_| {});
    assert!(readings.climate.is_err());
    assert!(readings.pressure.is_ok());
    assert_eq!(bus.pressure_reads, 1);
    bus.aht_present = true;
    let readings = sensors.sample(&mut bus, &mut |_| {});
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

#[derive(Default)]
struct SpiMock {
    sent: Vec<u8>,
    incoming: VecDeque<u8>,
    stalled_tx: bool,
    stalled_rx: bool,
    fault: bool,
}
impl FullDuplex<u8> for SpiMock {
    type Error = ();
    fn send(&mut self, byte: u8) -> nb::Result<(), ()> {
        if self.fault {
            return Err(nb::Error::Other(()));
        }
        if self.stalled_tx {
            return Err(nb::Error::WouldBlock);
        }
        self.sent.push(byte);
        self.incoming.push_back(0);
        assert!(self.incoming.len() <= 4);
        Ok(())
    }
    fn read(&mut self) -> nb::Result<u8, ()> {
        if self.stalled_rx {
            return Err(nb::Error::WouldBlock);
        }
        self.incoming.pop_front().ok_or(nb::Error::WouldBlock)
    }
}

#[test]
fn spi_transmits_without_overwriting_source_data() {
    let mut spi = SpiMock::default();
    let data = [1, 2, 3, 4, 5, 6];
    write_spi(&mut spi, || 0, &data, 1000).unwrap();
    assert_eq!(spi.sent, data);
    assert!(spi.incoming.is_empty());
}

#[test]
fn spi_stalled_tx_and_rx_timeout_across_clock_wrap() {
    for stalled_tx in [false, true] {
        let mut spi = SpiMock {
            stalled_tx,
            stalled_rx: !stalled_tx,
            ..SpiMock::default()
        };
        let mut now = u32::MAX - 20;
        let result = write_spi(
            &mut spi,
            || {
                now = now.wrapping_add(10);
                now
            },
            &[1, 2, 3, 4, 5],
            100,
        );
        assert_eq!(result, Err(TransferError::Timeout));
    }
}

#[test]
fn spi_errors_are_propagated() {
    let mut spi = SpiMock {
        fault: true,
        ..SpiMock::default()
    };
    assert_eq!(
        write_spi(&mut spi, || 0, &[1], 100),
        Err(TransferError::Spi(()))
    );
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
fn partial_render_matches_full_frame_after_shorter_numbers_and_errors() {
    let mut screen = screen();
    let mut partial = Panel::new();
    screen.draw_with_dma(&mut partial).unwrap();
    let mut temp = TempHumidSensor::new();
    let mut humid = TempHumidSensor::new();
    let mut pressure = PressureSensor::new();
    for value in [-12.3, 0.0, 5.0, 100.0] {
        temp.update_temp(value);
        humid.update_humid(50.0);
        pressure.update(1000.0);
        if value == 0.0 {
            temp.mark_failed();
            pressure.mark_failed();
        }
        screen.update_sensors(temp, humid, pressure);
        screen.draw_sensor_updates(&mut partial).unwrap();
        let mut full = Panel::new();
        screen.draw_with_dma(&mut full).unwrap();
        assert_eq!(partial.visible, full.visible);
    }
}

#[test]
fn unchanged_samples_skip_transfer_and_failed_refresh_keeps_dirty_regions() {
    let mut screen = screen();
    let mut panel = Panel::new();
    screen.draw_with_dma(&mut panel).unwrap();
    panel.windows.clear();
    let mut temp = TempHumidSensor::new();
    temp.update_temp(20.0);
    screen.update_sensors(temp, TempHumidSensor::new(), PressureSensor::new());
    panel.fail = true;
    assert!(screen.draw_sensor_updates(&mut panel).is_err());
    assert_eq!(screen.dirty_rects.len(), 1);
    screen.draw_sensor_updates(&mut panel).unwrap();
    assert_eq!(panel.windows, [(6, 95, 98, 115)]);
    assert!(screen.dirty_rects.is_empty());
    temp.update_temp(20.0);
    temp.mark_updated(100);
    screen.update_sensors(temp, TempHumidSensor::new(), PressureSensor::new());
    screen.draw_sensor_updates(&mut panel).unwrap();
    assert_eq!(panel.windows.len(), 1);
}

#[test]
fn packed_rectangle_skips_row_padding_and_preserves_big_endian_order() {
    let frame: Vec<u16> = (0x1200..0x1214).collect(); // 5 x 4 framebuffer
    let region = (1, 1, 3, 2);
    let mut first = [0; 8];
    let count = crate::framebuffer::pack_rect_chunk(&frame, 5, region, 0, &mut first);
    assert_eq!(count, 4);
    assert_eq!(first, [0x12, 6, 0x12, 7, 0x12, 8, 0x12, 11]);
    let mut second = [0xaa; 8];
    let count = crate::framebuffer::pack_rect_chunk(&frame, 5, region, 4, &mut second);
    assert_eq!(count, 2);
    assert_eq!(second, [0x12, 12, 0x12, 13, 0xaa, 0xaa, 0xaa, 0xaa]);
}

#[test]
fn pressure_failure_does_not_stop_climate_sampling() {
    let mut bus = SensorBus::new();
    bus.bmp_present = false;
    let mut sensors = Sensors::default();
    let readings = sensors.sample(&mut bus, &mut |_| {});
    assert!(readings.climate.is_ok());
    assert!(matches!(
        readings.pressure,
        Err(Bmp280Error::DeviceNotFound)
    ));
    bus.bmp_present = true;
    let readings = sensors.sample(&mut bus, &mut |_| {});
    assert!(readings.climate.is_ok());
    assert!(readings.pressure.is_ok());
}
