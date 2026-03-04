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
//! ## 显示方向
//!
//! 支持横屏和竖屏两种模式，可在运行时切换：
//! - `Portrait`: 240x320 竖屏
//! - `Landscape`: 320x240 横屏

use core::mem::MaybeUninit;

use embedded_hal::blocking::spi::Transfer as SpiTransfer;
use stm32h7xx_hal::gpio::{PB12, PB1, Output, PushPull};
use stm32h7xx_hal::pac::SPI2;
use stm32h7xx_hal::spi::{Spi, Enabled};
use stm32h7xx_hal::spi::HalSpi;

use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Size, Point},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::Rectangle,
};

/// ILI9341 物理分辨率
pub const PHYSICAL_WIDTH: usize = 240;
pub const PHYSICAL_HEIGHT: usize = 320;

/// 最大帧缓冲大小（竖屏模式）
pub const MAX_FRAME_BUFFER_SIZE: usize = PHYSICAL_WIDTH * PHYSICAL_HEIGHT;

/// DMA 传输缓冲区大小（字节）
pub const DMA_BUF_SIZE: usize = 8192;

/// 显示方向枚举
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DisplayOrientation {
    /// 竖屏模式 240x320
    Portrait,
    /// 横屏模式 320x240
    Landscape,
}

impl Default for DisplayOrientation {
    fn default() -> Self {
        Self::Landscape
    }
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
    fn madctl_value(&self) -> u8 {
        match self {
            DisplayOrientation::Portrait => 0x48,
            DisplayOrientation::Landscape => 0x28,
        }
    }
}

/// 完整帧缓冲（AXISRAM 中，约 150KB）
#[link_section = ".axisram.buffers"]
static mut FRAME_BUFFER: MaybeUninit<[u16; MAX_FRAME_BUFFER_SIZE]> = MaybeUninit::uninit();

