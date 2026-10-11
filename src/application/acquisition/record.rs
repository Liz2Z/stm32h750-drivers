//! 单个测量量的私有存储；只有采集组件能够写入。
use super::{ReadingSnapshot, SampleState, SampleValues};

#[derive(Debug, PartialEq, Eq)]
pub(super) enum SampleError {
    NonFinite,
}

/// 单个测量量的采样记录。温度、湿度、气压各持有独立实例。
/// 私有字段保证读数、历史、状态和成功时间戳只能一致地更新。
#[derive(Clone, Debug, Default)]
pub(super) struct Record {
    snapshot: ReadingSnapshot,
}

impl Record {
    /// 记录一次有效采样，同时更新极值、最近六次历史和成功时间戳。
    /// NaN/无穷返回错误，保持原有记录不变；now_us 为可回绕的微秒计数。
    pub(super) fn record(&mut self, value: f32, now_us: u32) -> Result<(), SampleError> {
        if !value.is_finite() {
            return Err(SampleError::NonFinite);
        }
        self.snapshot.values = Some(match self.snapshot.values {
            Some(previous) => SampleValues {
                current: value,
                high: previous.high.max(value),
                low: previous.low.min(value),
            },
            None => SampleValues {
                current: value,
                high: value,
                low: value,
            },
        });
        if self.snapshot.history_len == self.snapshot.history.len() {
            self.snapshot.history.copy_within(1.., 0);
            self.snapshot.history[self.snapshot.history_len - 1] = value;
        } else {
            self.snapshot.history[self.snapshot.history_len] = value;
            self.snapshot.history_len += 1;
        }
        self.snapshot.state = SampleState::Fresh;
        self.snapshot.last_success_us = Some(now_us);
        Ok(())
    }

    /// 采样失败：有有效记录则保留并标记 Stale，否则标记 Error。
    pub(super) fn mark_failed(&mut self) {
        self.snapshot.state = if self.snapshot.values.is_some() {
            SampleState::Stale
        } else {
            SampleState::Error
        };
    }

    pub(super) fn snapshot(&self) -> ReadingSnapshot {
        self.snapshot
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_and_negative_temperatures_preserve_extrema() {
        let mut data = Record::default();
        for temp in [0.0, 5.0, -3.0, 0.0, 2.0] {
            data.record(temp, 0).unwrap();
        }
        assert_eq!(
            (
                data.snapshot().values().unwrap().low,
                data.snapshot().values().unwrap().high
            ),
            (-3.0, 5.0)
        );
        let mut data = Record::default();
        for temp in [0.0, -5.0] {
            data.record(temp, 0).unwrap();
        }
        assert_eq!(data.snapshot().values().unwrap().high, 0.0);
        let mut data = Record::default();
        for humid in [0.0, 60.0] {
            data.record(humid, 0).unwrap();
        }
        assert_eq!(data.snapshot().values().unwrap().low, 0.0);
    }

    #[test]
    fn missing_stale_and_recovered_samples_are_distinct() {
        let mut data = Record::default();
        assert_eq!(data.snapshot().values(), None);
        data.mark_failed();
        assert_eq!(data.snapshot().state(), SampleState::Error);
        data.record(1000.0, 42).unwrap();
        data.mark_failed();
        assert_eq!(data.snapshot().state(), SampleState::Stale);
        assert_eq!(data.snapshot().values().unwrap().current, 1000.0);
        assert_eq!(data.snapshot().last_success_us(), Some(42));
        data.record(1001.0, 50).unwrap();
        assert_eq!(data.snapshot().state(), SampleState::Fresh);
        assert_eq!(data.snapshot().last_success_us(), Some(50));
    }

    #[test]
    fn nonfinite_samples_do_not_corrupt_history() {
        let mut data = Record::default();
        data.record(20.0, 10).unwrap();
        data.mark_failed();
        let values = data.snapshot().values();
        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert!(data.record(invalid, 20).is_err());
            assert_eq!(data.snapshot().history(), &[20.0]);
            assert_eq!(data.snapshot().values(), values);
            assert_eq!(data.snapshot().state(), SampleState::Stale);
            assert_eq!(data.snapshot().last_success_us(), Some(10));
        }
    }

    #[test]
    fn rolling_history_retains_all_time_extrema_and_wrapping_timestamp() {
        let mut data = Record::default();
        for (i, value) in [-10.0, 100.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0]
            .into_iter()
            .enumerate()
        {
            data.record(value, (u32::MAX - 3).wrapping_add(i as u32))
                .unwrap();
        }
        assert_eq!(data.snapshot().history(), &[2.0, 3.0, 4.0, 5.0, 6.0, 7.0]);
        let values = data.snapshot().values().unwrap();
        assert_eq!(
            (values.current, values.low, values.high),
            (7.0, -10.0, 100.0)
        );
        assert_eq!(data.snapshot().last_success_us(), Some(3));
        data.mark_failed();
        assert_eq!(data.snapshot().history(), &[2.0, 3.0, 4.0, 5.0, 6.0, 7.0]);
    }
}
