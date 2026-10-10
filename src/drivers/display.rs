//! # ILI9341 TFT LCD 屏幕驱动
//!
//! 这个模块驱动一块 2.8 英寸 TFT 彩色触摸屏（ILI9341 控制器）。
//! 屏幕分辨率为 240x320 像素，支持 65K 色（RGB565），适合显示：
//! - 温湿度数据
//! - 系统状态
//! - 简单的用户界面
//!
//! ## 为什么需要帧缓冲？
//!
//! 直接向屏幕写入像素太慢了（每个像素都要发送 SPI 命令）。
//! 我们在内存中维护一个"帧缓冲"——相当于一张"画布"：
//! 1. 所有绘图操作都在画布上进行（很快）
//! 2. 绘制完成后，一次性把画布传输到屏幕（有超时的阻塞式 SPI 传输）
//!
//! 传输耗时取决于 SPI 时钟和 CPU 开销；采样更新仅刷新变化区域。
//!
//! ## 内存占用
//!
//! - 帧缓冲：240 × 320 × 2 字节 = 153.6KB（存储在 AXISRAM）
//! - DMA 缓冲：8KB（用于分块传输，避免一次性占用太多内存）
//!
//! ## 硬件连接
//!
//! | STM32H750 | ILI9341 | 说明 |
//! |-----------|---------|------|
//! | PB15 | MOSI/SDI | SPI 数据输出 → 屏幕接收 |
//! | PB13 | SCK/SCL | SPI 时钟 |
//! | PB12 | CS | 片选（选中屏幕）|
//! | PB14 | MISO/SDO | SPI 数据输入 ← 屏幕发送（可不用）|
//! | PB1 | DC/RS | 数据/命令选择 |
//! | PB0 | BLK | 背光控制（高电平点亮）|
//!
//! ## 显示方向
//!
//! 支持两种显示方向，可以在运行时切换：
//! - **竖屏**：240×320，适合显示列表、文字
//! - **横屏**：320×240，适合并排显示多个数据卡片

use core::mem::MaybeUninit;

use crate::clock::Clock;
use embedded_hal::spi::FullDuplex;
use stm32h7xx_hal::gpio::{Output, PushPull, PB1, PB12};
use stm32h7xx_hal::pac::SPI2;
use stm32h7xx_hal::spi::HalSpi;
use stm32h7xx_hal::spi::{Enabled, Spi};

use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Point, Size},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::Rectangle,
};

/// 屏幕物理分辨率
///
/// ILI9341 的实际像素矩阵是 240 列 × 320 行
/// 这是硬件固定的，无论横屏还是竖屏显示
pub const PHYSICAL_WIDTH: usize = 240;
pub const PHYSICAL_HEIGHT: usize = 320;

/// 最大帧缓冲大小
///
/// 按竖屏模式计算：240 × 320 = 76,800 像素
/// 每个像素 2 字节（RGB565），共 153,600 字节 ≈ 150KB
pub const MAX_FRAME_BUFFER_SIZE: usize = PHYSICAL_WIDTH * PHYSICAL_HEIGHT;

/// SPI 分块传输缓冲区大小
///
/// 为什么是 8KB？
/// - 太小：传输次数多，效率低
/// - 太大：占用内存多，可能影响其他功能
/// - 8KB 可以一次传输 4096 个像素，是较好的平衡点
pub const DMA_BUF_SIZE: usize = 8192;

/// 显示方向
///
/// 屏幕可以横着放或竖着放，这个枚举让用户选择。
/// 实际上是通过修改 ILI9341 的内存访问方向来实现的，
/// 不需要物理旋转屏幕。
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum DisplayOrientation {
    /// 竖屏模式：240 宽 × 320 高
    /// 适合显示列表、长文本
    #[allow(dead_code)]
    Portrait,
    /// 横屏模式：320 宽 × 240 高
    /// 适合并排显示多个数据卡片
    #[default]
    Landscape,
}

impl DisplayOrientation {
    /// 获取当前方向的宽度
    pub fn width(&self) -> usize {
        match self {
            DisplayOrientation::Portrait => PHYSICAL_WIDTH,
            DisplayOrientation::Landscape => PHYSICAL_HEIGHT,
        }
    }

    /// 获取当前方向的高度
    pub fn height(&self) -> usize {
        match self {
            DisplayOrientation::Portrait => PHYSICAL_HEIGHT,
            DisplayOrientation::Landscape => PHYSICAL_WIDTH,
        }
    }

