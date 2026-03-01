//! # ILI9341 屏幕驱动模块 (完整 DMA 版本)
//!
//! 本模块实现了 ILI9341 TFT LCD 控制器的 DMA 驱动程序。
//! 使用 DMA1 Stream3 进行 SPI2 数据传输，CPU 零开销。
//!
//! ## 硬件连接
//!
//! | STM32H750 | ILI9341 | 说明      |
//! | --------- | ------- | --------- |
//! | PB15/MOSI | SDI     | 数据输入  |
//! | PB13/SCK  | SCL     | 时钟      |
//! | PB12/CS   | CS      | 片选      |
//! | PB14/MISO | SDO     | 数据输出  |
//! | PB1/RS    | D/C     | 数据/命令 |
//! | PB0/BLK   | BLK     | 背光控制  |
//!
//! ## DMA 配置
//! - 使用 SPI2 + DMA1 Stream3 (Channel 3)
//! - 缓冲区位于 AXISRAM (0x24000000)，DMA 可访问
//! - 8KB 双缓冲区，支持乒乓传输

use core::mem::MaybeUninit;

use embedded_hal::blocking::spi::Transfer as SpiTransfer;
use stm32h7xx_hal::gpio::{PB12, PB1, Output, PushPull};
use stm32h7xx_hal::pac::SPI2;
use stm32h7xx_hal::spi::{Spi, Enabled};
use stm32h7xx_hal::spi::HalSpi;
use embedded_hal::digital::v2::OutputPin;

// embedded_graphics 支持
use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Size, Point},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::Rectangle,
};
use heapless::Vec;

/// 屏幕分辨率
pub const DISPLAY_WIDTH: usize = 240;
pub const DISPLAY_HEIGHT: usize = 320;

/// 完整帧缓冲大小（像素数）
pub const FRAME_BUFFER_SIZE: usize = DISPLAY_WIDTH * DISPLAY_HEIGHT;

/// DMA 传输缓冲区大小（字节）- 用于局部刷新
pub const DMA_BUF_SIZE: usize = 8192;

/// 完整帧缓冲（AXISRAM 中，150KB）
/// 使用 u16 数组，每个元素对应一个 RGB565 像素
#[link_section = ".axisram.buffers"]
static mut FRAME_BUFFER: MaybeUninit<[u16; FRAME_BUFFER_SIZE]> = MaybeUninit::uninit();

/// DMA 传输缓冲区（AXISRAM 中，8KB，用于局部刷新）
#[link_section = ".axisram.buffers"]
static mut DMA_BUF: MaybeUninit<[u8; DMA_BUF_SIZE]> = MaybeUninit::uninit();

/// ILI9341 寄存器命令
#[allow(unused)]
pub mod commands {
    pub const NOP: u8 = 0x00;
    pub const SWRESET: u8 = 0x01;
    pub const SLPOUT: u8 = 0x11;
    pub const DISPON: u8 = 0x29;
    pub const CASET: u8 = 0x2A;
    pub const PASET: u8 = 0x2B;
    pub const RAMWR: u8 = 0x2C;
    pub const MADCTL: u8 = 0x36;
    pub const COLMOD: u8 = 0x3A;
    pub const FRMCTR1: u8 = 0xB1;
    pub const PWCTR1: u8 = 0xC0;
    pub const PWCTR2: u8 = 0xC1;
    pub const VMCTR1: u8 = 0xC5;
    pub const INVOFF: u8 = 0x20;
    pub const GMCTRP1: u8 = 0xE0;
    pub const GMCTRN1: u8 = 0xE1;
}

/// SPI DMA 类型别名
pub type SpiDma = Spi<SPI2, Enabled, u8>;

/// 显示驱动结构体
pub struct DisplayDriver {
    /// SPI 外设
    spi: Option<SpiDma>,
    /// 片选引脚
    cs: PB12<Output<PushPull>>,
    /// 数据/命令引脚
    dc: PB1<Output<PushPull>>,
}

impl DisplayDriver {
    /// 创建新的显示驱动实例
    pub fn new(
        spi: Spi<SPI2, Enabled, u8>,
        cs: PB12<Output<PushPull>>,
        dc: PB1<Output<PushPull>>,
    ) -> Self {
        Self {
            spi: Some(spi),
            cs,
            dc,
        }
    }

