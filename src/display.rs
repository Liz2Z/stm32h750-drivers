//! ILI9341 屏幕驱动模块
//! 使用硬件 SPI2 实现高速传输

use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Size},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::Rectangle,
};
use embedded_hal::blocking::spi::Transfer;
use embedded_hal::digital::v2::OutputPin;
use nb::block;

/// 屏幕分辨率
pub const DISPLAY_WIDTH: usize = 240;
pub const DISPLAY_HEIGHT: usize = 320;

/// ILI9341 命令
#[allow(unused)]
mod commands {
    pub const NOP: u8 = 0x00;
    pub const SWRESET: u8 = 0x01;
    pub const RDDID: u8 = 0x04;
    pub const RDDST: u8 = 0x09;
    pub const SLPIN: u8 = 0x10;
    pub const SLPOUT: u8 = 0x11;
    pub const PTLON: u8 = 0x12;
    pub const NORON: u8 = 0x13;
    pub const INVOFF: u8 = 0x20;
    pub const INVON: u8 = 0x21;
    pub const DISPOFF: u8 = 0x28;
    pub const DISPON: u8 = 0x29;
    pub const CASET: u8 = 0x2A;
    pub const PASET: u8 = 0x2B;
    pub const RAMWR: u8 = 0x2C;
    pub const RAMRD: u8 = 0x2E;
    pub const PTLAR: u8 = 0x30;
    pub const MADCTL: u8 = 0x36;
    pub const COLMOD: u8 = 0x3A;
    pub const FRMCTR1: u8 = 0xB1;
    pub const FRMCTR2: u8 = 0xB2;
    pub const FRMCTR3: u8 = 0xB3;
    pub const INVCTR: u8 = 0xB4;
    pub const DISSET5: u8 = 0xB6;
    pub const PWCTR1: u8 = 0xC0;
    pub const PWCTR2: u8 = 0xC1;
    pub const PWCTR3: u8 = 0xC2;
    pub const PWCTR4: u8 = 0xC3;
    pub const PWCTR5: u8 = 0xC4;
    pub const VMCTR1: u8 = 0xC5;
    pub const VMOFCTR: u8 = 0xC7;
    pub const WRABC: u8 = 0xD5;
    pub const RDID1: u8 = 0xDA;
    pub const RDID2: u8 = 0xDB;
    pub const RDID3: u8 = 0xDC;
    pub const RDID4: u8 = 0xDD;
    pub const GMCTRP1: u8 = 0xE0;
    pub const GMCTRN1: u8 = 0xE1;
}

/// 硬件 SPI 显示驱动
pub struct DisplaySpi<SPI, CS, DC> {
    spi: SPI,
    cs: CS,
    dc: DC,
}

