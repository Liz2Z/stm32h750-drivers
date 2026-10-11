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
//! 2. 绘制完成后，通过 SPI2 DMA 传输像素，等待发送完毕（有超时）
//!
//! 传输耗时取决于 SPI 时钟和 CPU 开销；采样更新仅刷新变化区域。
//!
//! ## 内存占用
//!
//! - 帧缓冲：240 × 320 × 2 字节 = 153.6KB（存储在 AXISRAM）
//! - DMA 缓冲：8KB（用于分块传输，避免一次性占用太多内存）
//!
//! STM32H750 的 SPI2/DMA 适配及引脚配置位于 `bsp::display` / `bsp::board`。
//!
//! ## 显示方向
//!
//! 支持两种显示方向，可以在运行时切换：
//! - **竖屏**：240×320，适合显示列表、文字
//! - **横屏**：320×240，适合并排显示多个数据卡片

use super::{
    pixel_pack::{clip_region, pack_rect_words},
    DisplayInterface,
};

pub const PHYSICAL_WIDTH: usize = 240;
pub const PHYSICAL_HEIGHT: usize = 320;
pub const MAX_FRAME_BUFFER_SIZE: usize = PHYSICAL_WIDTH * PHYSICAL_HEIGHT;
pub const DMA_BUF_WORDS: usize = 4096;

use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Point, Size},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::Rectangle,
};

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
pub struct DisplayDriver<T: DisplayInterface> {
    interface: T,
    framebuffer: &'static mut [u16; MAX_FRAME_BUFFER_SIZE],
    transfer_buffer: &'static mut [u16; DMA_BUF_WORDS],
    orientation: DisplayOrientation,
}

impl<T: DisplayInterface> DisplayDriver<T> {
    /// Own the transport and buffers; call init before displaying pixels.
    pub fn new(
        interface: T,
        framebuffer: &'static mut [u16; MAX_FRAME_BUFFER_SIZE],
        transfer_buffer: &'static mut [u16; DMA_BUF_WORDS],
        orientation: DisplayOrientation,
    ) -> Self {
        Self {
            interface,
            framebuffer,
            transfer_buffer,
            orientation,
        }
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
    pub fn init(&mut self, delay_ms: &mut impl FnMut(u32)) -> Result<(), T::Error> {
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
    pub fn set_orientation(&mut self, orientation: DisplayOrientation) -> Result<(), T::Error> {
        if self.orientation == orientation {
            return Ok(()); // 方向没变，无需操作
        }

        self.write_cmd_data(commands::MADCTL, orientation.madctl_value())?;
        self.orientation = orientation;
        Ok(())
    }

    fn write_command(&mut self, command: u8) -> Result<(), T::Error> {
        self.interface.write_command(command)
    }
    fn write_data(&mut self, data: u8) -> Result<(), T::Error> {
        self.interface.write_data(&[data])
    }
    fn write_data_bytes(&mut self, data: &[u8]) -> Result<(), T::Error> {
        self.interface.write_data(data)
    }
    fn write_cmd_data(&mut self, command: u8, data: u8) -> Result<(), T::Error> {
        self.interface.write_cmd_data(command, data)
    }

    fn set_address_window(&mut self, x0: u16, y0: u16, x1: u16, y1: u16) -> Result<(), T::Error> {
        self.write_command(commands::CASET)?;
        self.write_data_bytes(&[(x0 >> 8) as u8, x0 as u8, (x1 >> 8) as u8, x1 as u8])?;
        self.write_command(commands::PASET)?;
        self.write_data_bytes(&[(y0 >> 8) as u8, y0 as u8, (y1 >> 8) as u8, y1 as u8])
    }

    pub fn flush(&mut self) -> Result<(), T::Error> {
        self.flush_rect(0, 0, self.width() as u16, self.height() as u16)
    }

    /// Full-width windows are sent directly; narrow windows pack native words.
    pub fn flush_rect(&mut self, x: u16, y: u16, w: u16, h: u16) -> Result<(), T::Error> {
        let Some(region @ (x, y, w, h)) =
            clip_region(x, y, w, h, self.width() as u16, self.height() as u16)
        else {
            return Ok(());
        };
        let result = (|| {
            self.set_address_window(x, y, x + w - 1, y + h - 1)?;
            self.interface.begin_pixels(commands::RAMWR)?;
            let stride = self.width();
            let total = w as usize * h as usize;
            let transfer = (|| {
                if w as usize == stride {
                    let start = y as usize * stride;
                    for pixels in self.framebuffer[start..start + total].chunks(u16::MAX as usize) {
                        self.interface.write_pixels(pixels)?;
                    }
                } else {
                    let mut offset = 0;
                    while offset < total {
                        let count = pack_rect_words(
                            self.framebuffer,
                            stride,
                            region,
                            offset,
                            self.transfer_buffer,
                        );
                        self.interface
                            .write_pixels(&self.transfer_buffer[..count])?;
                        offset += count;
                    }
                }
                Ok(())
            })();
            self.interface.finish_pixels(transfer)
        })();
        result
    }
}

// ============================================================================
// embedded_graphics 集成
// ============================================================================
//
// 实现 DrawTarget trait，让这个驱动可以和 embedded_graphics 库配合使用。
// embedded_graphics 提供了丰富的绘图 API：线条、矩形、文字等。
// 我们只需要实现"如何把像素写到帧缓冲"，其他都由库处理。

impl<T: DisplayInterface> DrawTarget for DisplayDriver<T> {
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
            framebuffer[row_start..row_end].fill(color_value);
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
impl<T: DisplayInterface> OriginDimensions for DisplayDriver<T> {
    fn size(&self) -> Size {
        Size::new(self.width() as u32, self.height() as u32)
    }
}
