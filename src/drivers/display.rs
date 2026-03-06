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
//! 2. 绘制完成后，一次性把画布传输到屏幕（DMA 传输，CPU 不参与）
//!
//! 这样可以实现约 10fps 的全屏刷新，足够流畅显示温湿度变化。
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

use embedded_hal::blocking::spi::Transfer as SpiTransfer;
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

/// DMA 传输缓冲区大小
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
/// 存储在 AXISRAM（STM32H750 的 D2 域 SRAM），因为：
/// - DMA 控制器可以直接访问
/// - 容量足够大（512KB）
/// - 不会被其他 DMA 请求干扰
#[link_section = ".axisram.buffers"]
static mut FRAME_BUFFER: MaybeUninit<[u16; MAX_FRAME_BUFFER_SIZE]> = MaybeUninit::uninit();

/// DMA 传输缓冲区
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

/// SPI DMA 类型别名
pub type SpiDma = Spi<SPI2, Enabled, u8>;

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
pub struct DisplayDriver {
    /// SPI 接口（用于发送命令和数据）
    spi: Option<SpiDma>,
    /// 片选引脚（低电平选中屏幕）
    cs: PB12<Output<PushPull>>,
    /// 数据/命令选择引脚
    /// - 低电平：发送的是命令
    /// - 高电平：发送的是数据
    dc: PB1<Output<PushPull>>,
    /// 当前显示方向
    orientation: DisplayOrientation,
}

impl DisplayDriver {
    /// 创建显示驱动
    ///
    /// 创建后需要调用 `init()` 初始化屏幕。
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

    /// 创建带指定方向的显示驱动
    ///
    /// 如果知道要使用横屏或竖屏，可以用这个方法直接指定。
    #[allow(dead_code)]
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
    pub fn init(&mut self, delay_ms: &mut impl FnMut(u32)) {
        // 初始化引脚状态
        self.cs.set_high(); // 取消选中
        self.dc.set_high(); // 默认数据模式

        // 软件复位
        // 让屏幕重新初始化内部状态
        self.write_command(commands::SWRESET);
        delay_ms(5);

        // 退出睡眠模式
        // 屏幕上电后默认处于睡眠状态，需要唤醒
        self.write_command(commands::SLPOUT);
        delay_ms(5);

        // 设置颜色模式为 RGB565（16位色）
        // 0x55 = 16位/像素，这是最常用的模式
        self.write_cmd_data(commands::COLMOD, 0x55);

        // 设置显示方向
        self.write_cmd_data(commands::MADCTL, self.orientation.madctl_value());

        // 帧率控制
        // 设置刷新率为默认值
        self.write_cmd_data(commands::FRMCTR1, 0x00);
        self.write_data(0x1B);

        // 电源控制
        // 这些参数影响显示质量和功耗
        self.write_command(commands::PWCTR1);
        self.write_data(0x23); // VRH[5:0]
        self.write_data(0x10); // VC[2:0]

        self.write_cmd_data(commands::PWCTR2, 0x10); // BT[2:0]

        // VCOM 控制
        // 调整显示对比度
        self.write_cmd_data(commands::VMCTR1, 0x3E);
        self.write_data(0x28);

        // 关闭颜色反转
        self.write_command(commands::INVOFF);

        // 伽马校正
        // 这些参数让颜色显示更准确
        // 正极性伽马
        self.write_command(commands::GMCTRP1);
        self.write_data_bytes(&[
            0x0F, 0x31, 0x2B, 0x0C, 0x0E, 0x08, 0x4E, 0xF1, 0x37, 0x07, 0x10, 0x03, 0x0E, 0x09,
            0x00,
        ]);

        // 负极性伽马
        self.write_command(commands::GMCTRN1);
        self.write_data_bytes(&[
            0x00, 0x0E, 0x14, 0x03, 0x11, 0x07, 0x31, 0xC1, 0x48, 0x08, 0x0F, 0x0C, 0x31, 0x36,
            0x0F,
        ]);

        // 开启显示
        self.write_command(commands::DISPON);
        delay_ms(1);
    }

    /// 切换显示方向
    ///
    /// 在运行时切换横屏/竖屏模式。
    /// 切换后需要重新绘制界面，因为帧缓冲的内容不会自动旋转。
    pub fn set_orientation(&mut self, orientation: DisplayOrientation) {
        if self.orientation == orientation {
            return; // 方向没变，无需操作
        }

        self.orientation = orientation;

        // 更新屏幕的内存访问方向
        self.write_cmd_data(commands::MADCTL, self.orientation.madctl_value());
    }

    /// 发送命令
    ///
    /// 命令用于配置屏幕参数，不包含实际的像素数据。
    /// 通过拉低 DC 引脚告诉屏幕"接下来是命令"。
    fn write_command(&mut self, cmd: u8) {
        self.cs.set_low(); // 选中屏幕
        self.dc.set_low(); // DC=低 = 命令模式
        let mut buf = [cmd];
        let _ = self.spi.as_mut().unwrap().transfer(&mut buf);
        self.cs.set_high(); // 取消选中
    }

