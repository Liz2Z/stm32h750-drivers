//! MFRC522：ISO14443A 选卡、公开参数与 MIFARE Classic 只读访问。
//! 寄存器依据 NXP MFRC522 数据手册 Rev.3.9；选卡流程参照 miguelbalboa/rfid。

/// 独占接口。exchange 在一次 CS 低电平内交换两个字节，含地址和数据。
/// 实现必须保证有界返回；reset 必须复位器件并等待晶振稳定。
pub trait Interface {
    type Error;
    fn reset(&mut self) -> Result<(), Self::Error>;
    fn exchange(&mut self, bytes: &mut [u8; 2]) -> Result<(), Self::Error>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error<E> {
    Interface(E),
    NotInitialized,
    DeviceNotFound(u8),
    Timeout,
    Collision,
    Communication(u8),
    InvalidResponse,
    Checksum,
    AuthenticationFailed,
    NoResponse,
    CardChanged,
    UnsupportedCard,
    InvalidBlock,
    Nak(u8),
}

/// MIFARE Classic 扇区认证使用的密钥槽；驱动不保存或猜测密钥。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyType {
    A,
    B,
}

/// ISO14443A UID，实际长度为 4、7 或 10 字节；不代表卡片认证结果。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Uid {
    bytes: [u8; 10],
    len: usize,
}

impl Uid {
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

/// 选卡得到的公开信息。ATQA 保留空口字节序；SAK 不能证明芯片真伪。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CardInfo {
    pub uid: Uid,
    pub atqa: [u8; 2],
    pub sak: u8,
}

impl CardInfo {
    /// 根据最终 SAK 推定 Classic 布局，不适用于其他协议或真伪鉴定。
    pub fn classic_blocks(&self) -> Option<u16> {
        match self.sak {
            0x09 => Some(20),
            0x08 => Some(64),
            0x18 => Some(256),
            _ => None,
        }
    }

    /// 按最终 SAK 初步识别兼容类型；不能据此确定生产商或门禁数据格式。
    pub fn kind(&self) -> &'static str {
        match self.sak {
            0x09 => "MIFARE Mini compatible",
            0x08 => "MIFARE Classic 1K compatible",
            0x18 => "MIFARE Classic 4K compatible",
            0x00 => "Ultralight / NTAG compatible",
            0x20 => "ISO14443-4 compatible",
            _ => "Unknown ISO14443A card",
        }
    }
}

pub struct Rc522<I> {
    interface: I,
    initialized: bool,
}

impl<I: Interface> Rc522<I> {
    pub fn new(interface: I) -> Self {
        Self {
            interface,
            initialized: false,
        }
    }

    fn read(&mut self, register: u8) -> Result<u8, Error<I::Error>> {
        let mut bytes = [0x80 | (register << 1), 0];
        self.interface
            .exchange(&mut bytes)
            .map_err(Error::Interface)?;
        Ok(bytes[1])
    }

    fn write(&mut self, register: u8, value: u8) -> Result<(), Error<I::Error>> {
        self.interface
            .exchange(&mut [register << 1, value])
            .map_err(Error::Interface)
    }

    /// 可重试初始化，返回 VersionReg。接口复位耗时由 BSP 约束，普通读卡无复位延时。
    pub fn initialize(&mut self) -> Result<u8, Error<I::Error>> {
        self.initialized = false;
        self.interface.reset().map_err(Error::Interface)?;
        let version = self.read(0x37)?;
        if version == 0 || version == 0xff {
            return Err(Error::DeviceNotFound(version));
        }
        // 106 kBd；40 kHz 定时器自动启动，200 个计数即 5 ms 空场等待。
        for (reg, value) in [
            (0x12, 0),
            (0x13, 0),
            (0x24, 0x26),
            (0x2a, 0x80),
            (0x2b, 0xa9),
            (0x2c, 0),
            (0x2d, 200),
            (0x15, 0x40),
            (0x11, 0x3d),
            (0x08, 0),
            (0x14, 0x83),
        ] {
            self.write(reg, value)?;
        }
        self.initialized = true;
        Ok(version)
    }

