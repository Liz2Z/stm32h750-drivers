// src/rc522.rs

use crate::types::{CardType, KeyType, Error};
use embedded_hal::spi::SpiBus;
use embedded_hal::digital::OutputPin;

// RC522 寄存器地址
const COMMAND_REG: u8 = 0x01 << 3;
const COM_I_EN_REG: u8 = 0x02 << 3;
const DIV_IRQ_REG: u8 = 0x03 << 3;
const COM_IRQ_REG: u8 = 0x04 << 3;
const ERROR_REG: u8 = 0x06 << 3;
const FIFOLEVEL_REG: u8 = 0x0A << 3;
const FIFO_DATA_REG: u8 = 0x09 << 3;
const STATUS_REG: u8 = 0x07 << 3;
const BIT_FRAMING_REG: u8 = 0x0D << 3;

// RC522 命令
const CMD_IDLE: u8 = 0x00;
const CMD_TRANSCEIVE: u8 = 0x0C;
const CMD_AUTHENT: u8 = 0x0E;
const CMD_SOFT_RESET: u8 = 0x0F;

// Mifare 命令
const PICC_REQALL: u8 = 0x52;
const PICC_ANTICOLL1: u8 = 0x93;
const PICC_AUTHENT1A: u8 = 0x60;
const PICC_AUTHENT1B: u8 = 0x61;
const PICC_READ: u8 = 0x30;
const PICC_WRITE: u8 = 0xA0;

pub struct RC522<SPI, RST> {
    spi: SPI,
    rst: RST,
}

