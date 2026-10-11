//! 读卡流程：200 ms 查询和逐块只读扫描，故障退避，保留最近卡片快照。
use super::scheduling::Periodic;
use crate::drivers::rc522::{CardInfo, Error, Interface, KeyType, Rc522};

/// 每块保留读取结果；认证失败不能伪装成全零数据或已解码的门禁权限。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockState {
    Pending,
    ReadWithA,
    ReadWithB,
    DefaultKeyFailed,
    ReadDenied,
    ReadFailed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockRecord {
    pub state: BlockState,
    pub data: [u8; 16],
}

/// 应用持有最近一张卡的内存快照；最多覆盖 Classic 4K，不向 UI 暴露协议驱动。
pub struct MemoryScan {
    pub blocks: [BlockRecord; 256],
    pub total_blocks: u16,
    pub completed_blocks: u16,
    pub readable_blocks: u16,
    pub default_key_failed_blocks: u16,
    key_type: KeyType,
    first_failure: BlockState,
}

impl MemoryScan {
    fn new(total_blocks: u16) -> Self {
        Self {
            blocks: [BlockRecord {
                state: BlockState::Pending,
                data: [0; 16],
            }; 256],
            total_blocks,
            completed_blocks: 0,
            readable_blocks: 0,
            default_key_failed_blocks: 0,
            key_type: KeyType::A,
            first_failure: BlockState::Pending,
        }
    }

    /// 一次最多一个块、一次认证；A 不能读取时，下次轮询再试同一出厂密钥的 B 槽。
    /// 硬件故障、换卡或移卡不推进进度，防止把不同卡的内容拼在一起。
    fn step<I: Interface>(
        &mut self,
        device: &mut Rc522<I>,
        card: &CardInfo,
        now_us: &mut impl FnMut() -> u32,
    ) -> Result<bool, Error<I::Error>> {
        if self.completed_blocks == self.total_blocks {
            return Ok(false);
        }
        let result = device.read_classic_block(
            card,
            self.completed_blocks as u8,
            self.key_type,
            &[0xff; 6],
            now_us,
        );
        self.record(result)
    }

    fn record<E>(&mut self, result: Result<[u8; 16], Error<E>>) -> Result<bool, Error<E>> {
        match result {
            Ok(data) => {
                let state = if self.key_type == KeyType::A {
                    BlockState::ReadWithA
                } else {
                    BlockState::ReadWithB
                };
                self.blocks[usize::from(self.completed_blocks)] = BlockRecord { state, data };
                self.readable_blocks += 1;
            },
            Err(error) => {
                let failure = match error {
                    Error::AuthenticationFailed => BlockState::DefaultKeyFailed,
                    Error::Nak(_) => BlockState::ReadDenied,
                    Error::Checksum | Error::InvalidResponse | Error::Communication(_) => {
                        BlockState::ReadFailed
                    },
                    _ => return Err(error),
                };
                if self.key_type == KeyType::A {
                    self.first_failure = failure;
                    self.key_type = KeyType::B;
                    return Ok(false);
                }
                let state = if failure == self.first_failure {
                    failure
                } else {
                    BlockState::ReadFailed
                };
                self.blocks[usize::from(self.completed_blocks)].state = state;
                if state == BlockState::DefaultKeyFailed {
                    self.default_key_failed_blocks += 1;
                }
            },
        }
        self.completed_blocks += 1;
        self.key_type = KeyType::A;
        self.first_failure = BlockState::Pending;
        Ok(true)
    }
}

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
    pub last_card: Option<CardInfo>,
    pub memory: Option<MemoryScan>,
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
            last_card: None,
            memory: None,
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
        let previous = (self.state, self.last_card);
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
                    return previous != (self.state, self.last_card);
                },
            }
        }
        self.last_error = None;
        let mut memory_changed = false;
        match self.device.read_card(&mut now_us) {
            Ok(Some(card)) => {
                if self.last_card != Some(card) {
                    self.memory = Some(MemoryScan::new(card.classic_blocks().unwrap_or(0)));
                }
                self.last_card = Some(card);
                self.state = ReaderState::Present;
                if let Some(memory) = &mut self.memory {
                    match memory.step(&mut self.device, &card, &mut now_us) {
                        Ok(changed) => memory_changed = changed,
                        Err(error) => {
                            if matches!(error, Error::Interface(_) | Error::Timeout) {
                                self.ready = false;
                                self.last_attempt = Some(now);
                            }
                            self.state = ReaderState::ReadFailed;
                            self.last_error = Some(error);
                        },
                    }
                }
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
        memory_changed || previous != (self.state, self.last_card)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, rc::Rc};

    #[test]
    fn scan_falls_back_to_b_without_publishing_unread_bytes_and_resets_for_next_block() {
        let mut scan = MemoryScan::new(64);
        assert!(!scan.record::<()>(Err(Error::AuthenticationFailed)).unwrap());
        assert_eq!(scan.completed_blocks, 0);
        assert_eq!(scan.blocks[0].state, BlockState::Pending);
        assert_eq!(scan.key_type, KeyType::B);
        assert!(scan.record::<()>(Ok([0x5a; 16])).unwrap());
        assert_eq!(scan.readable_blocks, 1);
        assert_eq!(scan.blocks[0].state, BlockState::ReadWithB);
        assert_eq!(scan.blocks[0].data, [0x5a; 16]);
        assert_eq!(scan.key_type, KeyType::A);
        assert!(scan.record::<()>(Ok([0; 16])).unwrap());
        assert_eq!(
            scan.blocks[1].state,
            BlockState::ReadWithA,
            "全零块也是实际读取成功"
        );
    }

    #[test]
    fn scan_distinguishes_default_key_failure_from_denial_and_does_not_advance_on_removal() {
        let mut scan = MemoryScan::new(64);
        for error in [
            Error::NoResponse,
            Error::CardChanged,
            Error::Timeout,
            Error::Interface(()),
        ] {
            assert_eq!(scan.record(Err(error)), Err(error));
            assert_eq!(scan.completed_blocks, 0);
            assert_eq!(scan.key_type, KeyType::A);
        }
        scan.record::<()>(Err(Error::AuthenticationFailed)).unwrap();
        scan.record::<()>(Err(Error::AuthenticationFailed)).unwrap();
        assert_eq!(scan.blocks[0].state, BlockState::DefaultKeyFailed);
        assert_eq!(scan.default_key_failed_blocks, 1);
        scan.record::<()>(Err(Error::Nak(4))).unwrap();
        scan.record::<()>(Err(Error::Nak(4))).unwrap();
        assert_eq!(scan.blocks[1].state, BlockState::ReadDenied);
        assert_eq!(scan.default_key_failed_blocks, 1);
        assert_eq!(scan.readable_blocks, 0);
        assert_eq!(scan.completed_blocks, 2);
        let replacement = MemoryScan::new(20);
        assert_eq!(replacement.total_blocks, 20);
        assert_eq!(replacement.completed_blocks, 0);
        assert_eq!(replacement.blocks[0].state, BlockState::Pending);
    }

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