impl<SPI, CS, DC> DisplaySpi<SPI, CS, DC>
where
    SPI: Transfer<u8>,
    CS: OutputPin,
    DC: OutputPin,
{
    pub fn new(spi: SPI, cs: CS, dc: DC) -> Self {
        Self { spi, cs, dc }
    }

    /// 传输单个字节（带延时）
    fn transfer_byte(&mut self, data: u8) -> u8 {
        let mut buf = [data];
        self.spi.transfer(&mut buf).ok();
        // 添加短暂延时让信号稳定
        for _ in 0..10 {
            cortex_m::asm::nop();
        }
        buf[0]
    }

    /// 传输多个字节
    fn transfer_bytes(&mut self, data: &mut [u8]) {
        self.spi.transfer(data).ok();
    }

    /// 写命令
    fn write_command(&mut self, cmd: u8) {
        self.cs.set_low().ok();
        self.dc.set_low().ok();
        let _ = self.transfer_byte(cmd);
        self.cs.set_high().ok();
    }

    /// 写数据
    fn write_data(&mut self, data: u8) {
        self.cs.set_low().ok();
        self.dc.set_high().ok();
        let _ = self.transfer_byte(data);
        self.cs.set_high().ok();
    }

    /// 写多个数据字节（保持 CS 低）
    fn write_data_bytes(&mut self, data: &[u8]) {
        self.cs.set_low().ok();
        self.dc.set_high().ok();
        for &byte in data {
            let _ = self.transfer_byte(byte);
        }
        self.cs.set_high().ok();
    }

    /// 写命令后紧跟数据
    fn write_cmd_data(&mut self, cmd: u8, data: u8) {
        self.cs.set_low().ok();
        self.dc.set_low().ok();
        let _ = self.transfer_byte(cmd);
        self.dc.set_high().ok();
        let _ = self.transfer_byte(data);
        self.cs.set_high().ok();
    }

    /// 初始化屏幕（带串口调试）
    pub fn init(&mut self, tx: &mut impl embedded_hal::serial::Write<u8>) {
        // 写入调试信息
        let mut write_str = |s: &str| {
            for b in s.bytes() {
                let _ = block!(tx.write(b));
            }
        };

        write_str("[Display] Start init...\r\n");

        // 确保 CS 和 DC 为高电平
        let _ = self.cs.set_high();
        let _ = self.dc.set_high();

        write_str("[Display] Software reset...\r\n");
        self.write_command(commands::SWRESET);
        for _ in 0..500000 {
            cortex_m::asm::nop();
        }

        write_str("[Display] Exit sleep mode...\r\n");
        self.write_command(commands::SLPOUT);
        for _ in 0..500000 {
            cortex_m::asm::nop();
        }

        write_str("[Display] Set color format (16-bit RGB565)...\r\n");
        self.write_cmd_data(commands::COLMOD, 0x55);

        write_str("[Display] Set memory access control...\r\n");
        self.write_cmd_data(commands::MADCTL, 0x48);

        write_str("[Display] Set frame rate control...\r\n");
        self.write_cmd_data(commands::FRMCTR1, 0x00);
        self.write_data(0x1B);

        write_str("[Display] Set power control...\r\n");
        self.write_command(commands::PWCTR1);
        self.write_data(0x23);
        self.write_data(0x10);

        self.write_cmd_data(commands::PWCTR2, 0x10);

        self.write_cmd_data(commands::VMCTR1, 0x3E);
        self.write_data(0x28);

        write_str("[Display] Set inversion off...\r\n");
        self.write_command(commands::INVOFF);

        write_str("[Display] Set Gamma correction...\r\n");
        self.write_command(commands::GMCTRP1);
        self.write_data_bytes(&[
            0x0F, 0x31, 0x2B, 0x0C, 0x0E, 0x08, 0x4E, 0xF1, 0x37, 0x07, 0x10, 0x03, 0x0E, 0x09,
            0x00,
        ]);

        self.write_command(commands::GMCTRN1);
        self.write_data_bytes(&[
            0x00, 0x0E, 0x14, 0x03, 0x11, 0x07, 0x31, 0xC1, 0x48, 0x08, 0x0F, 0x0C, 0x31, 0x36,
            0x0F,
        ]);

        write_str("[Display] Turn on display...\r\n");
        self.write_command(commands::DISPON);
        for _ in 0..100000 {
            cortex_m::asm::nop();
        }

        write_str("[Display] Init complete!\r\n");
    }

    /// 设置绘图区域
    fn set_address_window(&mut self, x0: u16, y0: u16, x1: u16, y1: u16) {
        // 设置列地址 (CASET)
        let _ = self.cs.set_low();
        let _ = self.dc.set_low();
        let _ = self.transfer_byte(commands::CASET);
        let _ = self.dc.set_high();
        let _ = self.transfer_byte((x0 >> 8) as u8);
        let _ = self.transfer_byte((x0 & 0xFF) as u8);
        let _ = self.transfer_byte((x1 >> 8) as u8);
        let _ = self.transfer_byte((x1 & 0xFF) as u8);
        let _ = self.cs.set_high();

        // 设置页地址 (PASET)
        let _ = self.cs.set_low();
        let _ = self.dc.set_low();
        let _ = self.transfer_byte(commands::PASET);
        let _ = self.dc.set_high();
        let _ = self.transfer_byte((y0 >> 8) as u8);
        let _ = self.transfer_byte((y0 & 0xFF) as u8);
        let _ = self.transfer_byte((y1 >> 8) as u8);
        let _ = self.transfer_byte((y1 & 0xFF) as u8);
        let _ = self.cs.set_high();
    }

    /// 填充整个屏幕
    pub fn fill_screen(&mut self, color: Rgb565) {
        self.fill_rect(0, 0, DISPLAY_WIDTH as u16, DISPLAY_HEIGHT as u16, color);
    }

    /// 填充矩形
    pub fn fill_rect(&mut self, x: u16, y: u16, w: u16, h: u16, color: Rgb565) {
        let x1 = (x + w - 1).min((DISPLAY_WIDTH - 1) as u16);
        let y1 = (y + h - 1).min((DISPLAY_HEIGHT - 1) as u16);

        self.set_address_window(x, y, x1, y1);

        let pixel_color = color.into_storage();
        let high_byte = (pixel_color >> 8) as u8;
        let low_byte = (pixel_color & 0xFF) as u8;

        let num_pixels = (w as u32) * (h as u32);

        self.write_command(commands::RAMWR);

        let _ = self.cs.set_low();
        let _ = self.dc.set_high();

        for _ in 0..num_pixels {
            let _ = self.transfer_byte(high_byte);
            let _ = self.transfer_byte(low_byte);
        }

        let _ = self.cs.set_high();
    }
}