    fn transceive(
        &mut self,
        tx: &[u8],
        bits: u8,
        rx: &mut [u8],
        now_us: &mut impl FnMut() -> u32,
    ) -> Result<Option<usize>, Error<I::Error>> {
        let result = self.exchange_frame(tx, bits, rx, now_us);
        // 成功、无卡、碰撞和超时都停止命令；总线失败由上层重新初始化。
        self.write(0x01, 0)?;
        self.write(0x0d, 0)?;
        result
    }

    fn exchange_frame(
        &mut self,
        tx: &[u8],
        bits: u8,
        rx: &mut [u8],
        now_us: &mut impl FnMut() -> u32,
    ) -> Result<Option<usize>, Error<I::Error>> {
        self.write(0x01, 0)?;
        self.write(0x04, 0x7f)?;
        self.write(0x0a, 0x80)?;
        for &byte in tx {
            self.write(0x09, byte)?;
        }
        self.write(0x0d, bits)?;
        self.write(0x01, 0x0c)?;
        self.write(0x0d, 0x80 | bits)?;
        let start = now_us();
        loop {
            let irq = self.read(0x04)?;
            if irq & 0x32 != 0 {
                let error = self.read(0x06)?;
                if error & 0x08 != 0 {
                    return Err(Error::Collision);
                }
                if error & 0x17 != 0 {
                    return Err(Error::Communication(error));
                }
                let len = self.read(0x0a)? as usize;
                if len == 0 || len > rx.len() {
                    return Err(Error::InvalidResponse);
                }
                for byte in &mut rx[..len] {
                    *byte = self.read(0x09)?;
                }
                let tail_bits = self.read(0x0c)? & 7;
                if len == 1 && tail_bits == 4 {
                    return Err(Error::Nak(rx[0] & 0x0f));
                }
                if tail_bits != 0 {
                    return Err(Error::InvalidResponse);
                }
                return Ok(Some(len));
            }
            if irq & 1 != 0 {
                return Ok(None);
            }
            // 即使器件定时器失效，也不能让主循环永久卡住；减法支持 TIM2 回绕。
            if now_us().wrapping_sub(start) >= 10_000 {
                return Err(Error::Timeout);
            }
        }
    }

    /// WUPA → 防碰撞 → SELECT → HALT；支持 4/7/10 字节 UID。
    /// 仅适用于单卡演示，多卡碰撞显式返回错误，不执行任意选卡。
    /// 每帧硬件等待 5 ms、软件等待 10 ms，最多 8 帧；接口耗时另计。
    /// Ok(None) 表示空场。计时回调必须持续递增（单位 µs），允许 u32 回绕。
    pub fn read_uid(
        &mut self,
        now_us: impl FnMut() -> u32,
    ) -> Result<Option<Uid>, Error<I::Error>> {
        self.read_card(now_us).map(|card| card.map(|card| card.uid))
    }

    /// 与 read_uid 相同的单卡流程和阻塞上界，另外保留 ATQA 与最终 SAK。
    pub fn read_card(
        &mut self,
        mut now_us: impl FnMut() -> u32,
    ) -> Result<Option<CardInfo>, Error<I::Error>> {
        if !self.initialized {
            return Err(Error::NotInitialized);
        }
        let result = self
            .stop_crypto()
            .and_then(|()| self.select_card(&mut now_us))
            .and_then(|card| {
                if card.is_some() {
                    self.halt(&mut now_us)?;
                }
                Ok(card)
            });
        if matches!(result, Err(Error::Interface(_) | Error::Timeout)) {
            self.initialized = false;
        }
        result
    }

