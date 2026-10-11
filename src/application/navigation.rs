//! K1 上一页、K2 下一页，首尾循环；30ms 消抖，只响应一次按下。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DemoPage {
    #[default]
    Sensors,
    Rfid,
}

// 新增演示页时在此声明导航顺序。
const PAGES: [DemoPage; 2] = [DemoPage::Sensors, DemoPage::Rfid];

#[derive(Default)]
struct DebouncedPress {
    candidate: bool,
    stable: bool,
    changed_at: u32,
}

impl DebouncedPress {
    fn poll(&mut self, pressed: bool, now: u32) -> bool {
        if pressed != self.candidate {
            self.candidate = pressed;
            self.changed_at = now;
        }
        if self.stable != self.candidate && now.wrapping_sub(self.changed_at) >= 30_000 {
            self.stable = self.candidate;
            return self.stable;
        }
        false
    }
}

#[derive(Default)]
pub struct Navigation {
    selected: usize,
    k1: DebouncedPress,
    k2: DebouncedPress,
}

impl Navigation {
    pub fn page(&self) -> DemoPage {
        // selected 私有，只通过模页面数的运算更新，始终在数组范围内。
        PAGES[self.selected]
    }

    /// K1 上一页，K2 下一页，首尾循环；同时按住两键时忽略动作。
    /// 电平应在主循环中持续轮询，now 单位 µs，允许 u32 回绕。
    pub fn poll(&mut self, pressed: (bool, bool), now: u32) -> bool {
        let k1 = self.k1.poll(pressed.0, now);
        let k2 = self.k2.poll(pressed.1, now);
        let previous = self.selected;
        if !(pressed.0 && pressed.1) {
            if k1 {
                self.selected = (self.selected + PAGES.len() - 1) % PAGES.len();
            }
            if k2 {
                self.selected = (self.selected + 1) % PAGES.len();
            }
        }
        self.selected != previous
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounce_and_holding_do_not_repeat_page_changes() {
        let mut nav = Navigation::default();
        assert!(!nav.poll((false, true), 0));
        assert!(!nav.poll((false, false), 5_000));
        assert!(!nav.poll((false, true), 10_000));
        assert!(!nav.poll((false, true), 39_999));
        assert!(nav.poll((false, true), 40_000));
        assert_eq!(nav.page(), DemoPage::Rfid);
        assert!(!nav.poll((false, true), 500_000));
        assert!(!nav.poll((false, false), 510_000));
        assert!(!nav.poll((false, false), 540_000));
        assert!(!nav.poll((true, false), 550_000));
        assert!(nav.poll((true, false), 580_000));
        assert_eq!(nav.page(), DemoPage::Sensors);
    }

    #[test]
    fn debounce_survives_timer_wrap_and_simultaneous_keys_are_ignored() {
        let mut nav = Navigation::default();
        let start = u32::MAX - 10_000;
        assert!(!nav.poll((false, true), start));
        assert!(!nav.poll((false, true), start.wrapping_add(29_999)));
        assert!(nav.poll((false, true), start.wrapping_add(30_000)));
        assert!(!nav.poll((true, true), 40_000));
        assert!(!nav.poll((true, true), 70_000));
        assert_eq!(nav.page(), DemoPage::Rfid);
    }

    #[test]
    fn up_and_down_wrap_at_both_ends_instead_of_selecting_fixed_pages() {
        for key in [(true, false), (false, true)] {
            let mut nav = Navigation::default();
            for (cycle, expected) in [DemoPage::Rfid, DemoPage::Sensors, DemoPage::Rfid]
                .into_iter()
                .enumerate()
            {
                let now = cycle as u32 * 200_000;
                assert!(!nav.poll(key, now));
                assert!(nav.poll(key, now + 30_000));
                assert_eq!(nav.page(), expected);
                assert!(!nav.poll((false, false), now + 80_000));
                assert!(!nav.poll((false, false), now + 110_000));
            }
        }
    }
}
