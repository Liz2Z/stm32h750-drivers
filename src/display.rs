//! ILI9341 屏幕驱动模块
//! 使用软件 SPI 与 RC522 隔离，避免冲突

use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Size},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::Rectangle,
};
use embedded_hal::digital::v2::{InputPin, OutputPin};

/// 包装类型来适配 v2 trait
pub struct PinWrapper<P>(P);

impl<P> PinWrapper<P> {
    pub fn new(pin: P) -> Self {
        Self(pin)
    }
}

impl<P: OutputPin> OutputPin for PinWrapper<P> {
    type Error = P::Error;

    fn set_low(&mut self) -> Result<(), Self::Error> {
        self.0.set_low()
    }

    fn set_high(&mut self) -> Result<(), Self::Error> {
        self.0.set_high()
    }
}

impl<P: InputPin> InputPin for PinWrapper<P> {
    type Error = P::Error;

    fn is_high(&self) -> Result<bool, Self::Error> {
        self.0.is_high()
    }

    fn is_low(&self) -> Result<bool, Self::Error> {
        self.0.is_low()
    }
}

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

/// 显示方向
#[derive(Clone, Copy)]
pub enum Orientation {
    Portrait = 0x00,
    Landscape = 0x60,
    PortraitSwapped = 0xC0,
    LandscapeSwapped = 0xA0,
}

/// 软件 SPI 结构体（用于屏幕）
pub struct DisplaySpi<SCK, MOSI, MISO, CS, DC> {
    sck: SCK,
    mosi: MOSI,
    miso: MISO,
    cs: CS,
    dc: DC,
}