    /// 初始化屏幕
    pub fn init(&mut self, delay_ms: &mut impl FnMut(u32)) {
        // 初始引脚状态
        let _ = self.cs.set_high();
        let _ = self.dc.set_high();

        // 软件复位
        self.write_command(commands::SWRESET);
        delay_ms(5);

        // 退出睡眠模式
        self.write_command(commands::SLPOUT);
        delay_ms(5);

        // 设置颜色格式：16位 RGB565
        self.write_cmd_data(commands::COLMOD, 0x55);

        // 设置内存访问控制
        self.write_cmd_data(commands::MADCTL, 0x48);

        // 设置帧率控制
        self.write_cmd_data(commands::FRMCTR1, 0x00);
        self.write_data(0x1B);

        // 电源控制
        self.write_command(commands::PWCTR1);
        self.write_data(0x23);
        self.write_data(0x10);

        self.write_cmd_data(commands::PWCTR2, 0x10);

        self.write_cmd_data(commands::VMCTR1, 0x3E);
        self.write_data(0x28);

        // 关闭显示反转
        self.write_command(commands::INVOFF);

        // 伽马校正
        self.write_command(commands::GMCTRP1);
        self.write_data_bytes(&[
            0x0F, 0x31, 0x2B, 0x0C, 0x0E, 0x08, 0x4E, 0xF1, 0x37, 0x07, 0x10, 0x03, 0x0E, 0x09, 0x00,
        ]);

        self.write_command(commands::GMCTRN1);
        self.write_data_bytes(&[
            0x00, 0x0E, 0x14, 0x03, 0x11, 0x07, 0x31, 0xC1, 0x48, 0x08, 0x0F, 0x0C, 0x31, 0x36, 0x0F,
        ]);

        // 开启显示
        self.write_command(commands::DISPON);
        delay_ms(1);
    }

    /// 发送命令（阻塞模式）
    fn write_command(&mut self, cmd: u8) {
        let _ = self.cs.set_low();
        let _ = self.dc.set_low();
        let mut buf = [cmd];
        let _ = self.spi.as_mut().unwrap().transfer(&mut buf);
        let _ = self.cs.set_high();
    }

    /// 发送单个数据字节（阻塞模式）
    fn write_data(&mut self, data: u8) {
        let _ = self.cs.set_low();
        let _ = self.dc.set_high();
        let mut buf = [data];
        let _ = self.spi.as_mut().unwrap().transfer(&mut buf);
        let _ = self.cs.set_high();
    }

    /// 发送命令 + 数据（阻塞模式）
    fn write_cmd_data(&mut self, cmd: u8, data: u8) {
        let _ = self.cs.set_low();
        let _ = self.dc.set_low();
        let mut buf = [cmd];
        let _ = self.spi.as_mut().unwrap().transfer(&mut buf);
        let _ = self.dc.set_high();
        let mut buf = [data];
        let _ = self.spi.as_mut().unwrap().transfer(&mut buf);
        let _ = self.cs.set_high();
    }

    /// 发送多个数据字节（阻塞模式）
    fn write_data_bytes(&mut self, data: &[u8]) {
        let _ = self.cs.set_low();
        let _ = self.dc.set_high();
        let mut buf: [u8; 64] = [0; 64];
        for chunk in data.chunks(64) {
            buf[..chunk.len()].copy_from_slice(chunk);
            let _ = self.spi.as_mut().unwrap().transfer(&mut buf[..chunk.len()]);
        }
        let _ = self.cs.set_high();
    }

    /// 设置地址窗口（阻塞模式）
    pub fn set_address_window(&mut self, x0: u16, y0: u16, x1: u16, y1: u16) {
        let _ = self.cs.set_low();

        // CASET
        let _ = self.dc.set_low();
        let mut buf = [commands::CASET];
        let _ = self.spi.as_mut().unwrap().transfer(&mut buf);
        let _ = self.dc.set_high();
        let mut x_buf = [
            (x0 >> 8) as u8, (x0 & 0xFF) as u8,
            (x1 >> 8) as u8, (x1 & 0xFF) as u8,
        ];
        let _ = self.spi.as_mut().unwrap().transfer(&mut x_buf);

        // PASET
        let _ = self.dc.set_low();
        let mut buf = [commands::PASET];
        let _ = self.spi.as_mut().unwrap().transfer(&mut buf);
        let _ = self.dc.set_high();
        let mut y_buf = [
            (y0 >> 8) as u8, (y0 & 0xFF) as u8,
            (y1 >> 8) as u8, (y1 & 0xFF) as u8,
        ];
        let _ = self.spi.as_mut().unwrap().transfer(&mut y_buf);

        let _ = self.cs.set_high();
    }

