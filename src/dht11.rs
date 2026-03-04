//! DHT11 温湿度传感器驱动
//!
//! DHT11 使用单总线协议通信，时序要求：
//! - 启动信号：拉低至少 18ms，然后释放
//! - 响应信号：DHT11 拉低 80μs，然后拉高 80μs
//! - 数据格式：40位（湿度整数 + 湿度小数 + 温度整数 + 温度小数 + 校验和）
//!
//! 硬件连接：
//! - DATA 引脚需要 5K 上拉电阻
//! - 使用开漏输出模式（OpenDrain）实现双向通信

use embedded_hal::digital::v2::{InputPin, OutputPin};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DhtError {
    Timeout,
    ChecksumMismatch,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DhtReading {
    pub temperature: f32,
    pub humidity: f32,
}

pub struct Dht11<Pin> {
    pin: Pin,
}

impl<Pin, E> Dht11<Pin>
where
    Pin: OutputPin<Error = E> + InputPin<Error = E>,
{
    pub fn new(pin: Pin) -> Self {
        Self { pin }
    }

    pub fn read(&mut self) -> Result<DhtReading, DhtError> {
        let data = self.read_raw()?;

        Ok(DhtReading {
            temperature: data[2] as f32 + data[3] as f32 * 0.1,
            humidity: data[0] as f32 + data[1] as f32 * 0.1,
        })
    }

    fn read_raw(&mut self) -> Result<[u8; 5], DhtError> {
        self.send_start_signal()?;

        self.wait_for_response()?;

        let mut data = [0u8; 5];
        for i in 0..5 {
            data[i] = self.read_byte()?;
        }

        let checksum = data[0]
            .wrapping_add(data[1])
            .wrapping_add(data[2])
            .wrapping_add(data[3]);

        if checksum != data[4] {
            return Err(DhtError::ChecksumMismatch);
        }

        Ok(data)
    }

    fn send_start_signal(&mut self) -> Result<(), DhtError> {
        let _ = self.pin.set_low();
        delay_ms(20);

        let _ = self.pin.set_high();
        delay_us(40);

        Ok(())
    }

    fn wait_for_response(&mut self) -> Result<(), DhtError> {
        let mut timeout = 1000;
        while self.pin.is_high().unwrap_or(true) {
            timeout -= 1;
            if timeout == 0 {
                return Err(DhtError::Timeout);
            }
            delay_us(1);
        }

        timeout = 1000;
        while self.pin.is_low().unwrap_or(true) {
            timeout -= 1;
            if timeout == 0 {
                return Err(DhtError::Timeout);
            }
            delay_us(1);
        }

        timeout = 1000;
        while self.pin.is_high().unwrap_or(true) {
            timeout -= 1;
            if timeout == 0 {
                return Err(DhtError::Timeout);
            }
            delay_us(1);
        }

        Ok(())
    }

    fn read_byte(&mut self) -> Result<u8, DhtError> {
        let mut byte = 0u8;

        for _ in 0..8 {
            let mut timeout = 1000;
            while self.pin.is_low().unwrap_or(true) {
                timeout -= 1;
                if timeout == 0 {
                    return Err(DhtError::Timeout);
                }
                delay_us(1);
            }

            delay_us(28);

            if self.pin.is_high().unwrap_or(false) {
                byte = (byte << 1) | 1;

                timeout = 1000;
                while self.pin.is_high().unwrap_or(false) {
                    timeout -= 1;
                    if timeout == 0 {
                        return Err(DhtError::Timeout);
                    }
                    delay_us(1);
                }
            } else {
                byte <<= 1;
            }
        }

        Ok(byte)
    }
}

fn delay_ms(ms: u32) {
    for _ in 0..ms {
        for _ in 0..8000 {
            cortex_m::asm::nop();
        }
    }
}

fn delay_us(us: u32) {
    for _ in 0..us {
        for _ in 0..8 {
            cortex_m::asm::nop();
        }
    }
}
