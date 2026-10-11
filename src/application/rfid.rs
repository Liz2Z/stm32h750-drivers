//! 读卡演示流程：200 ms 查询，初始化故障 1 s 重试，保留最后一次成功 UID。
use super::scheduling::Periodic;
use crate::drivers::rc522::{Error, Interface, Rc522, Uid};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReaderState {
    Starting,
    Waiting,
    Present,
    Collision,
    InitFailed,
    ReadFailed,
}

pub struct CardReader<I: Interface> {
    device: Rc522<I>,
    pub state: ReaderState,
    pub last_uid: Option<Uid>,
    pub version: Option<u8>,
    ready: bool,
    poll_tick: Periodic,
    last_attempt: Option<u32>,
    pub last_error: Option<Error<I::Error>>,
}

impl<I: Interface> CardReader<I> {
    pub fn new(device: Rc522<I>, now: u32) -> Self {
        Self {
            device,
            state: ReaderState::Starting,
            last_uid: None,
            version: None,
            ready: false,
            poll_tick: Periodic::new(now.wrapping_sub(200_000), 200_000),
            last_attempt: None,
            last_error: None,
        }
    }

    /// 返回可见状态是否变化；空场不作为故障，碰撞/校验失败不丢失上次成功卡号。
    pub fn poll(&mut self, mut now_us: impl FnMut() -> u32) -> bool {
        let now = now_us();
        if !self.poll_tick.due(now) {
            return false;
        }
        let previous = (self.state, self.last_uid);
        if !self.ready {
            if self
                .last_attempt
                .is_some_and(|last| now.wrapping_sub(last) < 1_000_000)
            {
                return false;
            }
            self.last_attempt = Some(now);
            match self.device.initialize() {
                Ok(version) => {
                    self.version = Some(version);
                    self.ready = true;
                },
                Err(error) => {
                    self.state = ReaderState::InitFailed;
                    self.last_error = Some(error);
                    return previous != (self.state, self.last_uid);
                },
            }
        }
        self.last_error = None;
        match self.device.read_uid(now_us) {
            Ok(Some(uid)) => {
                self.last_uid = Some(uid);
                self.state = ReaderState::Present;
            },
            Ok(None) => self.state = ReaderState::Waiting,
            Err(error) => {
                if matches!(error, Error::Interface(_) | Error::Timeout) {
                    self.ready = false;
                    self.last_attempt = Some(now);
                }
                self.state = if matches!(error, Error::Collision) {
                    ReaderState::Collision
                } else {
                    ReaderState::ReadFailed
                };
                self.last_error = Some(error);
            },
        }
        previous != (self.state, self.last_uid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, rc::Rc};

    struct EmptyField {
        failing: Rc<Cell<bool>>,
        resets: Rc<Cell<u32>>,
    }
    impl Interface for EmptyField {
        type Error = ();
        fn reset(&mut self) -> Result<(), ()> {
            self.resets.set(self.resets.get() + 1);
            if self.failing.get() {
                Err(())
            } else {
                Ok(())
            }
        }
        fn exchange(&mut self, bytes: &mut [u8; 2]) -> Result<(), ()> {
            if self.failing.get() {
                return Err(());
            }
            if bytes[0] & 0x80 != 0 {
                bytes[1] = match (bytes[0] & 0x7e) >> 1 {
                    0x37 => 0x92,
                    0x04 => 1,
                    _ => 0,
                };
            }
            Ok(())
        }
    }

    #[test]
    fn failed_initialization_backs_off_across_wrap_and_recovers_to_waiting() {
        let failing = Rc::new(Cell::new(true));
        let resets = Rc::new(Cell::new(0));
        let start = u32::MAX - 500_000;
        let mut reader = CardReader::new(
            Rc522::new(EmptyField {
                failing: failing.clone(),
                resets: resets.clone(),
            }),
            start,
        );
        assert!(reader.poll(|| start));
        assert_eq!(reader.state, ReaderState::InitFailed);
        assert!(!reader.poll(|| start.wrapping_add(800_000)));
        assert_eq!(resets.get(), 1);
        assert!(!reader.poll(|| start.wrapping_add(1_000_000)));
        assert_eq!(resets.get(), 2);
        failing.set(false);
        assert!(reader.poll(|| start.wrapping_add(2_000_000)));
        assert_eq!(reader.state, ReaderState::Waiting);
        assert_eq!(reader.version, Some(0x92));
        assert!(reader.last_error.is_none());
        assert!(!reader.poll(|| start.wrapping_add(2_200_000)));
        assert_eq!(resets.get(), 3, "空场不能触发重新初始化");
        failing.set(true);
        assert!(reader.poll(|| start.wrapping_add(2_400_000)));
        assert_eq!(reader.state, ReaderState::ReadFailed);
        failing.set(false);
        assert!(!reader.poll(|| start.wrapping_add(3_200_000)));
        assert!(reader.poll(|| start.wrapping_add(3_400_000)));
        assert_eq!(reader.state, ReaderState::Waiting);
    }
}