    /// 发送单个数据字节
    ///
    /// 数据是命令的参数，比如设置地址范围、颜色值等。
    /// 通过拉高 DC 引脚告诉屏幕"接下来是数据"。
    fn write_data(&mut self, data: u8) {
        self.cs.set_low(); // 选中屏幕
        self.dc.set_high(); // DC=高 = 数据模式
        let mut buf = [data];
        let _ = self.spi.as_mut().unwrap().transfer(&mut buf);
        self.cs.set_high(); // 取消选中
    }

    /// 发送命令和数据（原子操作）
    ///
    /// 很多操作需要"命令+数据"的组合，比如设置寄存器值。
    /// 这个方法在一次 CS 选中期间完成，避免被其他操作打断。
    fn write_cmd_data(&mut self, cmd: u8, data: u8) {
        self.cs.set_low(); // 选中屏幕
        self.dc.set_low(); // 发送命令
        let mut buf = [cmd];
        let _ = self.spi.as_mut().unwrap().transfer(&mut buf);
        self.dc.set_high(); // 发送数据
        let mut buf = [data];
        let _ = self.spi.as_mut().unwrap().transfer(&mut buf);
        self.cs.set_high(); // 取消选中
    }

    /// 发送多个数据字节
    ///
    /// 用于发送较长的数据，比如伽马校正表。
    /// 分块传输避免栈溢出（每块最多 64 字节）。
    fn write_data_bytes(&mut self, data: &[u8]) {
        self.cs.set_low();
        self.dc.set_high();
        let mut buf: [u8; 64] = [0; 64];
        for chunk in data.chunks(64) {
            buf[..chunk.len()].copy_from_slice(chunk);
            let _ = self.spi.as_mut().unwrap().transfer(&mut buf[..chunk.len()]);
        }
        self.cs.set_high();
    }

    /// 设置显示窗口
    ///
    /// 告诉屏幕"接下来的像素数据写到这个矩形区域"。
    /// 这是刷新显示的第一步：
    /// 1. 设置窗口（这个方法）
    /// 2. 发送 RAMWR 命令
    /// 3. 发送像素数据
    ///
    /// # 参数
    ///
    /// - `x0`, `y0`: 窗口左上角坐标
    /// - `x1`, `y1`: 窗口右下角坐标
    pub fn set_address_window(&mut self, x0: u16, y0: u16, x1: u16, y1: u16) {
        self.cs.set_low();

        // 设置列地址（X 方向）
        self.dc.set_low();
        let mut buf = [commands::CASET];
        let _ = self.spi.as_mut().unwrap().transfer(&mut buf);
        self.dc.set_high();
        let mut x_buf = [
            (x0 >> 8) as u8,
            (x0 & 0xFF) as u8,
            (x1 >> 8) as u8,
            (x1 & 0xFF) as u8,
        ];
        let _ = self.spi.as_mut().unwrap().transfer(&mut x_buf);

        // 设置行地址（Y 方向）
        self.dc.set_low();
        let mut buf = [commands::PASET];
        let _ = self.spi.as_mut().unwrap().transfer(&mut buf);
        self.dc.set_high();
        let mut y_buf = [
            (y0 >> 8) as u8,
            (y0 & 0xFF) as u8,
            (y1 >> 8) as u8,
            (y1 & 0xFF) as u8,
        ];
        let _ = self.spi.as_mut().unwrap().transfer(&mut y_buf);

        self.cs.set_high();
    }

    /// 准备 DMA 传输
    ///
    /// 设置显示窗口并发送"写显存"命令，然后保持 CS 低电平，
    /// 等待后续的像素数据传输。
    pub fn prepare_dma_transfer(&mut self, x0: u16, y0: u16, x1: u16, y1: u16) {
        self.set_address_window(x0, y0, x1, y1);

        self.cs.set_low();
        self.dc.set_low();
        let mut buf = [commands::RAMWR];
        let _ = self.spi.as_mut().unwrap().transfer(&mut buf);
        self.dc.set_high();
    }

    /// 结束 DMA 传输
    ///
    /// 拉高 CS，告诉屏幕"这次传输结束了"。
    pub fn end_dma_transfer(&mut self) {
        self.cs.set_high();
    }

    /// 取出 SPI 所有权
    ///
    /// DMA 传输需要独占 SPI，这个方法把 SPI 从驱动中取出来。
    /// 传输完成后需要用 `put_spi()` 放回去。
    pub fn take_spi(&mut self) -> Option<Spi<SPI2, Enabled, u8>> {
        self.spi.take()
    }

    /// 归还 SPI 所有权
    pub fn put_spi(&mut self, spi: Spi<SPI2, Enabled, u8>) {
        self.spi = Some(spi);
    }