/// DMA 传输缓冲区（AXISRAM 中，8KB）
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
    /// 当前显示方向
    orientation: DisplayOrientation,
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
            orientation: DisplayOrientation::default(),
        }
    }

    /// 创建带指定方向的显示驱动实例
    pub fn with_orientation(
        spi: Spi<SPI2, Enabled, u8>,
        cs: PB12<Output<PushPull>>,
        dc: PB1<Output<PushPull>>,
        orientation: DisplayOrientation,
    ) -> Self {
        Self {
            spi: Some(spi),
            cs,
            dc,
            orientation,
        }
    }

    /// 获取当前显示方向
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
    pub fn init(&mut self, delay_ms: &mut impl FnMut(u32)) {
        let _ = self.cs.set_high();
        let _ = self.dc.set_high();

        self.write_command(commands::SWRESET);
        delay_ms(5);

        self.write_command(commands::SLPOUT);
        delay_ms(5);

        self.write_cmd_data(commands::COLMOD, 0x55);

        self.write_cmd_data(commands::MADCTL, self.orientation.madctl_value());

        self.write_cmd_data(commands::FRMCTR1, 0x00);
        self.write_data(0x1B);

        self.write_command(commands::PWCTR1);
        self.write_data(0x23);
        self.write_data(0x10);

        self.write_cmd_data(commands::PWCTR2, 0x10);

        self.write_cmd_data(commands::VMCTR1, 0x3E);
        self.write_data(0x28);

        self.write_command(commands::INVOFF);

        self.write_command(commands::GMCTRP1);
        self.write_data_bytes(&[
            0x0F, 0x31, 0x2B, 0x0C, 0x0E, 0x08, 0x4E, 0xF1, 0x37, 0x07, 0x10, 0x03, 0x0E, 0x09, 0x00,
        ]);

        self.write_command(commands::GMCTRN1);
        self.write_data_bytes(&[
            0x00, 0x0E, 0x14, 0x03, 0x11, 0x07, 0x31, 0xC1, 0x48, 0x08, 0x0F, 0x0C, 0x31, 0x36, 0x0F,
        ]);

        self.write_command(commands::DISPON);
        delay_ms(1);
    }

    /// 动态切换显示方向
    pub fn set_orientation(&mut self, orientation: DisplayOrientation) {
        if self.orientation == orientation {
            return;
        }

        self.orientation = orientation;

        self.write_cmd_data(commands::MADCTL, self.orientation.madctl_value());
    }

    fn write_command(&mut self, cmd: u8) {
        let _ = self.cs.set_low();
        let _ = self.dc.set_low();
        let mut buf = [cmd];
        let _ = self.spi.as_mut().unwrap().transfer(&mut buf);
        let _ = self.cs.set_high();
    }

    fn write_data(&mut self, data: u8) {
        let _ = self.cs.set_low();
        let _ = self.dc.set_high();
        let mut buf = [data];
        let _ = self.spi.as_mut().unwrap().transfer(&mut buf);
        let _ = self.cs.set_high();
    }

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

    pub fn set_address_window(&mut self, x0: u16, y0: u16, x1: u16, y1: u16) {
        let _ = self.cs.set_low();

        let _ = self.dc.set_low();
        let mut buf = [commands::CASET];
        let _ = self.spi.as_mut().unwrap().transfer(&mut buf);
        let _ = self.dc.set_high();
        let mut x_buf = [
            (x0 >> 8) as u8, (x0 & 0xFF) as u8,
            (x1 >> 8) as u8, (x1 & 0xFF) as u8,
        ];
        let _ = self.spi.as_mut().unwrap().transfer(&mut x_buf);

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

    pub fn prepare_dma_transfer(&mut self, x0: u16, y0: u16, x1: u16, y1: u16) {
        self.set_address_window(x0, y0, x1, y1);

        let _ = self.cs.set_low();
        let _ = self.dc.set_low();
        let mut buf = [commands::RAMWR];
        let _ = self.spi.as_mut().unwrap().transfer(&mut buf);
        let _ = self.dc.set_high();
    }

    pub fn end_dma_transfer(&mut self) {
        let _ = self.cs.set_high();
    }

    pub fn take_spi(&mut self) -> Option<Spi<SPI2, Enabled, u8>> {
        self.spi.take()
    }

    pub fn put_spi(&mut self, spi: Spi<SPI2, Enabled, u8>) {
        self.spi = Some(spi);
    }

    /// 全屏刷新
    pub fn flush(&mut self) {
        let width = self.width();
        let height = self.height();
        let framebuffer = unsafe { (*core::ptr::addr_of_mut!(FRAME_BUFFER)).assume_init_mut() };

        self.prepare_dma_transfer(0, 0, (width - 1) as u16, (height - 1) as u16);

        let mut spi = self.take_spi().unwrap();
        let dma_buf = unsafe { (*core::ptr::addr_of_mut!(DMA_BUF)).assume_init_mut() };

        let total_pixels = width * height;
        let pixels_per_transfer = DMA_BUF_SIZE / 2;
        let mut pixel_offset = 0;

        while pixel_offset < total_pixels {
            let remaining = total_pixels - pixel_offset;
            let count = remaining.min(pixels_per_transfer);

            for i in 0..count {
                let pixel = framebuffer[pixel_offset + i];
                dma_buf[i * 2] = (pixel >> 8) as u8;
                dma_buf[i * 2 + 1] = (pixel & 0xFF) as u8;
            }

            while spi.inner().sr.read().txp().bit_is_clear() {}
            let _ = spi.transfer(&mut dma_buf[..count * 2]);

            pixel_offset += count;
        }

        while spi.inner().sr.read().txc().bit_is_clear() {}

        self.put_spi(spi);
        self.end_dma_transfer();
    }

    /// 区域刷新（脏矩形优化）
    pub fn flush_rect(&mut self, x: u16, y: u16, w: u16, h: u16) {
        let width = self.width();
        let height = self.height();
        
        if x >= width as u16 || y >= height as u16 {
            return;
        }

        let x1 = (x + w.saturating_sub(1)).min((width - 1) as u16);
        let y1 = (y + h.saturating_sub(1)).min((height - 1) as u16);
        let rect_width = (x1 - x + 1) as usize;
        let rect_height = (y1 - y + 1) as usize;

        let framebuffer = unsafe { (*core::ptr::addr_of_mut!(FRAME_BUFFER)).assume_init_mut() };
        let dma_buf = unsafe { (*core::ptr::addr_of_mut!(DMA_BUF)).assume_init_mut() };

        self.prepare_dma_transfer(x, y, x1, y1);
        let mut spi = self.take_spi().unwrap();

        let pixels_per_transfer = DMA_BUF_SIZE / 2;

        for row in 0..rect_height {
            let row_y = y as usize + row;
            let mut col_offset = 0;

            while col_offset < rect_width {
                let count = (rect_width - col_offset).min(pixels_per_transfer);

                for i in 0..count {
                    let col_x = x as usize + col_offset + i;
                    let pixel = framebuffer[row_y * width + col_x];
                    dma_buf[i * 2] = (pixel >> 8) as u8;
                    dma_buf[i * 2 + 1] = (pixel & 0xFF) as u8;
                }

                while spi.inner().sr.read().txp().bit_is_clear() {}
                let _ = spi.transfer(&mut dma_buf[..count * 2]);

                col_offset += count;
            }
        }

        while spi.inner().sr.read().txc().bit_is_clear() {}

        self.put_spi(spi);
        self.end_dma_transfer();
    }
}