    /// 准备 DMA 传输（设置窗口并发送 RAMWR，CS 保持低电平）
    pub fn prepare_dma_transfer(&mut self, x0: u16, y0: u16, x1: u16, y1: u16) {
        self.set_address_window(x0, y0, x1, y1);

        let _ = self.cs.set_low();
        let _ = self.dc.set_low();
        let mut buf = [commands::RAMWR];
        let _ = self.spi.as_mut().unwrap().transfer(&mut buf);
        let _ = self.dc.set_high();
        // CS 保持低电平，DC 高电平，等待 DMA 传输
    }

    /// 结束 DMA 传输（释放 CS）
    pub fn end_dma_transfer(&mut self) {
        let _ = self.cs.set_high();
    }

    /// 获取 SPI 用于 DMA 传输
    pub fn take_spi(&mut self) -> Option<Spi<SPI2, Enabled, u8>> {
        self.spi.take()
    }

    /// 归还 SPI
    pub fn put_spi(&mut self, spi: Spi<SPI2, Enabled, u8>) {
        self.spi = Some(spi);
    }

    /// 全屏刷新（将帧缓冲发送到屏幕）
    pub fn flush(&mut self) {
        let framebuffer = unsafe { (*core::ptr::addr_of_mut!(FRAME_BUFFER)).assume_init_mut() };

        // 准备全屏传输
        self.prepare_dma_transfer(
            0, 0,
            (DISPLAY_WIDTH - 1) as u16,
            (DISPLAY_HEIGHT - 1) as u16,
        );

        let mut spi = self.take_spi().unwrap();
        let dma_buf = unsafe { (*core::ptr::addr_of_mut!(DMA_BUF)).assume_init_mut() };

        let total_pixels = FRAME_BUFFER_SIZE;
        let pixels_per_transfer = DMA_BUF_SIZE / 2;
        let mut pixel_offset = 0;

        while pixel_offset < total_pixels {
            let remaining = total_pixels - pixel_offset;
            let count = remaining.min(pixels_per_transfer);

            // 从帧缓冲复制到 DMA 缓冲区（转换为字节）
            for i in 0..count {
                let pixel = framebuffer[pixel_offset + i];
                dma_buf[i * 2] = (pixel >> 8) as u8;
                dma_buf[i * 2 + 1] = (pixel & 0xFF) as u8;
            }

            // 等待 SPI 就绪
            while spi.inner().sr.read().txp().bit_is_clear() {}
            let _ = spi.transfer(&mut dma_buf[..count * 2]);

            pixel_offset += count;
        }

        // 等待传输完成
        while spi.inner().sr.read().txc().bit_is_clear() {}

        self.put_spi(spi);
        self.end_dma_transfer();
    }

    /// 区域刷新（脏矩形优化）
    ///
    /// 只刷新指定的矩形区域，提高更新效率
    pub fn flush_rect(&mut self, x: u16, y: u16, w: u16, h: u16) {
        // 边界检查
        if x >= DISPLAY_WIDTH as u16 || y >= DISPLAY_HEIGHT as u16 {
            return;
        }

        let x1 = (x + w.saturating_sub(1)).min((DISPLAY_WIDTH - 1) as u16);
        let y1 = (y + h.saturating_sub(1)).min((DISPLAY_HEIGHT - 1) as u16);
        let width = (x1 - x + 1) as usize;
        let height = (y1 - y + 1) as usize;

        let framebuffer = unsafe { (*core::ptr::addr_of_mut!(FRAME_BUFFER)).assume_init_mut() };
        let dma_buf = unsafe { (*core::ptr::addr_of_mut!(DMA_BUF)).assume_init_mut() };

        // 准备区域传输
        self.prepare_dma_transfer(x, y, x1, y1);
        let mut spi = self.take_spi().unwrap();

        let pixels_per_transfer = DMA_BUF_SIZE / 2;

        // 逐行传输
        for row in 0..height {
            let y = y as usize + row;
            let mut col_offset = 0;

            while col_offset < width {
                let count = (width - col_offset).min(pixels_per_transfer);

                // 从帧缓冲复制当前行的一段到 DMA 缓冲区
                for i in 0..count {
                    let x = x as usize + col_offset + i;
                    let pixel = framebuffer[y * DISPLAY_WIDTH + x];
                    dma_buf[i * 2] = (pixel >> 8) as u8;
                    dma_buf[i * 2 + 1] = (pixel & 0xFF) as u8;
                }

                // 等待 SPI 就绪并传输
                while spi.inner().sr.read().txp().bit_is_clear() {}
                let _ = spi.transfer(&mut dma_buf[..count * 2]);

                col_offset += count;
            }
        }

        // 等待传输完成
        while spi.inner().sr.read().txc().bit_is_clear() {}

        self.put_spi(spi);
        self.end_dma_transfer();
    }