impl<SPI, CS, DC> DrawTarget for DisplaySpi<SPI, CS, DC>
where
    SPI: Transfer<u8>,
    CS: OutputPin,
    DC: OutputPin,
{
    type Color = Rgb565;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels.into_iter() {
            let x = point.x as u16;
            let y = point.y as u16;

            if x < DISPLAY_WIDTH as u16 && y < DISPLAY_HEIGHT as u16 {
                self.set_address_window(x, y, x, y);

                let pixel_color = color.into_storage();
                let high_byte = (pixel_color >> 8) as u8;
                let low_byte = (pixel_color & 0xFF) as u8;

                self.write_command(commands::RAMWR);

                let _ = self.cs.set_low();
                let _ = self.dc.set_high();
                let _ = self.transfer_byte(high_byte);
                let _ = self.transfer_byte(low_byte);
                let _ = self.cs.set_high();
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

        let x0 = area.top_left.x as u16;
        let y0 = area.top_left.y as u16;
        let x1 = x0 + area.size.width as u16 - 1;
        let y1 = y0 + area.size.height as u16 - 1;

        self.set_address_window(x0, y0, x1, y1);
        self.write_command(commands::RAMWR);

        let _ = self.cs.set_low();
        let _ = self.dc.set_high();

        for color in colors {
            let pixel_color = color.into_storage();
            let _ = self.transfer_byte((pixel_color >> 8) as u8);
            let _ = self.transfer_byte((pixel_color & 0xFF) as u8);
        }

        let _ = self.cs.set_high();

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

        self.fill_rect(
            area.top_left.x as u16,
            area.top_left.y as u16,
            area.size.width as u16,
            area.size.height as u16,
            color,
        );

        Ok(())
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.fill_screen(color);
        Ok(())
    }
}

impl<SPI, CS, DC> OriginDimensions for DisplaySpi<SPI, CS, DC>
where
    SPI: Transfer<u8>,
    CS: OutputPin,
    DC: OutputPin,
{
    fn size(&self) -> Size {
        Size::new(DISPLAY_WIDTH as u32, DISPLAY_HEIGHT as u32)
    }
}