    fn select_card(
        &mut self,
        now_us: &mut impl FnMut() -> u32,
    ) -> Result<Option<CardInfo>, Error<I::Error>> {
        let mut response = [0; 5];
        match self.transceive(&[0x52], 7, &mut response, now_us)? {
            None => return Ok(None),
            Some(2) => {},
            _ => return Err(Error::InvalidResponse),
        }
        let atqa = [response[0], response[1]];
        let mut uid = Uid {
            bytes: [0; 10],
            len: 0,
        };
        for select in [0x93, 0x95, 0x97] {
            if self.transceive(&[select, 0x20], 0, &mut response, now_us)? != Some(5) {
                return Err(Error::InvalidResponse);
            }
            if response[..4].iter().fold(0, |bcc, byte| bcc ^ byte) != response[4] {
                return Err(Error::Checksum);
            }
            let cascade = response[0] == 0x88;
            if cascade && select == 0x97 {
                return Err(Error::InvalidResponse);
            }
            let part = if cascade {
                &response[1..4]
            } else {
                &response[..4]
            };
            uid.bytes[uid.len..uid.len + part.len()].copy_from_slice(part);
            uid.len += part.len();
            let mut command = [0; 9];
            command[..2].copy_from_slice(&[select, 0x70]);
            command[2..7].copy_from_slice(&response);
            let crc = crc_a(&command[..7]);
            command[7..].copy_from_slice(&crc);
            if self.transceive(&command, 0, &mut response, now_us)? != Some(3) {
                return Err(Error::InvalidResponse);
            }
            if crc_a(&response[..1]) != response[1..3] {
                return Err(Error::Checksum);
            }
            if (response[0] & 4 != 0) != cascade {
                return Err(Error::InvalidResponse);
            }
            if !cascade {
                let sak = response[0];
                return Ok(Some(CardInfo { uid, atqa, sak }));
            }
        }
        Err(Error::InvalidResponse)
    }

    fn halt(&mut self, now_us: &mut impl FnMut() -> u32) -> Result<(), Error<I::Error>> {
        // HALT 在 Crypto1 仍开启时发送，随后才能关闭加密；正常行为是不应答。
        if self
            .transceive(&[0x50, 0, 0x57, 0xcd], 0, &mut [0; 5], now_us)?
            .is_some()
        {
            return Err(Error::InvalidResponse);
        }
        Ok(())
    }

    fn stop_crypto(&mut self) -> Result<(), Error<I::Error>> {
        let status = self.read(0x08)?;
        self.write(0x08, status & !8)
    }

    fn authenticate(
        &mut self,
        uid: &Uid,
        block: u8,
        key_type: KeyType,
        key: &[u8; 6],
        now_us: &mut impl FnMut() -> u32,
    ) -> Result<(), Error<I::Error>> {
        self.write(0x01, 0)?;
        self.write(0x04, 0x7f)?;
        self.write(0x0a, 0x80)?;
        self.write(0x0d, 0)?;
        let mut command = [0; 12];
        command[0] = match key_type {
            KeyType::A => 0x60,
            KeyType::B => 0x61,
        };
        command[1] = block;
        command[2..8].copy_from_slice(key);
        // 完整选卡后的 Classic 认证使用 UID 最后四字节，包括七字节 UID 卡。
        command[8..].copy_from_slice(&uid.as_bytes()[uid.len - 4..]);
        for byte in command {
            self.write(0x09, byte)?;
        }
        self.write(0x01, 0x0e)?;
        let start = now_us();
        let result = loop {
            let irq = self.read(0x04)?;
            if irq & 0x13 != 0 {
                self.write(0x01, 0)?;
                break if irq & 1 == 0 && self.read(0x06)? & 0x1f == 0 && self.read(0x08)? & 8 != 0 {
                    Ok(())
                } else {
                    Err(Error::AuthenticationFailed)
                };
            }
            if now_us().wrapping_sub(start) >= 10_000 {
                break Err(Error::Timeout);
            }
        };
        self.write(0x01, 0)?;
        result
    }