    /// 获取 MADCTL 寄存器值
    ///
    /// MADCTL（Memory Access Control）控制屏幕如何解释发送的像素数据：
    /// - 0x48：竖屏模式（BGR 颜色顺序）
    /// - 0x28：横屏模式（MV 位交换 XY，BGR 颜色顺序）
    fn madctl_value(&self) -> u8 {
        match self {
            DisplayOrientation::Portrait => 0x48,
            DisplayOrientation::Landscape => 0x28,
        }
    }
}

/// 帧缓冲区
///
/// 这是我们的"画布"，所有绘图操作都在这里进行。
/// 存储在 AXISRAM（STM32H750 的 D1 域 SRAM），因为：
/// - DMA 控制器可以直接访问
/// - 容量足够大（512KB）
#[link_section = ".axisram.buffers"]
static mut FRAME_BUFFER: MaybeUninit<[u16; MAX_FRAME_BUFFER_SIZE]> = MaybeUninit::uninit();

/// SPI 分块传输缓冲区
///
/// 用于分块传输帧缓冲数据到屏幕。
/// 不能直接传输整个帧缓冲（太大），需要分成小块：
/// 1. 从帧缓冲复制 8KB 数据到这里
/// 2. 通过 SPI 发送这 8KB
/// 3. 重复直到全部发送完毕
#[link_section = ".axisram.buffers"]
static mut DMA_BUF: MaybeUninit<[u8; DMA_BUF_SIZE]> = MaybeUninit::uninit();

/// ILI9341 寄存器命令
///
/// 这些是 ILI9341 控制器定义的命令码，用于控制屏幕的各种功能。
/// 每个命令对应一个寄存器，通过 SPI 发送后可以配置屏幕。
#[allow(unused)]
pub mod commands {
    /// 空操作
    pub const NOP: u8 = 0x00;
    /// 软件复位
    pub const SWRESET: u8 = 0x01;
    /// 退出睡眠模式
    pub const SLPOUT: u8 = 0x11;
    /// 开启显示
    pub const DISPON: u8 = 0x29;
    /// 列地址设置
    pub const CASET: u8 = 0x2A;
    /// 行地址设置
    pub const PASET: u8 = 0x2B;
    /// 写显存
    pub const RAMWR: u8 = 0x2C;
    /// 内存访问控制（方向、颜色顺序）
    pub const MADCTL: u8 = 0x36;
    /// 颜色模式设置
    pub const COLMOD: u8 = 0x3A;
    /// 帧率控制
    pub const FRMCTR1: u8 = 0xB1;
    /// 电源控制 1
    pub const PWCTR1: u8 = 0xC0;
    /// 电源控制 2
    pub const PWCTR2: u8 = 0xC1;
    /// VCOM 控制
    pub const VMCTR1: u8 = 0xC5;
    /// 关闭反转
    pub const INVOFF: u8 = 0x20;
    /// 正极性伽马校正
    pub const GMCTRP1: u8 = 0xE0;
    /// 负极性伽马校正
    pub const GMCTRN1: u8 = 0xE1;
}

/// SPI 类型别名
pub type DisplaySpi = Spi<SPI2, Enabled, u8>;

pub type DisplayError = stm32h750_drivers::transport::TransferError<stm32h7xx_hal::spi::Error>;

fn write_spi(spi: &mut DisplaySpi, clock: &Clock, bytes: &[u8]) -> Result<(), DisplayError> {
    let start = clock.now_us();
    stm32h750_drivers::transport::write_spi(spi, || clock.now_us(), bytes, 100_000)?;
    // Do not release CS or switch DC until the last bit has left the wire.
    while spi.inner().sr.read().txc().bit_is_clear() {
        if clock.expired(start, 100_000) {
            return Err(DisplayError::Timeout);
        }
    }
    Ok(())
}

/// 显示驱动
///
/// 这个结构体管理屏幕的所有操作：
/// - 初始化屏幕
/// - 切换显示方向
/// - 刷新显示内容
///
/// # 使用流程
///
/// 1. 初始化 SPI 和控制引脚
/// 2. 创建驱动实例
/// 3. 调用 `init()` 初始化屏幕
/// 4. 使用 embedded_graphics API 绘图
/// 5. 调用 `flush()` 将内容显示到屏幕
pub struct DisplayDriver<'a> {
    /// SPI 接口（用于发送命令和数据）
    spi: DisplaySpi,
    clock: &'a Clock,
    framebuffer: &'static mut [u16; MAX_FRAME_BUFFER_SIZE],
    transfer_buffer: &'static mut [u8; DMA_BUF_SIZE],
    /// 片选引脚（低电平选中屏幕）
    cs: PB12<Output<PushPull>>,
    /// 数据/命令选择引脚
    /// - 低电平：发送的是命令
    /// - 高电平：发送的是数据
    dc: PB1<Output<PushPull>>,
    /// 当前显示方向
    orientation: DisplayOrientation,
}