impl<SCK, MOSI, MISO, CS, DC, E> DisplaySpi<SCK, MOSI, MISO, CS, DC>
where
    SCK: OutputPin<Error = E>,
    MOSI: OutputPin<Error = E>,
    MISO: InputPin<Error = E>,
    CS: OutputPin<Error = E>,
    DC: OutputPin<Error = E>,
    E: core::fmt::Debug,
{
    pub fn new(sck: SCK, mosi: MOSI, miso: MISO, cs: CS, dc: DC) -> Self {
        Self {
            sck,
            mosi,
            miso,
            cs,
            dc,
        }
    }

    /// 延时函数 - 最小化延时
    fn delay_ns(&self, _ns: u32) {
        cortex_m::asm::nop();
    }

    /// 传输单个字节 - 极速版本
    fn transfer_byte(&mut self, data: u8) -> u8 {
        let mut received = 0u8;

        // 展开循环以减少开销
        let bits = [
            (data >> 7) & 1,
            (data >> 6) & 1,
            (data >> 5) & 1,
            (data >> 4) & 1,
            (data >> 3) & 1,
            (data >> 2) & 1,
            (data >> 1) & 1,
            (data >> 0) & 1,
        ];

        for i in 0..8 {
            if bits[i] == 1 {
                let _ = self.mosi.set_high();
            } else {
                let _ = self.mosi.set_low();
            }

            let _ = self.sck.set_high();

            if self.miso.is_high().unwrap_or(false) {
                received |= 1 << (7 - i);
            }

            let _ = self.sck.set_low();
        }

        received
    }

    /// 写命令
    fn write_command(&mut self, cmd: u8) {
        // 拉高 CS，准备新的传输
        let _ = self.cs.set_high();
        self.delay_ns(1);

        let _ = self.cs.set_low();
        let _ = self.dc.set_low(); // DC = 0 表示命令
        self.delay_ns(1);

        let _ = self.transfer_byte(cmd);

        let _ = self.cs.set_high();
        self.delay_ns(1);
    }

    /// 写数据（跟随命令）
    fn write_data(&mut self, data: u8) {
        // 保持 CS 低电平，只切换 DC
        let _ = self.dc.set_high(); // DC = 1 表示数据
        self.delay_ns(1);

        let _ = self.transfer_byte(data);

        // 数据传输完成后拉高 CS
        let _ = self.cs.set_high();
        self.delay_ns(1);
    }

    /// 写多个数据字节（跟随命令）
    fn write_data_bytes(&mut self, data: &[u8]) {
        let _ = self.dc.set_high();
        self.delay_ns(1);

        for &byte in data {
            let _ = self.transfer_byte(byte);
        }

        let _ = self.cs.set_high();
        self.delay_ns(1);
    }

    /// 写命令后紧跟数据（保持 CS 低）
    fn write_cmd_data(&mut self, cmd: u8, data: u8) {
        // 发送命令
        let _ = self.cs.set_high();
        self.delay_ns(1);
        let _ = self.cs.set_low();
        let _ = self.dc.set_low();
        self.delay_ns(1);
        let _ = self.transfer_byte(cmd);

        // 切换为数据模式
        let _ = self.dc.set_high();
        self.delay_ns(1);
        let _ = self.transfer_byte(data);

        let _ = self.cs.set_high();
        self.delay_ns(1);
    }

    /// 软件复位
    fn soft_reset(&mut self) {
        self.write_command(commands::SWRESET);
        for _ in 0..500000 { cortex_m::asm::nop(); }
    }

    /// 初始化屏幕
    pub fn init(&mut self) {
        // 确保 CS 和 DC 为高电平
        let _ = self.cs.set_high();
        let _ = self.dc.set_high();

        // 软件复位
        self.write_command(commands::SWRESET);
        // 减少复位延时到 50ms
        for _ in 0..500000 { cortex_m::asm::nop(); }

        // 退出睡眠模式
        self.write_command(commands::SLPOUT);
        // 减少延时到 50ms
        for _ in 0..500000 { cortex_m::asm::nop(); }

        // 设置颜色格式 (16位 RGB565)
        self.write_cmd_data(commands::COLMOD, 0x55);

        // 设置扫描方向 - 使用 0x48 表示行/列交换，以便正确显示
        // bit 3: BGR=1 (BGR 顺序)
        // bit 6: MV=1 (行/列交换，横屏)
        self.write_cmd_data(commands::MADCTL, 0x48);

        // 设置帧率控制
        self.write_cmd_data(commands::FRMCTR1, 0x00);
        self.write_data(0x1B); // 继续写数据

        // 电源控制
        self.write_command(commands::PWCTR1);
        self.write_data(0x23);
        self.write_data(0x10);

        self.write_cmd_data(commands::PWCTR2, 0x10);

        self.write_cmd_data(commands::VMCTR1, 0x3E);
        self.write_data(0x28);

        // 设置显示正常模式（不反转）
        self.write_command(commands::INVOFF);

        // Gamma 校正
        self.write_command(commands::GMCTRP1);
        self.write_data_bytes(&[
            0x0F, 0x31, 0x2B, 0x0C, 0x0E, 0x08, 0x4E, 0xF1,
            0x37, 0x07, 0x10, 0x03, 0x0E, 0x09, 0x00,
        ]);

        self.write_command(commands::GMCTRN1);
        self.write_data_bytes(&[
            0x00, 0x0E, 0x14, 0x03, 0x11, 0x07, 0x31, 0xC1,
            0x48, 0x08, 0x0F, 0x0C, 0x31, 0x36, 0x0F,
        ]);

        // 打开显示
        self.write_command(commands::DISPON);
        // 短暂延时
        for _ in 0..100000 { cortex_m::asm::nop(); }
    }

    /// 设置绘图区域
    fn set_address_window(&mut self, x0: u16, y0: u16, x1: u16, y1: u16) {
        // 设置列地址 (CASET) - 保持 CS 低电平传输所有参数
        let _ = self.cs.set_high();
        self.delay_ns(1);
        let _ = self.cs.set_low();
        let _ = self.dc.set_low();
        self.delay_ns(1);
        let _ = self.transfer_byte(commands::CASET);

        // 切换到数据模式，发送 4 个参数
        let _ = self.dc.set_high();
        self.delay_ns(1);
        let _ = self.transfer_byte((x0 >> 8) as u8);
        let _ = self.transfer_byte((x0 & 0xFF) as u8);
        let _ = self.transfer_byte((x1 >> 8) as u8);
        let _ = self.transfer_byte((x1 & 0xFF) as u8);
        let _ = self.cs.set_high();

        // 设置页地址 (PASET) - 保持 CS 低电平传输所有参数
        let _ = self.cs.set_high();
        self.delay_ns(1);
        let _ = self.cs.set_low();
        let _ = self.dc.set_low();
        self.delay_ns(1);
        let _ = self.transfer_byte(commands::PASET);

        // 切换到数据模式，发送 4 个参数
        let _ = self.dc.set_high();
        self.delay_ns(1);
        let _ = self.transfer_byte((y0 >> 8) as u8);
        let _ = self.transfer_byte((y0 & 0xFF) as u8);
        let _ = self.transfer_byte((y1 >> 8) as u8);
        let _ = self.transfer_byte((y1 & 0xFF) as u8);
        let _ = self.cs.set_high();
        self.delay_ns(1);
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
        self.delay_ns(1);

        for _ in 0..num_pixels {
            let _ = self.transfer_byte(high_byte);
            let _ = self.transfer_byte(low_byte);
        }

        let _ = self.cs.set_high();
    }
}

impl<SCK, MOSI, MISO, CS, DC, E> DrawTarget for DisplaySpi<SCK, MOSI, MISO, CS, DC>
where
    SCK: OutputPin<Error = E>,
    MOSI: OutputPin<Error = E>,
    MISO: InputPin<Error = E>,
    CS: OutputPin<Error = E>,
    DC: OutputPin<Error = E>,
    E: core::fmt::Debug,
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
                self.delay_ns(10);

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
        self.delay_ns(10);

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

impl<SCK, MOSI, MISO, CS, DC, E> OriginDimensions for DisplaySpi<SCK, MOSI, MISO, CS, DC>
where
    SCK: OutputPin<Error = E>,
    MOSI: OutputPin<Error = E>,
    MISO: InputPin<Error = E>,
    CS: OutputPin<Error = E>,
    DC: OutputPin<Error = E>,
    E: core::fmt::Debug,
{
    fn size(&self) -> Size {
        Size::new(DISPLAY_WIDTH as u32, DISPLAY_HEIGHT as u32)
    }
}