    /// 独立只读会话：重新选卡并核对身份 → 指定密钥认证 → READ → HALT → 关闭 Crypto1。
    /// 每次只读一个 16 字节块，不包含任何写卡命令。块地址必须符合卡型推定布局。
    /// 最多 10 帧及一次认证，各等待硬件 5 ms / 软件 10 ms；接口耗时另计。
    /// 卡移开、换卡、认证失败和访问拒绝分别返回错误，错误路径也清理命令与加密状态。
    pub fn read_classic_block(
        &mut self,
        expected: &CardInfo,
        block: u8,
        key_type: KeyType,
        key: &[u8; 6],
        mut now_us: impl FnMut() -> u32,
    ) -> Result<[u8; 16], Error<I::Error>> {
        if !self.initialized {
            return Err(Error::NotInitialized);
        }
        let count = expected.classic_blocks().ok_or(Error::UnsupportedCard)?;
        if u16::from(block) >= count {
            return Err(Error::InvalidBlock);
        }
        let mut selected = false;
        let result = (|| {
            self.stop_crypto()?;
            let card = self.select_card(&mut now_us)?.ok_or(Error::NoResponse)?;
            selected = true;
            if card != *expected {
                return Err(Error::CardChanged);
            }
            self.authenticate(&card.uid, block, key_type, key, &mut now_us)?;
            let crc = crc_a(&[0x30, block]);
            let mut response = [0; 18];
            match self.transceive(
                &[0x30, block, crc[0], crc[1]],
                0,
                &mut response,
                &mut now_us,
            )? {
                None => return Err(Error::NoResponse),
                Some(18) => {},
                _ => return Err(Error::InvalidResponse),
            }
            if crc_a(&response[..16]) != response[16..] {
                return Err(Error::Checksum);
            }
            let mut data = [0; 16];
            data.copy_from_slice(&response[..16]);
            Ok(data)
        })();
        let halt_result = if selected {
            self.halt(&mut now_us)
        } else {
            Ok(())
        };
        // 即使 HALT 失败，也必须尝试关闭命令和 Crypto1，避免污染下一次 WUPA。
        let idle = self.write(0x01, 0);
        let framing = self.write(0x0d, 0);
        let crypto = self.stop_crypto();
        let cleanup = idle.and(framing).and(crypto);
        let result = match (cleanup, halt_result, result) {
            (Err(error), _, _) | (_, Err(error @ (Error::Interface(_) | Error::Timeout)), _) => {
                Err(error)
            },
            (_, _, Err(error)) => Err(error),
            (_, Err(error), _) => Err(error),
            (_, _, Ok(data)) => Ok(data),
        };
        if matches!(result, Err(Error::Interface(_) | Error::Timeout)) {
            self.initialized = false;
        }
        result
    }
}