impl<'a> DisplayDriver<'a> {
    /// 创建显示驱动
    ///
    /// 创建后需要调用 `init()` 初始化屏幕。
    pub fn new(
        spi: Spi<SPI2, Enabled, u8>,
        cs: PB12<Output<PushPull>>,
        dc: PB1<Output<PushPull>>,
        clock: &'a Clock,
    ) -> Self {
        let (framebuffer, transfer_buffer) = take_buffers();
        Self {
            spi,
            clock,
            framebuffer,
            transfer_buffer,
            cs,
            dc,
            orientation: DisplayOrientation::default(),
        }
    }

    /// 创建带指定方向的显示驱动
    ///
    /// 如果知道要使用横屏或竖屏，可以用这个方法直接指定。
    #[allow(dead_code)]
    pub fn with_orientation(
        spi: DisplaySpi,
        cs: PB12<Output<PushPull>>,
        dc: PB1<Output<PushPull>>,
        clock: &'a Clock,
        orientation: DisplayOrientation,
    ) -> Self {
        let mut display = Self::new(spi, cs, dc, clock);
        display.orientation = orientation;
        display
    }

    /// 获取当前显示方向
    #[allow(dead_code)]
    pub fn orientation(&self) -> DisplayOrientation {
        self.orientation
    }

    /// 获取当前宽度
    pub fn width(&self) -> usize {
        self.orientation.width()
    }

    /// 获取当前高度
    pub fn height(&self) -> usize {
        self.orientation.height()
    }

    /// 初始化屏幕
    ///
    /// 这个方法发送一系列初始化命令，配置屏幕的：
    /// - 显示方向
    /// - 颜色模式（RGB565）
    /// - 电源参数
    /// - 伽马校正曲线
    ///
    /// 初始化完成后屏幕会点亮，显示白色。
    ///
    /// # 参数
    ///
    /// - `delay_ms`: 延时函数，用于等待屏幕复位完成
    pub fn init(&mut self, delay_ms: &mut impl FnMut(u32)) -> Result<(), DisplayError> {
        // 初始化引脚状态
        self.cs.set_high(); // 取消选中
        self.dc.set_high(); // 默认数据模式

        // 软件复位
        // 让屏幕重新初始化内部状态
        self.write_command(commands::SWRESET)?;
        delay_ms(5);

        // 退出睡眠模式
        // 屏幕上电后默认处于睡眠状态，需要唤醒
        self.write_command(commands::SLPOUT)?;
        delay_ms(5);

        // 设置颜色模式为 RGB565（16位色）
        // 0x55 = 16位/像素，这是最常用的模式
        self.write_cmd_data(commands::COLMOD, 0x55)?;

        // 设置显示方向
        self.write_cmd_data(commands::MADCTL, self.orientation.madctl_value())?;

        // 帧率控制
        // 设置刷新率为默认值
        self.write_cmd_data(commands::FRMCTR1, 0x00)?;
        self.write_data(0x1B)?;

        // 电源控制
        // 这些参数影响显示质量和功耗
        self.write_command(commands::PWCTR1)?;
        self.write_data(0x23)?; // VRH[5:0]
        self.write_data(0x10)?; // VC[2:0]

        self.write_cmd_data(commands::PWCTR2, 0x10)?; // BT[2:0]

        // VCOM 控制
        // 调整显示对比度
        self.write_cmd_data(commands::VMCTR1, 0x3E)?;
        self.write_data(0x28)?;

        // 关闭颜色反转
        self.write_command(commands::INVOFF)?;

        // 伽马校正
        // 这些参数让颜色显示更准确
        // 正极性伽马
        self.write_command(commands::GMCTRP1)?;
        self.write_data_bytes(&[
            0x0F, 0x31, 0x2B, 0x0C, 0x0E, 0x08, 0x4E, 0xF1, 0x37, 0x07, 0x10, 0x03, 0x0E, 0x09,
            0x00,
        ])?;

        // 负极性伽马
        self.write_command(commands::GMCTRN1)?;
        self.write_data_bytes(&[
            0x00, 0x0E, 0x14, 0x03, 0x11, 0x07, 0x31, 0xC1, 0x48, 0x08, 0x0F, 0x0C, 0x31, 0x36,
            0x0F,
        ])?;

        // 开启显示
        self.write_command(commands::DISPON)?;
        delay_ms(1);
        Ok(())
    }

