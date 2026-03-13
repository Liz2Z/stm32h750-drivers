//! RC522 RFID 读卡器驱动（SPI）
//!
//! 支持 ISO14443A 的基础流程：
//! - 读卡请求（REQA）
//! - 防冲突获取 UID
//! - 选卡
//! - MIFARE Classic 扇区认证
//! - 读块 / 写块

use embedded_hal::blocking::spi::{Transfer, Write};
use embedded_hal::digital::v2::OutputPin;

const REG_COMMAND: u8 = 0x01;
const REG_COMM_IE_N: u8 = 0x02;
const REG_COMM_IRQ: u8 = 0x04;
const REG_DIV_IRQ: u8 = 0x05;
const REG_ERROR: u8 = 0x06;
const REG_STATUS2: u8 = 0x08;
const REG_FIFO_DATA: u8 = 0x09;
const REG_FIFO_LEVEL: u8 = 0x0A;
const REG_CONTROL: u8 = 0x0C;
const REG_BIT_FRAMING: u8 = 0x0D;
const REG_MODE: u8 = 0x11;
const REG_TX_MODE: u8 = 0x12;
const REG_RX_MODE: u8 = 0x13;
const REG_TX_CONTROL: u8 = 0x14;
const REG_T_MODE: u8 = 0x2A;
const REG_T_PRESCALER: u8 = 0x2B;
const REG_T_RELOAD_H: u8 = 0x2C;
const REG_T_RELOAD_L: u8 = 0x2D;
const REG_VERSION: u8 = 0x37;

const CMD_IDLE: u8 = 0x00;
const CMD_CALC_CRC: u8 = 0x03;
const CMD_TRANSCEIVE: u8 = 0x0C;
const CMD_MF_AUTHENT: u8 = 0x0E;
const CMD_SOFT_RESET: u8 = 0x0F;