impl<SPI, E, RST> RC522<SPI, RST>
where
    SPI: SpiBus<Error = E>,
    RST: OutputPin<Error = E>,
{
    pub fn new(mut spi: SPI, mut rst: RST) -> Result<Self, Error<E>> {
        // 硬件复位
        rst.set_low().map_err(Error::Spi)?;
        cortex_m::asm::delay(10_000);
        rst.set_high().map_err(Error::Spi)?;
        cortex_m::asm::delay(100_000);

        let mut rc522 = Self { spi, rst };

        // 软复位
        rc522.write_reg(COMMAND_REG, CMD_SOFT_RESET)?;

        // 配置
        rc522.write_reg(COM_I_EN_REG, 0x7F)?;
        rc522.write_reg(DIV_IRQ_REG, 0x00)?;

        // 开启天线
        rc522.set_antenna(true)?;

        Ok(rc522)
    }

    fn read_reg(&mut self, reg: u8) -> Result<u8, Error<E>> {
        let addr = 0x80 | reg;
        let mut tx = [addr, 0x00];
        let mut rx = [0u8; 2];

        self.spi.transfer(&mut rx, &tx).map_err(Error::Spi)?;
        Ok(rx[1])
    }

    fn write_reg(&mut self, reg: u8, val: u8) -> Result<(), Error<E>> {
        let addr = reg & 0x7F;
        let tx = [addr, val];
        let mut rx = [0u8; 2];

        self.spi.transfer(&mut rx, &tx).map_err(Error::Spi)?;
        Ok(())
    }

    fn set_antenna(&mut self, on: bool) -> Result<(), Error<E>> {
        let val = if on { 0x03 } else { 0x00 };
        self.write_reg(0x26, val)
    }

    pub fn request(&mut self) -> Result<CardType, Error<E>> {
        self.write_reg(BIT_FRAMING_REG, 0x07)?;

        let cmd = PICC_REQALL;
        self.write_reg(COMMAND_REG, CMD_IDLE)?;
        self.write_reg(FIFO_DATA_REG, cmd)?;
        self.write_reg(COMMAND_REG, CMD_TRANSCEIVE)?;
        self.write_reg(BIT_FRAMING_REG, 0x80 | 0x07)?;

        // 等待完成
        let mut timeout = 1000;
        while self.read_reg(COM_IRQ_REG)? & 0x01 == 0 {
            timeout -= 1;
            if timeout == 0 {
                return Err(Error::Timeout);
            }
        }

        let status = self.read_reg(ERROR_REG)?;
        if status & 0x08 != 0 {
            return Err(Error::Collision);
        }

        let n = self.read_reg(FIFOLEVEL_REG)?;
        if n != 2 {
            return Err(Error::NoCard);
        }

        let byte1 = self.read_reg(FIFO_DATA_REG)?;
        let byte2 = self.read_reg(FIFO_DATA_REG)?;

        match (byte1, byte2) {
            (0x02, 0x00) => Ok(CardType::Mifare1K),
            (0x04, 0x00) => Ok(CardType::MifareUltralight),
            (0x02, 0x04) => Ok(CardType::Mifare4K),
            _ => Ok(CardType::Unknown),
        }
    }

    pub fn anticoll(&mut self) -> Result<[u8; 4], Error<E>> {
        let mut uid = [0u8; 4];

        self.write_reg(BIT_FRAMING_REG, 0x00)?;
        self.write_reg(FIFOLEVEL_REG, 0x80)?;

        self.write_reg(FIFO_DATA_REG, PICC_ANTICOLL1)?;
        self.write_reg(FIFO_DATA_REG, 0x20)?;

        self.write_reg(COMMAND_REG, CMD_TRANSCEIVE)?;
        self.write_reg(BIT_FRAMING_REG, 0x80)?;

        let mut timeout = 1000;
        while self.read_reg(COM_IRQ_REG)? & 0x01 == 0 {
            timeout -= 1;
            if timeout == 0 {
                return Err(Error::Timeout);
            }
        }

        let n = self.read_reg(FIFOLEVEL_REG)?;
        if n != 5 {
            return Err(Error::Collision);
        }

        for i in 0..5 {
            let byte = self.read_reg(FIFO_DATA_REG)?;
            if i < 4 {
                uid[i] = byte;
            }
        }

        Ok(uid)
    }

    pub fn authenticate(
        &mut self,
        block: u8,
        key_type: KeyType,
        key: &[u8; 6],
        uid: &[u8; 4],
    ) -> Result<(), Error<E>> {
        let cmd = match key_type {
            KeyType::KeyA => PICC_AUTHENT1A,
            KeyType::KeyB => PICC_AUTHENT1B,
        };

        self.write_reg(FIFOLEVEL_REG, 0x80)?;
        self.write_reg(FIFO_DATA_REG, cmd)?;
        self.write_reg(FIFO_DATA_REG, block)?;

        for &b in key.iter() {
            self.write_reg(FIFO_DATA_REG, b)?;
        }
        for &b in uid.iter() {
            self.write_reg(FIFO_DATA_REG, b)?;
        }

        self.write_reg(COMMAND_REG, CMD_AUTHENT)?;
        self.write_reg(BIT_FRAMING_REG, 0x00)?;

        let mut timeout = 5000;
        while self.read_reg(COM_IRQ_REG)? & 0x01 == 0 {
            timeout -= 1;
            if timeout == 0 {
                return Err(Error::Timeout);
            }
        }

        let status = self.read_reg(STATUS_REG)?;
        if status & 0x08 != 0 {
            return Err(Error::Timeout);
        }

        Ok(())
    }

    pub fn read(&mut self, block: u8) -> Result<[u8; 16], Error<E>> {
        let mut data = [0u8; 16];

        self.write_reg(FIFOLEVEL_REG, 0x80)?;
        self.write_reg(COMMAND_REG, CMD_IDLE)?;
        self.write_reg(FIFO_DATA_REG, PICC_READ)?;
        self.write_reg(FIFO_DATA_REG, block)?;

        self.write_reg(COMMAND_REG, CMD_TRANSCEIVE)?;
        self.write_reg(BIT_FRAMING_REG, 0x80)?;

        let mut timeout = 5000;
        while self.read_reg(COM_IRQ_REG)? & 0x01 == 0 {
            timeout -= 1;
            if timeout == 0 {
                return Err(Error::Timeout);
            }
        }

        let error = self.read_reg(ERROR_REG)?;
        if error & 0x08 != 0 {
            return Err(Error::Collision);
        }

        let n = self.read_reg(FIFOLEVEL_REG)?;
        if n < 18 {
            return Err(Error::NoCard);
        }

        for i in 0..16 {
            data[i] = self.read_reg(FIFO_DATA_REG)?;
        }

        Ok(data)
    }

    pub fn write(&mut self, block: u8, data: &[u8; 16]) -> Result<(), Error<E>> {
        self.write_reg(FIFOLEVEL_REG, 0x80)?;
        self.write_reg(COMMAND_REG, CMD_IDLE)?;
        self.write_reg(FIFO_DATA_REG, PICC_WRITE)?;
        self.write_reg(FIFO_DATA_REG, block)?;

        self.write_reg(COMMAND_REG, CMD_TRANSCEIVE)?;
        self.write_reg(BIT_FRAMING_REG, 0x80)?;

        let mut timeout = 5000;
        while self.read_reg(COM_IRQ_REG)? & 0x01 == 0 {
            timeout -= 1;
            if timeout == 0 {
                return Err(Error::Timeout);
            }
        }

        let n = self.read_reg(FIFOLEVEL_REG)?;
        if n != 1 {
            return Err(Error::Collision);
        }

        let byte = self.read_reg(FIFO_DATA_REG)?;
        if byte != 0x0A {
            return Err(Error::Collision);
        }

        self.write_reg(COMMAND_REG, CMD_IDLE)?;
        self.write_reg(FIFOLEVEL_REG, 0x80)?;

        for &b in data.iter() {
            self.write_reg(FIFO_DATA_REG, b)?;
        }

        self.write_reg(COMMAND_REG, CMD_TRANSCEIVE)?;
        self.write_reg(BIT_FRAMING_REG, 0x80)?;

        let mut timeout = 5000;
        while self.read_reg(COM_IRQ_REG)? & 0x01 == 0 {
            timeout -= 1;
            if timeout == 0 {
                return Err(Error::Timeout);
            }
        }

        let error = self.read_reg(ERROR_REG)?;
        if error & 0x08 != 0 {
            return Err(Error::Collision);
        }

        Ok(())
    }
}