    /// 切换显示方向
    ///
    /// 在运行时切换横屏/竖屏模式。
    /// 切换后需要重新绘制界面，因为帧缓冲的内容不会自动旋转。
    #[allow(dead_code)]
    pub fn set_orientation(&mut self, orientation: DisplayOrientation) -> Result<(), DisplayError> {
        if self.orientation == orientation {
            return Ok(()); // 方向没变，无需操作
        }

        self.write_cmd_data(commands::MADCTL, orientation.madctl_value())?;
        self.orientation = orientation;
        Ok(())
    }

    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), DisplayError> {
        write_spi(&mut self.spi, self.clock, bytes)
    }

    /// All transaction exits release CS, including errors.
    fn finish(&mut self, result: Result<(), DisplayError>) -> Result<(), DisplayError> {
        self.cs.set_high();
        if result.is_err() {
            // Discard partial FIFO state. The caller retries a complete frame.
            self.spi.inner().cr1.modify(|_, w| w.spe().clear_bit());
            for _ in 0..16 {
                if self.spi.inner().sr.read().rxp().bit_is_clear() {
                    break;
                }
                let _ = self.spi.read();
            }
            self.spi.inner().ifcr.write(|w| {
                w.ovrc()
                    .set_bit()
                    .modfc()
                    .set_bit()
                    .eotc()
                    .set_bit()
                    .txtfc()
                    .set_bit()
                    .udrc()
                    .set_bit()
                    .crcec()
                    .set_bit()
            });
            self.spi.inner().cr1.modify(|_, w| w.spe().set_bit());
        }
        result
    }

    fn write_command(&mut self, cmd: u8) -> Result<(), DisplayError> {
        self.cs.set_low();
        self.dc.set_low();
        let result = self.write_bytes(&[cmd]);
        self.finish(result)
    }

    fn write_data(&mut self, data: u8) -> Result<(), DisplayError> {
        self.write_data_bytes(&[data])
    }

    fn write_cmd_data(&mut self, cmd: u8, data: u8) -> Result<(), DisplayError> {
        self.cs.set_low();
        self.dc.set_low();
        let result = (|| {
            self.write_bytes(&[cmd])?;
            self.dc.set_high();
            self.write_bytes(&[data])
        })();
        self.finish(result)
    }

    fn write_data_bytes(&mut self, bytes: &[u8]) -> Result<(), DisplayError> {
        self.cs.set_low();
        self.dc.set_high();
        let result = self.write_bytes(bytes);
        self.finish(result)
    }

    fn set_address_window(
        &mut self,
        x0: u16,
        y0: u16,
        x1: u16,
        y1: u16,
    ) -> Result<(), DisplayError> {
        self.write_command(commands::CASET)?;
        self.write_data_bytes(&[(x0 >> 8) as u8, x0 as u8, (x1 >> 8) as u8, x1 as u8])?;
        self.write_command(commands::PASET)?;
        self.write_data_bytes(&[(y0 >> 8) as u8, y0 as u8, (y1 >> 8) as u8, y1 as u8])
    }

    pub fn flush(&mut self) -> Result<(), DisplayError> {
        self.flush_rect(0, 0, self.width() as u16, self.height() as u16)
    }

    /// Pack multiple rows into each transfer, even for narrow dirty rectangles.
    pub fn flush_rect(&mut self, x: u16, y: u16, w: u16, h: u16) -> Result<(), DisplayError> {
        let Some((x, y, w, h)) =
            stm32h750_drivers::clip_region(x, y, w, h, self.width() as u16, self.height() as u16)
        else {
            return Ok(());
        };
        let result = (|| {
            self.set_address_window(x, y, x + w - 1, y + h - 1)?;
            self.cs.set_low();
            self.dc.set_low();
            self.write_bytes(&[commands::RAMWR])?;
            self.dc.set_high();
            let stride = self.width();
            let rect_width = w as usize;
            let total = rect_width * h as usize;
            let mut offset = 0;
            while offset < total {
                let count = stm32h750_drivers::framebuffer::pack_rect_chunk(
                    self.framebuffer,
                    stride,
                    (x, y, w, h),
                    offset,
                    self.transfer_buffer,
                );
                write_spi(
                    &mut self.spi,
                    self.clock,
                    &self.transfer_buffer[..count * 2],
                )?;
                offset += count;
            }
            Ok(())
        })();
        self.finish(result)
    }
}