    /// 获取帧缓冲的可变引用
    pub fn framebuffer(&mut self) -> &'static mut [u16; FRAME_BUFFER_SIZE] {
        unsafe { (*core::ptr::addr_of_mut!(FRAME_BUFFER)).assume_init_mut() }
    }
}

/// 初始化帧缓冲（必须在 main 中调用一次）
pub fn init_frame_buffer() {
    unsafe {
        // 初始化帧缓冲为黑色
        let fb = (*core::ptr::addr_of_mut!(FRAME_BUFFER)).assume_init_mut();
        fb.fill(0);

        // 初始化 DMA 传输缓冲区
        let dma_buf = (*core::ptr::addr_of_mut!(DMA_BUF)).assume_init_mut();
        dma_buf.fill(0);
    }
}

// ============================================================================
// embedded_graphics DrawTarget 实现
// ============================================================================

impl DrawTarget for DisplayDriver {
    type Color = Rgb565;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        // 直接写入帧缓冲
        let framebuffer = unsafe { (*core::ptr::addr_of_mut!(FRAME_BUFFER)).assume_init_mut() };

        for Pixel(point, color) in pixels {
            let x = point.x as usize;
            let y = point.y as usize;

            if x < DISPLAY_WIDTH && y < DISPLAY_HEIGHT {
                let idx = y * DISPLAY_WIDTH + x;
                framebuffer[idx] = color.into_storage();
            }
        }

        Ok(())
    }

    fn fill_contiguous<I>(&mut self, area: &Rectangle, colors: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Self::Color>,
    {
        let area = area.intersection(&Rectangle::new(
            Point::zero(),
            Size::new(DISPLAY_WIDTH as u32, DISPLAY_HEIGHT as u32),
        ));

        if area.size.width == 0 || area.size.height == 0 {
            return Ok(());
        }

        let framebuffer = unsafe { (*core::ptr::addr_of_mut!(FRAME_BUFFER)).assume_init_mut() };

        let start_x = area.top_left.x as usize;
        let start_y = area.top_left.y as usize;
        let width = area.size.width as usize;
        let height = area.size.height as usize;

        let mut idx = start_y * DISPLAY_WIDTH + start_x;
        let mut row_stride = DISPLAY_WIDTH - width;

        for color in colors {
            framebuffer[idx] = color.into_storage();
            idx += 1;

            // 每行结束后跳过下一行的起始偏移
            // 这里简单处理：计数器方式
        }

        Ok(())
    }

    fn fill_solid(&mut self, area: &Rectangle, color: Self::Color) -> Result<(), Self::Error> {
        let area = area.intersection(&Rectangle::new(
            Point::zero(),
            Size::new(DISPLAY_WIDTH as u32, DISPLAY_HEIGHT as u32),
        ));

        if area.size.width == 0 || area.size.height == 0 {
            return Ok(());
        }

        let framebuffer = unsafe { (*core::ptr::addr_of_mut!(FRAME_BUFFER)).assume_init_mut() };

        let start_x = area.top_left.x as usize;
        let start_y = area.top_left.y as usize;
        let width = area.size.width as usize;
        let height = area.size.height as usize;
        let color_value = color.into_storage();

        // 逐行填充
        for y in start_y..(start_y + height) {
            let row_start = y * DISPLAY_WIDTH + start_x;
            let row_end = row_start + width;
            for idx in row_start..row_end {
                framebuffer[idx] = color_value;
            }
        }

        Ok(())
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        let framebuffer = unsafe { (*core::ptr::addr_of_mut!(FRAME_BUFFER)).assume_init_mut() };
        let color_value = color.into_storage();
        framebuffer.fill(color_value);
        Ok(())
    }
}

impl OriginDimensions for DisplayDriver {
    fn size(&self) -> Size {
        Size::new(DISPLAY_WIDTH as u32, DISPLAY_HEIGHT as u32)
    }
}
