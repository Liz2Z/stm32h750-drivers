//! Exclusive AXI SRAM storage for the framebuffer and packed DMA pixels.
use core::mem::MaybeUninit;

use crate::drivers::display::{DMA_BUF_WORDS, MAX_FRAME_BUFFER_SIZE};
use core::cell::UnsafeCell;

struct Buffer<T>(UnsafeCell<MaybeUninit<T>>);
// SAFETY: only take_buffers accesses these cells, guarded by a BSS AtomicBool.
// Their references are handed to one display instance exactly once.
unsafe impl<T: Send> Sync for Buffer<T> {}

/// 帧缓冲区
///
/// 这是我们的"画布"，所有绘图操作都在这里进行。
/// 存储在 AXISRAM（STM32H750 的 D1 域 SRAM），因为：
/// - DMA 控制器可以直接访问
/// - 容量足够大（512KB）
#[link_section = ".axisram.buffers"]
static FRAME_BUFFER: Buffer<[u16; MAX_FRAME_BUFFER_SIZE]> =
    Buffer(UnsafeCell::new(MaybeUninit::uninit()));

/// SPI 分块传输缓冲区
///
/// 将窄区域的多行像素连续打包，跳过行末不属于窗口的像素。
/// 整行区域直接从帧缓冲发送，不使用此缓冲。
#[link_section = ".axisram.buffers"]
static DMA_BUF: Buffer<[u16; DMA_BUF_WORDS]> = Buffer(UnsafeCell::new(MaybeUninit::uninit()));

/// AXISRAM is NOLOAD: initialize through raw pointers before forming references.
/// Buffers are taken once and then owned exclusively by the display instance.
pub(super) fn take_buffers() -> (
    &'static mut [u16; MAX_FRAME_BUFFER_SIZE],
    &'static mut [u16; DMA_BUF_WORDS],
) {
    use core::sync::atomic::{AtomicBool, Ordering};
    static TAKEN: AtomicBool = AtomicBool::new(false);
    assert!(
        !TAKEN.swap(true, Ordering::AcqRel),
        "Display buffers already taken"
    );
    // SAFETY: the guard above grants exclusive ownership, and both NOLOAD
    // allocations are initialized before forming references.
    unsafe {
        let frame = FRAME_BUFFER.0.get().cast::<u16>();
        let transfer = DMA_BUF.0.get().cast::<u16>();
        frame.write_bytes(0, MAX_FRAME_BUFFER_SIZE);
        transfer.write_bytes(0, DMA_BUF_WORDS);
        (
            &mut *frame.cast::<[u16; MAX_FRAME_BUFFER_SIZE]>(),
            &mut *transfer.cast::<[u16; DMA_BUF_WORDS]>(),
        )
    }
}