/// ISO14443A CRC，初值 0x6363，反射多项式 0x8408，低字节先传输。
fn crc_a(bytes: &[u8]) -> [u8; 2] {
    let mut crc = 0x6363u16;
    for &byte in bytes {
        crc ^= byte as u16;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0x8408
            } else {
                crc >> 1
            };
        }
    }
    crc.to_le_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::VecDeque, vec, vec::Vec};

    struct Reply {
        tx: Vec<u8>,
        payload: Vec<u8>,
        irq: u8,
        error: u8,
    }

    // 模拟寄存器/FIFO，而非直接模拟 read_uid 返回值。
    // 根据 ISO14443A 的 WUPA、防碰撞、SELECT、HALT 顺序断言每一帧。
    struct Bus {
        registers: [u8; 64],
        tx: Vec<u8>,
        rx: VecDeque<u8>,
        replies: VecDeque<Reply>,
        fail: bool,
    }

    impl Bus {
        fn new() -> Self {
            let mut registers = [0; 64];
            registers[0x37] = 0x92;
            Self {
                registers,
                tx: Vec::new(),
                rx: VecDeque::new(),
                replies: VecDeque::new(),
                fail: false,
            }
        }
        fn frame(&mut self, tx: &[u8], payload: &[u8]) {
            self.replies.push_back(Reply {
                tx: tx.to_vec(),
                payload: payload.to_vec(),
                irq: if payload.is_empty() { 1 } else { 0x30 },
                error: 0,
            });
        }
        fn card(&mut self, uid: &[u8]) {
            self.frame(&[0x52], &[0x04, 0x00]);
            let mut offset = 0;
            for select in [0x93, 0x95, 0x97] {
                let more = uid.len() - offset > 4;
                let mut block = [0; 5];
                if more {
                    block[0] = 0x88;
                    block[1..4].copy_from_slice(&uid[offset..offset + 3]);
                    offset += 3;
                } else {
                    block[..4].copy_from_slice(&uid[offset..offset + 4]);
                    offset += 4;
                }
                block[4] = block[..4].iter().fold(0, |bcc, byte| bcc ^ byte);
                self.frame(&[select, 0x20], &block);
                let mut select_frame = vec![select, 0x70];
                select_frame.extend_from_slice(&block);
                select_frame.extend_from_slice(&crc_a(&select_frame));
                let sak = if more { 4 } else { 8 };
                let mut response = vec![sak];
                response.extend_from_slice(&crc_a(&[sak]));
                self.frame(&select_frame, &response);
                if !more {
                    break;
                }
            }
            self.frame(&[0x50, 0, 0x57, 0xcd], &[]);
        }
    }

    impl Interface for Bus {
        type Error = ();
        fn reset(&mut self) -> Result<(), ()> {
            Ok(())
        }
        fn exchange(&mut self, bytes: &mut [u8; 2]) -> Result<(), ()> {
            if self.fail {
                return Err(());
            }
            assert_eq!(bytes[0] & 1, 0, "SPI 地址最低位必须为 0");
            let reg = ((bytes[0] >> 1) & 0x3f) as usize;
            if bytes[0] & 0x80 != 0 {
                bytes[1] = match reg {
                    0x09 => self.rx.pop_front().unwrap(),
                    0x0a => self.rx.len() as u8,
                    _ => self.registers[reg],
                };
            } else {
                match reg {
                    0x09 => self.tx.push(bytes[1]),
                    0x0a if bytes[1] == 0x80 => {
                        self.tx.clear();
                        self.rx.clear();
                    },
                    0x0d if bytes[1] & 0x80 != 0 => {
                        let reply = self.replies.pop_front().expect("出现额外射频帧");
                        assert_eq!(self.tx, reply.tx);
                        assert_eq!(bytes[1] & 7, if self.tx == [0x52] { 7 } else { 0 });
                        self.rx = reply.payload.into();
                        self.registers[0x04] = reply.irq;
                        self.registers[0x06] = reply.error;
                        self.registers[0x0c] = if self.rx.len() == 1 { 4 } else { 0 };
                    },
                    0x01 if bytes[1] == 0x0e => {
                        let reply = self.replies.pop_front().expect("出现额外认证命令");
                        assert_eq!(self.tx, reply.tx);
                        assert_eq!(self.tx.len(), 12);
                        self.registers[0x04] = reply.irq;
                        self.registers[0x06] = reply.error;
                        self.registers[0x08] = if reply.irq == 0x10 && reply.error == 0 {
                            8
                        } else {
                            0
                        };
                    },
                    _ => {},
                }
                if reg != 0x04 {
                    self.registers[reg] = bytes[1];
                }
            }
            Ok(())
        }
    }

    fn reader(bus: Bus) -> Rc522<Bus> {
        let mut reader = Rc522::new(bus);
        assert_eq!(reader.initialize(), Ok(0x92));
        reader
    }

    #[test]
    fn crc_matches_known_halt_and_sak_vectors() {
        assert_eq!(crc_a(&[0x50, 0]), [0x57, 0xcd]);
        assert_eq!(crc_a(&[0x08]), [0xb6, 0xdd]);
    }

    fn selected_card(bus: &mut Bus, uid: &[u8]) -> CardInfo {
        bus.card(uid);
        bus.replies.pop_back(); // 单独会话暂不 HALT，接下来认证和读取。
        let mut bytes = [0; 10];
        bytes[..uid.len()].copy_from_slice(uid);
        CardInfo {
            uid: Uid {
                bytes,
                len: uid.len(),
            },
            atqa: [4, 0],
            sak: 8,
        }
    }

    fn auth(bus: &mut Bus, card: &CardInfo, key_type: KeyType, irq: u8) {
        let mut tx = vec![if key_type == KeyType::A { 0x60 } else { 0x61 }, 1];
        tx.extend_from_slice(&[0xff; 6]);
        tx.extend_from_slice(&card.uid.as_bytes()[card.uid.len - 4..]);
        bus.replies.push_back(Reply {
            tx,
            payload: vec![],
            irq,
            error: 0,
        });
    }

    #[test]
    fn card_parameters_and_classic_reads_preserve_crc_and_authenticate_last_uid_bytes() {
        for uid in [&[0xde, 0xad, 0xbe, 0xef][..], &[4, 1, 2, 3, 4, 5, 6]] {
            for key_type in [KeyType::A, KeyType::B] {
                let mut bus = Bus::new();
                let card = selected_card(&mut bus, uid);
                auth(&mut bus, &card, key_type, 0x10);
                let data = [0x5a; 16];
                let mut response = data.to_vec();
                response.extend_from_slice(&crc_a(&data));
                let crc = crc_a(&[0x30, 1]);
                bus.frame(&[0x30, 1, crc[0], crc[1]], &response);
                bus.frame(&[0x50, 0, 0x57, 0xcd], &[]);
                bus.card(uid);
                let mut reader = reader(bus);
                assert_eq!(
                    reader.read_classic_block(&card, 1, key_type, &[0xff; 6], || 0),
                    Ok(data)
                );
                assert_eq!(
                    reader.interface.registers[8] & 8,
                    0,
                    "读取后必须关闭 Crypto1"
                );
                assert_eq!(reader.read_card(|| 0), Ok(Some(card)));
                assert!(reader.interface.replies.is_empty());
            }
        }
    }

    #[test]
    fn authentication_failure_nak_checksum_and_stalled_auth_clean_up_and_allow_retry() {
        for failure in [
            Error::AuthenticationFailed,
            Error::Nak(4),
            Error::Checksum,
            Error::Timeout,
        ] {
            let mut bus = Bus::new();
            let card = selected_card(&mut bus, &[0xde, 0xad, 0xbe, 0xef]);
            auth(
                &mut bus,
                &card,
                KeyType::A,
                match failure {
                    Error::AuthenticationFailed => 1,
                    Error::Timeout => 0,
                    _ => 0x10,
                },
            );
            if matches!(failure, Error::Nak(_) | Error::Checksum) {
                let mut response = vec![0; 18];
                if matches!(failure, Error::Nak(_)) {
                    response = vec![4];
                }
                let crc = crc_a(&[0x30, 1]);
                bus.frame(&[0x30, 1, crc[0], crc[1]], &response);
            }
            bus.frame(&[0x50, 0, 0x57, 0xcd], &[]);
            bus.card(card.uid.as_bytes());
            let mut reader = reader(bus);
            let mut now = u32::MAX - 5_000;
            assert_eq!(
                reader.read_classic_block(&card, 1, KeyType::A, &[0xff; 6], || {
                    now = now.wrapping_add(1_000);
                    now
                }),
                Err(failure)
            );
            assert_eq!(reader.interface.registers[1], 0);
            assert_eq!(reader.interface.registers[8] & 8, 0);
            assert_eq!(reader.interface.registers[0x0d], 0);
            if failure == Error::Timeout {
                assert_eq!(reader.read_card(|| 0), Err(Error::NotInitialized));
                reader.initialize().unwrap();
            }
            assert_eq!(reader.read_card(|| 0), Ok(Some(card)));
        }
    }

    #[test]
    fn changed_card_and_invalid_block_never_start_authentication() {
        let mut bus = Bus::new();
        let mut card = selected_card(&mut bus, &[0xde, 0xad, 0xbe, 0xef]);
        bus.frame(&[0x50, 0, 0x57, 0xcd], &[]);
        let mut reader = reader(bus);
        assert_eq!(
            reader.read_classic_block(&card, 64, KeyType::A, &[0xff; 6], || 0),
            Err(Error::InvalidBlock)
        );
        card.uid.bytes[0] ^= 1;
        assert_eq!(
            reader.read_classic_block(&card, 1, KeyType::A, &[0xff; 6], || 0),
            Err(Error::CardChanged)
        );
        assert!(reader.interface.replies.is_empty());
        card.sak = 0x20;
        assert_eq!(
            reader.read_classic_block(&card, 1, KeyType::A, &[0xff; 6], || 0),
            Err(Error::UnsupportedCard)
        );
    }

    #[test]
    fn reads_all_uid_lengths_and_wakes_a_held_card_again() {
        for uid in [
            &[0xde, 0xad, 0xbe, 0xef][..],
            &[4, 1, 2, 3, 4, 5, 6],
            &[4, 1, 2, 3, 4, 5, 6, 7, 8, 9],
        ] {
            let mut bus = Bus::new();
            bus.card(uid);
            bus.card(uid);
            let mut reader = reader(bus);
            for _ in 0..2 {
                assert_eq!(reader.read_uid(|| 0).unwrap().unwrap().as_bytes(), uid);
            }
            assert!(reader.interface.replies.is_empty());
            assert_eq!(reader.interface.registers[1], 0);
            assert_eq!(reader.interface.registers[0x0d], 0);
        }
    }

    #[test]
    fn empty_field_is_not_a_fault_and_stuck_timer_times_out_across_wrap() {
        let mut bus = Bus::new();
        bus.frame(&[0x52], &[]);
        bus.replies.push_back(Reply {
            tx: vec![0x52],
            payload: vec![],
            irq: 0,
            error: 0,
        });
        let mut reader = reader(bus);
        assert_eq!(reader.read_uid(|| 0), Ok(None));
        let mut now = u32::MAX - 5_000;
        assert_eq!(
            reader.read_uid(|| {
                now = now.wrapping_add(1_000);
                now
            }),
            Err(Error::Timeout)
        );
        assert_eq!(reader.interface.registers[1], 0);
        assert_eq!(reader.interface.registers[0x0d], 0);
        assert_eq!(reader.read_uid(|| 0), Err(Error::NotInitialized));
    }

    #[test]
    fn collision_and_bad_checksums_never_publish_a_uid() {
        let mut bus = Bus::new();
        bus.replies.push_back(Reply {
            tx: vec![0x52],
            payload: vec![],
            irq: 2,
            error: 8,
        });
        let mut reader = reader(bus);
        assert_eq!(reader.read_uid(|| 0), Err(Error::Collision));
        for bad_sak in [false, true] {
            let mut bus = Bus::new();
            bus.card(&[0xde, 0xad, 0xbe, 0xef]);
            bus.replies[if bad_sak { 2 } else { 1 }].payload[if bad_sak { 2 } else { 4 }] ^= 1;
            let mut reader = super::tests::reader(bus);
            assert_eq!(reader.read_uid(|| 0), Err(Error::Checksum));
            assert_eq!(reader.interface.registers[1], 0);
        }
    }

    #[test]
    fn missing_device_and_bus_errors_require_initialization() {
        for version in [0, 0xff] {
            let mut bus = Bus::new();
            bus.registers[0x37] = version;
            let mut reader = Rc522::new(bus);
            assert_eq!(reader.initialize(), Err(Error::DeviceNotFound(version)));
            assert_eq!(reader.read_uid(|| 0), Err(Error::NotInitialized));
        }
        let mut reader = reader(Bus::new());
        reader.interface.fail = true;
        assert_eq!(reader.read_uid(|| 0), Err(Error::Interface(())));
        reader.interface.fail = false;
        assert_eq!(reader.read_uid(|| 0), Err(Error::NotInitialized));
        assert_eq!(reader.initialize(), Ok(0x92));
    }
}