/// AXISRAM is NOLOAD: initialize through raw pointers before forming references.
/// Buffers are taken once and then owned exclusively by the display instance.
fn take_buffers() -> (
    &'static mut [u16; MAX_FRAME_BUFFER_SIZE],
    &'static mut [u8; DMA_BUF_SIZE],
) {
    use core::sync::atomic::{AtomicBool, Ordering};
    static TAKEN: AtomicBool = AtomicBool::new(false);
    assert!(
        !TAKEN.swap(true, Ordering::AcqRel),
        "Display buffers already taken"
    );
    unsafe {
        let frame = core::ptr::addr_of_mut!(FRAME_BUFFER).cast::<u16>();
        let transfer = core::ptr::addr_of_mut!(DMA_BUF).cast::<u8>();
        frame.write_bytes(0, MAX_FRAME_BUFFER_SIZE);
        transfer.write_bytes(0, DMA_BUF_SIZE);
        (
            &mut *frame.cast::<[u16; MAX_FRAME_BUFFER_SIZE]>(),
            &mut *transfer.cast::<[u8; DMA_BUF_SIZE]>(),
        )
    }
}

// ============================================================================
// embedded_graphics 集成
// ============================================================================
//
// 实现 DrawTarget trait，让这个驱动可以和 embedded_graphics 库配合使用。
// embedded_graphics 提供了丰富的绘图 API：线条、矩形、文字等。
// 我们只需要实现"如何把像素写到帧缓冲"，其他都由库处理。

impl DrawTarget for DisplayDriver<'_> {
    type Color = Rgb565;
    type Error = core::convert::Infallible;

    /// 绘制像素
    ///
    /// embedded_graphics 会调用这个方法来绘制每个像素。
    /// 我们只需要把颜色值写入帧缓冲对应的位置。
    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        let width = self.width();
        let height = self.height();
        let framebuffer = &mut self.framebuffer;

        for Pixel(point, color) in pixels {
            let x = point.x as usize;
            let y = point.y as usize;

            // 检查坐标是否在屏幕范围内
            if x < width && y < height {
                let idx = y * width + x;
                framebuffer[idx] = color.into_storage();
            }
        }

        Ok(())
    }

    /// 填充矩形
    ///
    /// 这是一个优化：当需要填充大面积矩形时，
    /// 直接写入帧缓冲比逐像素调用 draw_iter 快得多。
    fn fill_solid(&mut self, area: &Rectangle, color: Self::Color) -> Result<(), Self::Error> {
        let width = self.width();
        let height = self.height();

        // 裁剪到屏幕范围
        let area = area.intersection(&Rectangle::new(
            Point::zero(),
            Size::new(width as u32, height as u32),
        ));

        if area.size.width == 0 || area.size.height == 0 {
            return Ok(());
        }

        let framebuffer = &mut self.framebuffer;

        let start_x = area.top_left.x as usize;
        let start_y = area.top_left.y as usize;
        let area_width = area.size.width as usize;
        let area_height = area.size.height as usize;
        let color_value = color.into_storage();

        // 逐行填充
        for y in start_y..(start_y + area_height) {
            let row_start = y * width + start_x;
            let row_end = row_start + area_width;
            for pixel in framebuffer.iter_mut().take(row_end).skip(row_start) {
                *pixel = color_value;
            }
        }

        Ok(())
    }

    /// 清屏
    ///
    /// 用指定颜色填充整个帧缓冲。
    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        let total_pixels = self.width() * self.height();
        let framebuffer = &mut self.framebuffer;
        let color_value = color.into_storage();
        framebuffer[..total_pixels].fill(color_value);
        Ok(())
    }
}

/// 提供屏幕尺寸信息
///
/// embedded_graphics 需要知道屏幕大小来进行边界检查等操作。
impl OriginDimensions for DisplayDriver<'_> {
    fn size(&self) -> Size {
        Size::new(self.width() as u32, self.height() as u32)
    }
}

impl crate::ui::FlushTarget for DisplayDriver<'_> {
    type FlushError = DisplayError;
    fn flush(&mut self) -> Result<(), Self::FlushError> {
        DisplayDriver::flush(self)
    }
    fn flush_rect(&mut self, x: u16, y: u16, w: u16, h: u16) -> Result<(), Self::FlushError> {
        DisplayDriver::flush_rect(self, x, y, w, h)
    }
}
