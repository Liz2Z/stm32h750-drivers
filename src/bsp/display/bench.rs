//! One-shot hardware timing; excluded from normal firmware.
use super::DisplayDriver;
use crate::{bsp::clock::Clock, ui::GrayTheme};
use core::cell::UnsafeCell;
use embedded_graphics::prelude::*;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Measurement {
    pub width: u32,
    pub height: u32,
    pub attempts: u32,
    pub successes: u32,
    pub failures: u32,
    pub min_us: u32,
    pub max_us: u32,
    pub total_us: u64,
}

impl Measurement {
    const EMPTY: Self = Self {
        width: 0,
        height: 0,
        attempts: 0,
        successes: 0,
        failures: 0,
        min_us: 0,
        max_us: 0,
        total_us: 0,
    };
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct BenchResults {
    /// 0: not started, 1: running, 2: complete, 3: display init failed.
    pub status: u32,
    pub spi_kernel_hz: u32,
    pub spi_cfg1: u32,
    pub spi_sck_hz: u32,
    pub scb_ccr: u32,
    pub framebuffer_clear: Measurement,
    pub full_frame_transfer: Measurement,
    pub three_cards_transfer: Measurement,
    pub animation_strip_transfer: Measurement,
    pub draw_and_full_frame: Measurement,
}

#[repr(transparent)]
pub struct DebugSnapshot(UnsafeCell<BenchResults>);
// SAFETY: only the main thread publishes. GDB reads while the core is halted.
unsafe impl Sync for DebugSnapshot {}

#[used]
#[no_mangle]
pub static DISPLAY_BENCH_RESULTS: DebugSnapshot = DebugSnapshot(UnsafeCell::new(BenchResults {
    status: 0,
    spi_kernel_hz: 0,
    spi_cfg1: 0,
    spi_sck_hz: 0,
    scb_ccr: 0,
    framebuffer_clear: Measurement::EMPTY,
    full_frame_transfer: Measurement::EMPTY,
    three_cards_transfer: Measurement::EMPTY,
    animation_strip_transfer: Measurement::EMPTY,
    draw_and_full_frame: Measurement::EMPTY,
}));

fn publish(results: BenchResults) {
    // Only the main thread writes; the debugger reads after status becomes 2.
    // SAFETY: one main-thread writer; no live Rust references to the snapshot.
    unsafe { core::ptr::write_volatile(DISPLAY_BENCH_RESULTS.0.get(), results) };
}

fn measure(
    clock: &Clock,
    width: u32,
    height: u32,
    attempts: u32,
    mut operation: impl FnMut() -> bool,
) -> Measurement {
    let mut stats = Measurement {
        width,
        height,
        attempts,
        min_us: u32::MAX,
        ..Measurement::EMPTY
    };
    for _ in 0..attempts {
        let start = clock.now_us();
        let ok = operation();
        let elapsed = clock.now_us().wrapping_sub(start);
        if ok {
            stats.successes += 1;
            stats.min_us = stats.min_us.min(elapsed);
            stats.max_us = stats.max_us.max(elapsed);
            stats.total_us += u64::from(elapsed);
        } else {
            stats.failures += 1;
        }
    }
    if stats.successes == 0 {
        stats.min_us = 0;
    }
    stats
}

pub fn run(display: &mut DisplayDriver<'_>, clock: &Clock, spi_kernel_hz: u32, ready: bool) {
    // SAFETY: the main thread owns SPI2; this reads a mapped, read-only snapshot.
    let spi_cfg1 = unsafe { (*stm32h7xx_hal::pac::SPI2::ptr()).cfg1.read().bits() };
    let mut results = BenchResults {
        status: if ready { 1 } else { 3 },
        spi_kernel_hz,
        spi_cfg1,
        spi_sck_hz: spi_kernel_hz / (1 << (((spi_cfg1 >> 28) & 7) + 1)),
        // SAFETY: Cortex-M7 CCR is a valid, readable system register.
        scb_ccr: unsafe { core::ptr::read_volatile(0xe000_ed14 as *const u32) },
        framebuffer_clear: Measurement::EMPTY,
        full_frame_transfer: Measurement::EMPTY,
        three_cards_transfer: Measurement::EMPTY,
        animation_strip_transfer: Measurement::EMPTY,
        draw_and_full_frame: Measurement::EMPTY,
    };
    publish(results);
    if !ready {
        return;
    }
    let theme = GrayTheme::new();
    let _ = display.clear(theme.background());
    // Warm up the same transfer path, then time without breakpoints or delays.
    let _ = display.flush();
    results.framebuffer_clear = measure(clock, 320, 240, 64, || {
        core::hint::black_box(&mut *display)
            .clear(theme.background())
            .is_ok()
    });
    results.full_frame_transfer = measure(clock, 320, 240, 32, || display.flush().is_ok());
    results.three_cards_transfer = measure(clock, 98 * 3, 115, 64, || {
        display.flush_rect(6, 95, 98, 115).is_ok()
            && display.flush_rect(111, 95, 98, 115).is_ok()
            && display.flush_rect(216, 95, 98, 115).is_ok()
    });
    results.animation_strip_transfer = measure(clock, 310, 14, 128, || {
        display.flush_rect(5, 3, 310, 14).is_ok()
    });
    results.draw_and_full_frame = measure(clock, 320, 240, 32, || {
        display.clear(theme.background()).is_ok() && display.flush().is_ok()
    });
    results.status = 2;
    publish(results);
    // Resume the usual UI so the one-shot benchmark never strands the board.
}
