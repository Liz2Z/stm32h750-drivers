//! # 性能检测模块
//!
//! 提供代码执行时间测量和串口输出功能。
//!
//! ## 使用方法
//!
//! ```rust
//! use cortex_m::peripheral::DWT;
//!
//! // 在 main 中初始化
//! let dwt = cp.DWT;
//! dwt.enable_cycle_counter();
//!
//! // 测量代码执行时间
//! let time_us = profiler::measure_time(&mut tx, "Operation name", || {
//!     // 要测量的代码
//!     some_operation();
//! });
//! ```
//!
//! ## 启用/禁用
//!
//! 在 `Cargo.toml` 中通过 feature 控制：
//!
//! ```toml
//! [features]
//! default = []
//! profiler = []  # 启用性能检测
//! ```
//!
//! 然后运行时：
//! ```bash
//! cargo run --features profiler  # 启用性能检测
//! cargo run                      # 禁用性能检测（无开销）
//! ```

#![cfg_attr(not(feature = "profiler"), allow(dead_code))]

use cortex_m::peripheral::DWT;

// 使用 serial 模块的 SerialTx
use crate::serial::SerialTx;

/// 测量并输出耗时
///
/// # 参数
/// - `tx`: 串口发送器
/// - `name`: 操作名称
/// - `f`: 要测量的闭包
///
/// # 返回
/// 返回执行时间（微秒）
///
/// # 示例
/// ```rust
/// use profiler::measure_time;
///
/// let time_us = measure_time(&mut tx, "Clear screen", || {
///     display.clear(Rgb565::BLACK).unwrap();
/// });
/// ```
#[cfg(feature = "profiler")]
pub fn measure_time<T, F>(tx: &mut SerialTx<T>, name: &str, f: F) -> u32
where
    T: embedded_hal::serial::Write<u8, Error = core::convert::Infallible>,
    F: FnOnce(),
{
    let start = DWT::cycle_count();
    f();
    let end = DWT::cycle_count();

    let cycles = end.wrapping_sub(start);
    let time_us = cycles / 400; // 400MHz = 400 cycles/us

    tx.write_str(name);
    tx.write_str(" took: ");
    tx.write_num(time_us / 1000);
    tx.write_str(".");
    tx.write_num((time_us % 1000) / 100);
    tx.write_str(" ms\r\n");

    time_us
}

/// 禁用时的空实现（零开销）
#[cfg(not(feature = "profiler"))]
pub fn measure_time<T, F>(_tx: &mut SerialTx<T>, _name: &str, f: F) -> u32
where
    T: embedded_hal::serial::Write<u8, Error = core::convert::Infallible>,
    F: FnOnce(),
{
    f();
    0
}

/// 性能检测器结构
///
/// 提供更友好的 API，支持链式调用和上下文管理
pub struct Profiler<'a, T>
where
    T: embedded_hal::serial::Write<u8, Error = core::convert::Infallible>,
{
    tx: &'a mut SerialTx<T>,
    name: &'a str,
    start: u32,
}

impl<'a, T> Profiler<'a, T>
where
    T: embedded_hal::serial::Write<u8, Error = core::convert::Infallible>,
{
    /// 创建新的性能检测器并开始计时
    #[cfg(feature = "profiler")]
    pub fn new(tx: &'a mut SerialTx<T>, name: &'a str) -> Self {
        let start = DWT::cycle_count();
        Self { tx, name, start }
    }

    /// 创建新的性能检测器（禁用版本）
    #[cfg(not(feature = "profiler"))]
    pub fn new(tx: &'a mut SerialTx<T>, name: &'a str) -> Self {
        Self { tx, name, start: 0 }
    }

    /// 结束计时并输出结果
    #[cfg(feature = "profiler")]
    pub fn finish(mut self) -> u32 {
        let end = DWT::cycle_count();
        let cycles = end.wrapping_sub(self.start);
        let time_us = cycles / 400;

        self.tx.write_str(self.name);
        self.tx.write_str(" took: ");
        self.tx.write_num(time_us / 1000);
        self.tx.write_str(".");
        self.tx.write_num((time_us % 1000) / 100);
        self.tx.write_str(" ms\r\n");

        time_us
    }

    /// 结束计时（禁用版本）
    #[cfg(not(feature = "profiler"))]
    pub fn finish(self) -> u32 {
        0
    }
}

impl<'a, T> Drop for Profiler<'a, T>
where
    T: embedded_hal::serial::Write<u8, Error = core::convert::Infallible>,
{
    /// 自动在作用域结束时输出结果
    #[cfg(feature = "profiler")]
    fn drop(&mut self) {
        let end = DWT::cycle_count();
        let cycles = end.wrapping_sub(self.start);
        let time_us = cycles / 400;

        self.tx.write_str(self.name);
        self.tx.write_str(" took: ");
        self.tx.write_num(time_us / 1000);
        self.tx.write_str(".");
        self.tx.write_num((time_us % 1000) / 100);
        self.tx.write_str(" ms\r\n");
    }

    #[cfg(not(feature = "profiler"))]
    fn drop(&mut self) {}
}

/// 便利宏：创建一个自动计时的 Profiler
///
/// # 示例
/// ```rust
/// let _profiler = profiler::scope!("Operation name", tx);
/// // 代码执行...
/// // 作用域结束时自动输出耗时
/// ```
#[macro_export]
macro_rules! scope {
    ($name:expr, $tx:expr) => {
        $crate::profiler::Profiler::new($tx, $name)
    };
}
