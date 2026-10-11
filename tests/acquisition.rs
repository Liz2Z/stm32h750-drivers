//! 通过公开接口验证驱动采集、状态快照、显示转换及刷新协作。
mod common;

use common::{screen, Panel};
use embedded_hal::blocking::i2c::{Read, Write, WriteRead};
use stm32h750_drivers::{
    application::{
        acquisition::{Acquisition, SampleState},
        presentation,
    },
    drivers::bmp280::Bmp280Error,
};

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
        // BMP280 补偿示例：温度 25.08°C，气压约 1006.53hPa。
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
        // 湿度 50%、温度 25°C；当前协议解析器不使用末尾 CRC 字节。
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
fn acquisition_snapshots_map_failure_and_recovery_without_exposing_records() {
    use stm32h750_drivers::ui::ValueStatus;
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
