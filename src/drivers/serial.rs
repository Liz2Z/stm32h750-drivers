//! # 串口通信模块
//!
//! 提供简单的串口输出功能，支持字符串和数字输出。
//!
//! ## 使用方法
//!
//! ```rust
//! use serial::SerialTx;
//!
//! // 创建串口发送器
//! let tx = SerialTx::new(usart1_tx);
//!
//! // 输出字符串
//! tx.write_str("Hello, World!\r\n");
//!
//! // 输出数字
//! tx.write_num(12345);
//! ```
//!
//! 注意：此模块主要用于 profiler 功能，在非 profiler 模式下可能未使用

#![allow(dead_code)]

use embedded_hal::serial::Write as HalWrite;

/// 串口发送器封装
pub struct SerialTx<T> {
    tx: T,
}

impl<T> SerialTx<T>
where
    T: HalWrite<u8, Error = core::convert::Infallible>,
{
    /// 创建新的串口发送器
    pub fn new(tx: T) -> Self {
        Self { tx }
    }

    /// 写入单个字节
    pub fn write_byte(&mut self, byte: u8) -> nb::Result<(), core::convert::Infallible> {
        self.tx.write(byte)
    }

    /// 写入字符串
    pub fn write_str(&mut self, s: &str) {
        for b in s.bytes() {
            let _ = self.write_byte(b);
        }
    }

    /// 写入数字（十进制）
    pub fn write_num(&mut self, mut n: u32) {
        let mut buf = [0u8; 12];
        let mut i = 0;

        if n == 0 {
            buf[i] = b'0';
            i = 1;
        } else {
            let mut temp = n;
            let mut len = 0;
            while temp > 0 {
                len += 1;
                temp /= 10;
            }
            i = len;
            while n > 0 {
                i -= 1;
                buf[i] = b'0' + (n % 10) as u8;
                n /= 10;
            }
            i = len;
        }

        for &byte in buf.iter().take(i) {
            let _ = self.write_byte(byte);
        }
    }

    /// 写入十六进制数字
    pub fn write_hex(&mut self, mut n: u32) {
        const HEX_CHARS: &[u8; 16] = b"0123456789ABCDEF";

        let mut buf = [0u8; 8];
        let mut i = 0;

        if n == 0 {
            buf[i] = b'0';
            i = 1;
        } else {
            while n > 0 {
                buf[i] = HEX_CHARS[(n & 0xF) as usize];
                n >>= 4;
                i += 1;
            }
        }

        // 反转输出（高位在前）
        for j in (0..i).rev() {
            let _ = self.write_byte(buf[j]);
        }
    }

    /// 写入换行
    pub fn write_line(&mut self, s: &str) {
        self.write_str(s);
        self.write_str("\r\n");
    }

    /// 获取内部发送器的引用
    pub fn inner(&mut self) -> &mut T {
        &mut self.tx
    }

    /// 释放内部发送器
    pub fn release(self) -> T {
        self.tx
    }
}

/// 实现 core::fmt::Write trait，支持格式化输出
impl<T> core::fmt::Write for SerialTx<T>
where
    T: HalWrite<u8, Error = core::convert::Infallible>,
{
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for b in s.bytes() {
            let _ = self.write_byte(b);
        }
        Ok(())
    }
}

/// 便利宏：格式化输出到串口
#[macro_export]
macro_rules! serial_print {
    ($tx:expr, $($arg:tt)*) => {
        use core::fmt::Write;
        let _ = write!($tx, $($arg)*);
    };
}

/// 便利宏：格式化输出到串口（带换行）
#[macro_export]
macro_rules! serial_println {
    ($tx:expr, $($arg:tt)*) => {
        use core::fmt::Write;
        let _ = writeln!($tx, $($arg)*);
    };
}