const PICC_REQA: u8 = 0x26;
const PICC_ANTICOLL_CL1: u8 = 0x93;
const PICC_SELECT_CL1: u8 = 0x93;
const PICC_AUTH_KEY_A: u8 = 0x60;
const PICC_AUTH_KEY_B: u8 = 0x61;
const PICC_READ: u8 = 0x30;
const PICC_WRITE: u8 = 0xA0;
const PICC_HALT: u8 = 0x50;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rc522Error {
    Spi,
    Pin,
    Timeout,
    Collision,
    Crc,
    Protocol,
    Authentication,
    BufferTooSmall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MifareKeyType {
    A,
    B,
}

pub struct Rc522<SPI, CS> {
    spi: SPI,
    cs: CS,
}

impl<SPI, CS, E> Rc522<SPI, CS>
where
    SPI: Transfer<u8, Error = E> + Write<u8, Error = E>,
    CS: OutputPin,
{
    pub fn new(spi: SPI, cs: CS) -> Self {
        Self { spi, cs }
    }

    pub fn init(&mut self) -> Result<(), Rc522Error> {
        self.reset()?;

        self.write_register(REG_T_MODE, 0x8D)?;
        self.write_register(REG_T_PRESCALER, 0x3E)?;
        self.write_register(REG_T_RELOAD_L, 30)?;
        self.write_register(REG_T_RELOAD_H, 0)?;

        self.write_register(REG_TX_MODE, 0x00)?;
        self.write_register(REG_RX_MODE, 0x00)?;
        self.write_register(REG_MODE, 0x3D)?;

        self.antenna_on()
    }

    pub fn version(&mut self) -> Result<u8, Rc522Error> {
        self.read_register(REG_VERSION)
    }

    pub fn request_a(&mut self) -> Result<[u8; 2], Rc522Error> {
        self.write_register(REG_BIT_FRAMING, 0x07)?;

        let mut back = [0u8; 16];
        let (len, valid_bits) = self.transceive_raw(&[PICC_REQA], &mut back)?;

        if len != 2 || valid_bits != 0 {
            return Err(Rc522Error::Protocol);
        }

        Ok([back[0], back[1]])
    }

    pub fn anticollision_cl1(&mut self) -> Result<[u8; 4], Rc522Error> {
        self.write_register(REG_BIT_FRAMING, 0x00)?;

        let mut back = [0u8; 16];
        let (len, _) = self.transceive_raw(&[PICC_ANTICOLL_CL1, 0x20], &mut back)?;
        if len != 5 {
            return Err(Rc522Error::Protocol);
        }

        let bcc = back[0] ^ back[1] ^ back[2] ^ back[3];
        if bcc != back[4] {
            return Err(Rc522Error::Crc);
        }

        Ok([back[0], back[1], back[2], back[3]])
    }

    pub fn select_cl1(&mut self, uid: [u8; 4]) -> Result<u8, Rc522Error> {
        let mut frame = [0u8; 9];
        frame[0] = PICC_SELECT_CL1;
        frame[1] = 0x70;
        frame[2..6].copy_from_slice(&uid);
        frame[6] = uid[0] ^ uid[1] ^ uid[2] ^ uid[3];

        let crc = self.calculate_crc(&frame[..7])?;
        frame[7] = crc[0];
        frame[8] = crc[1];

        let mut back = [0u8; 3];
        let (len, valid_bits) = self.transceive_raw(&frame, &mut back)?;
        if len != 1 || valid_bits != 0 {
            return Err(Rc522Error::Protocol);
        }

        Ok(back[0])
    }

    pub fn authenticate(
        &mut self,
        key_type: MifareKeyType,
        block_addr: u8,
        key: [u8; 6],
        uid: [u8; 4],
    ) -> Result<(), Rc522Error> {
        let mut packet = [0u8; 12];
        packet[0] = match key_type {
            MifareKeyType::A => PICC_AUTH_KEY_A,
            MifareKeyType::B => PICC_AUTH_KEY_B,
        };
        packet[1] = block_addr;
        packet[2..8].copy_from_slice(&key);
        packet[8..12].copy_from_slice(&uid);

        self.communicate_with_picc(CMD_MF_AUTHENT, &packet, &mut [])?;

        let status2 = self.read_register(REG_STATUS2)?;
        if status2 & 0x08 == 0 {
            return Err(Rc522Error::Authentication);
        }

        Ok(())
    }

    pub fn stop_crypto(&mut self) -> Result<(), Rc522Error> {
        self.clear_bit_mask(REG_STATUS2, 0x08)
    }

    pub fn read_block(&mut self, block_addr: u8) -> Result<[u8; 16], Rc522Error> {
        let mut cmd = [PICC_READ, block_addr, 0, 0];
        let crc = self.calculate_crc(&cmd[..2])?;
        cmd[2] = crc[0];
        cmd[3] = crc[1];

        let mut back = [0u8; 18];
        let (len, valid_bits) = self.transceive_raw(&cmd, &mut back)?;
        if len != 16 || valid_bits != 0 {
            return Err(Rc522Error::Protocol);
        }

        let mut data = [0u8; 16];
        data.copy_from_slice(&back[..16]);
        Ok(data)
    }

    pub fn write_block(&mut self, block_addr: u8, data: [u8; 16]) -> Result<(), Rc522Error> {
        let mut cmd = [PICC_WRITE, block_addr, 0, 0];
        let crc = self.calculate_crc(&cmd[..2])?;
        cmd[2] = crc[0];
        cmd[3] = crc[1];

        let mut ack = [0u8; 1];
        let (_, valid_bits) = self.transceive_raw(&cmd, &mut ack)?;
        if valid_bits != 4 || (ack[0] & 0x0F) != 0x0A {
            return Err(Rc522Error::Protocol);
        }

        let mut payload = [0u8; 18];
        payload[..16].copy_from_slice(&data);
        let crc = self.calculate_crc(&payload[..16])?;
        payload[16] = crc[0];
        payload[17] = crc[1];

        let (_, valid_bits) = self.transceive_raw(&payload, &mut ack)?;
        if valid_bits != 4 || (ack[0] & 0x0F) != 0x0A {
            return Err(Rc522Error::Protocol);
        }

        Ok(())
    }

    pub fn halt(&mut self) -> Result<(), Rc522Error> {
        let mut cmd = [PICC_HALT, 0x00, 0x00, 0x00];
        let crc = self.calculate_crc(&cmd[..2])?;
        cmd[2] = crc[0];
        cmd[3] = crc[1];

        let _ = self.transceive_raw(&cmd, &mut [0u8; 1]);
        Ok(())
    }

    pub fn release(self) -> (SPI, CS) {
        (self.spi, self.cs)
    }

    fn reset(&mut self) -> Result<(), Rc522Error> {
        self.write_register(REG_COMMAND, CMD_SOFT_RESET)?;
        self.delay_ms(50);
        Ok(())
    }

    fn antenna_on(&mut self) -> Result<(), Rc522Error> {
        let value = self.read_register(REG_TX_CONTROL)?;
        if value & 0x03 != 0x03 {
            self.set_bit_mask(REG_TX_CONTROL, 0x03)?;
        }
        Ok(())
    }

    fn communicate_with_picc(
        &mut self,
        command: u8,
        send_data: &[u8],
        back_data: &mut [u8],
    ) -> Result<(usize, u8), Rc522Error> {
        let irq_en = if command == CMD_MF_AUTHENT {
            0x12
        } else {
            0x77
        };
        let wait_irq = if command == CMD_MF_AUTHENT {
            0x10
        } else {
            0x30
        };

        self.write_register(REG_COMM_IE_N, irq_en | 0x80)?;
        self.clear_bit_mask(REG_COMM_IRQ, 0x80)?;
        self.set_bit_mask(REG_FIFO_LEVEL, 0x80)?;

        self.write_register(REG_COMMAND, CMD_IDLE)?;
        for &b in send_data {
            self.write_register(REG_FIFO_DATA, b)?;
        }

        self.write_register(REG_COMMAND, command)?;
        if command == CMD_TRANSCEIVE {
            self.set_bit_mask(REG_BIT_FRAMING, 0x80)?;
        }

        let mut timeout = 2500;
        loop {
            let irq = self.read_register(REG_COMM_IRQ)?;
            if irq & wait_irq != 0 {
                break;
            }
            if irq & 0x01 != 0 {
                return Err(Rc522Error::Timeout);
            }

            timeout -= 1;
            if timeout == 0 {
                return Err(Rc522Error::Timeout);
            }
        }

        self.clear_bit_mask(REG_BIT_FRAMING, 0x80)?;

        let err = self.read_register(REG_ERROR)?;
        if err & 0x13 != 0 {
            if err & 0x08 != 0 {
                return Err(Rc522Error::Collision);
            }
            return Err(Rc522Error::Protocol);
        }

        if command == CMD_TRANSCEIVE {
            let fifo_level = self.read_register(REG_FIFO_LEVEL)? as usize;
            if fifo_level > back_data.len() {
                return Err(Rc522Error::BufferTooSmall);
            }

            for byte in back_data.iter_mut().take(fifo_level) {
                *byte = self.read_register(REG_FIFO_DATA)?;
            }

            let valid_bits = self.read_register(REG_CONTROL)? & 0x07;
            Ok((fifo_level, valid_bits))
        } else {
            Ok((0, 0))
        }
    }

    fn transceive_raw(
        &mut self,
        data: &[u8],
        back_data: &mut [u8],
    ) -> Result<(usize, u8), Rc522Error> {
        self.communicate_with_picc(CMD_TRANSCEIVE, data, back_data)
    }

    fn calculate_crc(&mut self, data: &[u8]) -> Result<[u8; 2], Rc522Error> {
        self.write_register(REG_COMMAND, CMD_IDLE)?;
        self.clear_bit_mask(REG_DIV_IRQ, 0x04)?;
        self.set_bit_mask(REG_FIFO_LEVEL, 0x80)?;

        for &b in data {
            self.write_register(REG_FIFO_DATA, b)?;
        }

        self.write_register(REG_COMMAND, CMD_CALC_CRC)?;

        let mut timeout = 0xFF;
        loop {
            let n = self.read_register(REG_DIV_IRQ)?;
            if n & 0x04 != 0 {
                break;
            }
            timeout -= 1;
            if timeout == 0 {
                return Err(Rc522Error::Timeout);
            }
        }

        let lsb = self.read_register(0x22)?;
        let msb = self.read_register(0x21)?;
        Ok([lsb, msb])
    }

    fn write_register(&mut self, reg: u8, value: u8) -> Result<(), Rc522Error> {
        let addr = (reg << 1) & 0x7E;
        self.cs.set_low().map_err(|_| Rc522Error::Pin)?;
        let ret = self.spi.write(&[addr, value]).map_err(|_| Rc522Error::Spi);
        self.cs.set_high().map_err(|_| Rc522Error::Pin)?;
        ret
    }

    fn read_register(&mut self, reg: u8) -> Result<u8, Rc522Error> {
        let addr = ((reg << 1) & 0x7E) | 0x80;
        let mut frame = [addr, 0x00];

        self.cs.set_low().map_err(|_| Rc522Error::Pin)?;
        let ret = self.spi.transfer(&mut frame).map_err(|_| Rc522Error::Spi);
        self.cs.set_high().map_err(|_| Rc522Error::Pin)?;

        ret.map(|buf| buf[1])
    }

    fn set_bit_mask(&mut self, reg: u8, mask: u8) -> Result<(), Rc522Error> {
        let value = self.read_register(reg)?;
        self.write_register(reg, value | mask)
    }

    fn clear_bit_mask(&mut self, reg: u8, mask: u8) -> Result<(), Rc522Error> {
        let value = self.read_register(reg)?;
        self.write_register(reg, value & !mask)
    }

    fn delay_ms(&self, ms: u32) {
        cortex_m::asm::delay(ms * 400_000);
    }
}
