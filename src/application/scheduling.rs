/// Periodic deadlines on a wrapping, microsecond hardware counter.
/// Poll at least once per counter wrap. Long stalls skip missed samples.
pub struct Periodic {
    last: u32,
    interval: u32,
}

impl Periodic {
    pub fn new(now: u32, interval: u32) -> Self {
        assert!(interval > 0);
        Self {
            last: now,
            interval,
        }
    }

    pub fn due(&mut self, now: u32) -> bool {
        let elapsed = now.wrapping_sub(self.last);
        if elapsed < self.interval {
            return false;
        }
        self.last = now.wrapping_sub(elapsed % self.interval);
        true
    }
}

/// Successful content refreshes per second over the last completed 10 s window.
/// Footer-only transfers are excluded so the meter cannot inflate its own rate.
pub struct RefreshRate {
    started_at: u32,
    completed: u32,
    centihz: Option<u32>,
}

impl RefreshRate {
    const WINDOW_US: u32 = 10_000_000;

    pub fn new(now: u32) -> Self {
        Self {
            started_at: now,
            completed: 0,
            centihz: None,
        }
    }

    pub fn record_success(&mut self) {
        self.completed = self.completed.saturating_add(1);
    }

    pub fn poll(&mut self, now: u32) {
        let elapsed = now.wrapping_sub(self.started_at);
        if elapsed >= Self::WINDOW_US {
            self.centihz = Some(
                (((u64::from(self.completed) * 100_000_000 + u64::from(elapsed) / 2)
                    / u64::from(elapsed))
                .min(u64::from(u32::MAX))) as u32,
            );
            self.completed = 0;
            self.started_at = now;
        }
    }

    pub fn centihz(&self) -> Option<u32> {
        self.centihz
    }
}

#[cfg(test)]
mod tests {
    use super::{Periodic, RefreshRate};
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
}