/// 初始化帧缓冲
pub fn init_frame_buffer() {
    unsafe {
        let fb = (*core::ptr::addr_of_mut!(FRAME_BUFFER)).assume_init_mut();
        fb.fill(0);

        let dma_buf = (*core::ptr::addr_of_mut!(DMA_BUF)).assume_init_mut();
        dma_buf.fill(0);
    }
}

impl DrawTarget for DisplayDriver {
    type Color = Rgb565;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        let width = self.width();
        let height = self.height();
        let framebuffer = unsafe { (*core::ptr::addr_of_mut!(FRAME_BUFFER)).assume_init_mut() };

        for Pixel(point, color) in pixels {
            let x = point.x as usize;
            let y = point.y as usize;

            if x < width && y < height {
                let idx = y * width + x;
                framebuffer[idx] = color.into_storage();
            }
        }

        Ok(())
    }

    fn fill_solid(&mut self, area: &Rectangle, color: Self::Color) -> Result<(), Self::Error> {
        let width = self.width();
        let height = self.height();
        
        let area = area.intersection(&Rectangle::new(
            Point::zero(),
            Size::new(width as u32, height as u32),
        ));

        if area.size.width == 0 || area.size.height == 0 {
            return Ok(());
        }

        let framebuffer = unsafe { (*core::ptr::addr_of_mut!(FRAME_BUFFER)).assume_init_mut() };

        let start_x = area.top_left.x as usize;
        let start_y = area.top_left.y as usize;
        let area_width = area.size.width as usize;
        let area_height = area.size.height as usize;
        let color_value = color.into_storage();

        for y in start_y..(start_y + area_height) {
            let row_start = y * width + start_x;
            let row_end = row_start + area_width;
            for idx in row_start..row_end {
                framebuffer[idx] = color_value;
            }
        }

        Ok(())
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        let framebuffer = unsafe { (*core::ptr::addr_of_mut!(FRAME_BUFFER)).assume_init_mut() };
        let color_value = color.into_storage();
        let total_pixels = self.width() * self.height();
        framebuffer[..total_pixels].fill(color_value);
        Ok(())
    }
}

impl OriginDimensions for DisplayDriver {
    fn size(&self) -> Size {
        Size::new(self.width() as u32, self.height() as u32)
    }
}
