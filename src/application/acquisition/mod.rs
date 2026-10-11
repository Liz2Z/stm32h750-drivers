//! 一次采集的所有者：管理器件重试，并一致地更新记录与采样次数。
mod record;

use crate::drivers::{
    aht20::{Aht20, Aht20Error, Aht20Reading},
    bmp280::{Bmp280, Bmp280Error, Bmp280Reading},
};
use embedded_hal::blocking::i2c::{Read, Write, WriteRead};
use record::Record;

/// 尚未采样、成功、失败后保留旧值、失败且没有有效值。
/// Stale 由采样失败触发，不代表按时间自动过期。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SampleState {
    #[default]
    Pending,
    Fresh,
    Stale,
    Error,
}

/// 当前值和自首次成功采样以来的累计极值，单位与对应测量量一致。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SampleValues {
    pub current: f32,
    pub high: f32,
    pub low: f32,
}

/// 单个测量量的只读副本，不借用内部存储，也不提供写入接口。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ReadingSnapshot {
    values: Option<SampleValues>,
    history: [f32; 6],
    history_len: usize,
    state: SampleState,
    last_success_us: Option<u32>,
}

impl ReadingSnapshot {
    pub fn values(&self) -> Option<SampleValues> {
        self.values
    }

    /// 最近六次成功值，按从旧到新排列；失败不会写入历史。
    pub fn history(&self) -> &[f32] {
        &self.history[..self.history_len]
    }

    pub fn state(&self) -> SampleState {
        self.state
    }

    /// 成功完成读取后的微秒时间戳，允许 u32 回绕；失败保持原值。
    pub fn last_success_us(&self) -> Option<u32> {
        self.last_success_us
    }
}

/// 同一采集周期结束后的结果副本；温度 °C、湿度 %RH、气压 hPa。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AcquisitionSnapshot {
    pub temperature: ReadingSnapshot,
    pub humidity: ReadingSnapshot,
    pub pressure: ReadingSnapshot,
    /// 已执行的采样周期数，含失败周期；按 u32 回绕。
    pub sample_count: u32,
}

/// 本次器件读取结果，保留故障类型；失败不会抹掉组件内的上次有效值。
pub struct Readings {
    pub climate: Result<Aht20Reading, Aht20Error>,
    pub pressure: Result<Bmp280Reading, Bmp280Error>,
}

/// 独占两个驱动、初始化状态和三个测量记录；总线仅在采样期间顺序借用。
#[derive(Default)]
pub struct Acquisition {
    aht20: Aht20,
    bmp280: Bmp280,
    aht20_ready: bool,
    bmp280_ready: bool,
    temperature: Record,
    humidity: Record,
    pressure: Record,
    sample_count: u32,
}

impl Acquisition {
    /// 完成一轮读取、记录更新和计数；每个器件失败后在下轮重新初始化。
    /// delay_ms 执行驱动要求的有界等待，now_us 在各器件读取成功后取时。
    /// 阻塞上界由驱动轮询次数及总线事务期限共同决定；总线实现必须有期限。
    /// 任一器件失败均继续读取另一器件。NaN/无穷拒绝入库并标记失败。
    pub fn sample<I2C: Read + Write + WriteRead>(
        &mut self,
        bus: &mut I2C,
        delay_ms: &mut impl FnMut(u32),
        now_us: &mut impl FnMut() -> u32,
    ) -> Readings {
        self.sample_count = self.sample_count.wrapping_add(1);
        let climate = (|| {
            if !self.aht20_ready {
                self.aht20.init(bus, delay_ms)?;
            }
            self.aht20.read(bus, delay_ms)
        })();
        self.aht20_ready = climate.is_ok();
        match climate {
            Ok(reading) => {
                let now = now_us();
                Self::record(&mut self.temperature, reading.temperature, now);
                Self::record(&mut self.humidity, reading.humidity, now);
            },
            Err(_) => {
                self.temperature.mark_failed();
                self.humidity.mark_failed();
            },
        }
        let pressure = (|| {
            if !self.bmp280_ready {
                self.bmp280.init(bus, delay_ms)?;
            }
            self.bmp280.read(bus)
        })();
        self.bmp280_ready = pressure.is_ok();
        match pressure {
            Ok(reading) => Self::record(&mut self.pressure, reading.pressure, now_us()),
            Err(_) => self.pressure.mark_failed(),
        }
        Readings { climate, pressure }
    }

    fn record(record: &mut Record, value: f32, now_us: u32) {
        if record.record(value, now_us).is_err() {
            record.mark_failed();
        }
    }

    pub fn snapshot(&self) -> AcquisitionSnapshot {
        AcquisitionSnapshot {
            temperature: self.temperature.snapshot(),
            humidity: self.humidity.snapshot(),
            pressure: self.pressure.snapshot(),
            sample_count: self.sample_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejected_values_mark_failure_without_losing_last_success() {
        let mut record = Record::default();
        Acquisition::record(&mut record, f32::NAN, 1);
        assert_eq!(record.snapshot().state(), SampleState::Error);
        assert_eq!(record.snapshot().last_success_us(), None);
        Acquisition::record(&mut record, 0.0, 2);
        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            Acquisition::record(&mut record, invalid, 3);
            let snapshot = record.snapshot();
            assert_eq!(snapshot.state(), SampleState::Stale);
            assert_eq!(snapshot.values().unwrap().current, 0.0);
            assert_eq!(snapshot.history(), &[0.0]);
            assert_eq!(snapshot.last_success_us(), Some(2));
        }
        Acquisition::record(&mut record, -5.0, 4);
        let recovered = record.snapshot();
        assert_eq!(recovered.state(), SampleState::Fresh);
        let values = recovered.values().unwrap();
        assert_eq!((values.current, values.low, values.high), (-5.0, -5.0, 0.0));
    }
}