    /// 全屏刷新
    ///
    /// 将帧缓冲的内容传输到屏幕显示。
    /// 这是"画布→屏幕"的过程。
    ///
    /// # 性能
    ///
    /// - 320×240 像素 ≈ 10fps
    /// - 传输期间 CPU 等待，不参与数据搬运
    pub fn flush(&mut self) {
        let width = self.width();
        let height = self.height();
        let framebuffer = unsafe { (*core::ptr::addr_of_mut!(FRAME_BUFFER)).assume_init_mut() };

        // 准备传输整个屏幕
        self.prepare_dma_transfer(0, 0, (width - 1) as u16, (height - 1) as u16);

        let mut spi = self.take_spi().unwrap();
        let dma_buf = unsafe { (*core::ptr::addr_of_mut!(DMA_BUF)).assume_init_mut() };

        let total_pixels = width * height;
        let pixels_per_transfer = DMA_BUF_SIZE / 2; // 每次传输 4096 像素
        let mut pixel_offset = 0;

        // 分块传输
        while pixel_offset < total_pixels {
            let remaining = total_pixels - pixel_offset;
            let count = remaining.min(pixels_per_transfer);

            // 将像素数据复制到 DMA 缓冲区
            // 每个像素 16 位，需要拆成 2 个字节传输
            for i in 0..count {
                let pixel = framebuffer[pixel_offset + i];
                dma_buf[i * 2] = (pixel >> 8) as u8; // 高字节
                dma_buf[i * 2 + 1] = (pixel & 0xFF) as u8; // 低字节
            }

            // 等待 SPI 发送缓冲区空闲
            while spi.inner().sr.read().txp().bit_is_clear() {}
            let _ = spi.transfer(&mut dma_buf[..count * 2]);

            pixel_offset += count;
        }

        // 等待所有数据发送完成
        while spi.inner().sr.read().txc().bit_is_clear() {}

        self.put_spi(spi);
        self.end_dma_transfer();
    }

    /// 区域刷新（脏矩形优化）
    ///
    /// 只刷新指定的矩形区域，而不是整个屏幕。
    /// 当只有一小块区域需要更新时（比如数字变化），用这个方法更快。
    ///
    /// # 参数
    ///
    /// - `x`, `y`: 矩形左上角
    /// - `w`, `h`: 矩形宽高
    pub fn flush_rect(&mut self, x: u16, y: u16, w: u16, h: u16) {
        let width = self.width();
        let height = self.height();

        // 边界检查
        if x >= width as u16 || y >= height as u16 {
            return;
        }

        // 计算实际刷新区域（裁剪到屏幕边界）
        let x1 = (x + w.saturating_sub(1)).min((width - 1) as u16);
        let y1 = (y + h.saturating_sub(1)).min((height - 1) as u16);
        let rect_width = (x1 - x + 1) as usize;
        let rect_height = (y1 - y + 1) as usize;

        let framebuffer = unsafe { (*core::ptr::addr_of_mut!(FRAME_BUFFER)).assume_init_mut() };
        let dma_buf = unsafe { (*core::ptr::addr_of_mut!(DMA_BUF)).assume_init_mut() };

        self.prepare_dma_transfer(x, y, x1, y1);
        let mut spi = self.take_spi().unwrap();

        let pixels_per_transfer = DMA_BUF_SIZE / 2;

        // 逐行传输
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
///
/// 在使用显示驱动之前必须调用一次。
/// 将帧缓冲和 DMA 缓冲清零（显示为黑色）。
pub fn init_frame_buffer() {
    unsafe {
        let fb = (*core::ptr::addr_of_mut!(FRAME_BUFFER)).assume_init_mut();
        fb.fill(0);

        let dma_buf = (*core::ptr::addr_of_mut!(DMA_BUF)).assume_init_mut();
        dma_buf.fill(0);
    }
}

// ============================================================================
// embedded_graphics 集成
// ============================================================================
//
// 实现 DrawTarget trait，让这个驱动可以和 embedded_graphics 库配合使用。
// embedded_graphics 提供了丰富的绘图 API：线条、矩形、文字等。
// 我们只需要实现"如何把像素写到帧缓冲"，其他都由库处理。

impl DrawTarget for DisplayDriver {
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
        let framebuffer = unsafe { (*core::ptr::addr_of_mut!(FRAME_BUFFER)).assume_init_mut() };

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

        let framebuffer = unsafe { (*core::ptr::addr_of_mut!(FRAME_BUFFER)).assume_init_mut() };

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
        let framebuffer = unsafe { (*core::ptr::addr_of_mut!(FRAME_BUFFER)).assume_init_mut() };
        let color_value = color.into_storage();
        let total_pixels = self.width() * self.height();
        framebuffer[..total_pixels].fill(color_value);
        Ok(())
    }
}

/// 提供屏幕尺寸信息
///
/// embedded_graphics 需要知道屏幕大小来进行边界检查等操作。
impl OriginDimensions for DisplayDriver {
    fn size(&self) -> Size {
        Size::new(self.width() as u32, self.height() as u32)
    }
}
