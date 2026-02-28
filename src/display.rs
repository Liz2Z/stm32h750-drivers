//! # ILI9341 屏幕驱动模块
//!
//! 本模块实现了 ILI9341 TFT LCD 控制器的驱动程序。
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
//! ## 特性
//! - 使用硬件 SPI2 实现高速传输
//! - 支持 embedded_graphics 绘图库
//! - 16 位 RGB565 颜色格式（65536 色）
//! - 240×320 像素分辨率
//!
//! ## 使用示例
//! ```ignore
//! // 创建显示实例
//! let mut display = DisplaySpi::new(spi, cs, dc);
//!
//! // 初始化屏幕
//! display.init(&mut tx);
//!
//! // 填充屏幕为红色
//! display.fill_screen(Rgb565::RED);
//!
//! // 绘制矩形
//! display.fill_rect(10, 10, 100, 50, Rgb565::BLUE);
//! ```

// ============================================================================
// 依赖引入
// ============================================================================

// embedded_graphics - Rust 嵌入式图形库
// 提供了统一的绘图接口，支持绘制形状、文本、图像等
use embedded_graphics::{
    draw_target::DrawTarget,            // 绘图目标 trait，定义如何绘制像素
    geometry::{OriginDimensions, Size}, // 几何类型：尺寸、原点尺寸
    pixelcolor::Rgb565,                 // 颜色格式：16 位 RGB565
    prelude::*,                         // 常用类型：Point, Pixel, PrimitiveStyle 等
    primitives::Rectangle,              // 图形：矩形
};

// embedded_hal - 嵌入式硬件抽象层
// 定义了与硬件通信的标准接口
use embedded_hal::blocking::spi::Transfer; // SPI 传输 trait
use embedded_hal::digital::v2::OutputPin; // 输出引脚 trait

// nb - 非阻塞 I/O 原语
// 提供了非阻塞操作的标准类型和宏
use nb::block; // block! 宏：将非阻塞操作转换为阻塞操作

/// 屏幕分辨率 - 水平方向像素数（列数）
pub const DISPLAY_WIDTH: usize = 240;

/// 屏幕分辨率 - 垂直方向像素数（行数）
pub const DISPLAY_HEIGHT: usize = 320;

/// ILI9341 寄存器命令定义模块
///
/// ILI9341 是一款 240x320 分辨率的 TFT LCD 控制器
/// 通过 SPI 接口接收命令来控制显示行为
#[allow(unused)]
mod commands {
    /// 无操作命令 (No Operation)
    /// 用于空操作或作为填充命令
    pub const NOP: u8 = 0x00;

    /// 软件复位 (Software Reset)
    /// 将所有寄存器恢复到默认值，执行后需要等待约 5ms
    pub const SWRESET: u8 = 0x01;

    /// 读取显示 ID (Read Display ID)
    /// 用于读取芯片厂商代码，返回 4 个字节：0x93, 0x41 (厂商 ID)
    pub const RDDID: u8 = 0x04;

    /// 读取显示状态 (Read Display Status)
    /// 读取屏幕当前状态信息（如是否在睡眠模式）
    pub const RDDST: u8 = 0x09;

    /// 进入睡眠模式 (Sleep In)
    /// 降低功耗，关闭显示但保持寄存器值
    pub const SLPIN: u8 = 0x10;

    /// 退出睡眠模式 (Sleep Out)
    /// 从睡眠模式唤醒，需要等待约 5ms 后才能发送其他命令
    pub const SLPOUT: u8 = 0x11;

    /// 部分模式开启 (Partial Mode On)
    /// 启用部分显示模式（只更新屏幕的部分区域）
    pub const PTLON: u8 = 0x12;

    /// 正常显示模式 (Normal Display Mode On)
    /// 退出部分模式，恢复全屏显示
    pub const NORON: u8 = 0x13;

    /// 关闭显示反转 (Display Inversion Off)
    /// 使用正常的颜色显示（不反转）
    pub const INVOFF: u8 = 0x20;

    /// 开启显示反转 (Display Inversion On)
    /// 反转所有颜色值，黑变白，白变黑
    pub const INVON: u8 = 0x21;

    /// 关闭显示 (Display Off)
    /// 关闭显示输出，进入空白状态（寄存器值保持）
    pub const DISPOFF: u8 = 0x28;

    /// 开启显示 (Display On)
    /// 开启显示输出，开始显示图像
    pub const DISPON: u8 = 0x29;

    /// 列地址设置 (Column Address Set)
    /// 设置后续像素数据写入的 X 轴范围（列起始和结束位置）
    /// 参数格式：[x_start_high, x_start_low, x_end_high, x_end_low]
    pub const CASET: u8 = 0x2A;

    /// 页地址设置 (Page Address Set)
    /// 设置后续像素数据写入的 Y 轴范围（行起始和结束位置）
    /// 参数格式：[y_start_high, y_start_low, y_end_high, y_end_low]
    pub const PASET: u8 = 0x2B;

    /// 内存写入 (Memory Write)
    /// 发送此命令后，所有后续数据字节被视为像素颜色值并写入显存
    /// 必须先通过 CASET 和 PASET 设置写入区域
    pub const RAMWR: u8 = 0x2C;

    /// 内存读取 (Memory Read)
    /// 从显存读取像素数据（需要额外的 dummy 字节）
    pub const RAMRD: u8 = 0x2E;

    /// 部分区域地址设置 (Partial Area)
    /// 设置部分显示模式的滚动区域
    pub const PTLAR: u8 = 0x30;

    /// 内存访问控制 (Memory Access Control)
    /// 控制显存访问顺序和屏幕方向
    /// 位定义：
    ///   - bit 7 (MY): 行地址顺序 (0=正常, 1=反转)
    ///   - bit 6 (MX): 列地址顺序 (0=正常, 1=反转)
    ///   - bit 5 (MV): 行/列交换 (0=正常, 1=交换)
    ///   - bit 4 (ML): 垂直刷新顺序 (0=从上到下, 1=从下到上)
    ///   - bit 3 (BGR): 颜色顺序 (0=RGB, 1=BGR)
    ///   - bit 2 (MH): 水平刷新顺序
    pub const MADCTL: u8 = 0x36;

    /// 像素格式设置 (Interface Pixel Format)
    /// 设置每个像素的颜色深度
    /// 参数值：
    ///   - 0x03: 12 位/像素 (RGB444)
    ///   - 0x05: 16 位/像素 (RGB565) ← 常用
    ///   - 0x06: 18 位/像素 (RGB666)
    pub const COLMOD: u8 = 0x3A;

    /// 帧率控制 1 (Frame Rate Control - In Normal Mode)
    /// 控制正常模式下的刷新率（帧频）
    /// 参数影响分频系数和内部时钟
    pub const FRMCTR1: u8 = 0xB1;

    /// 帧率控制 2 (Frame Rate Control - In Idle Mode)
    /// 控制空闲模式下的刷新率
    pub const FRMCTR2: u8 = 0xB2;

    /// 帧率控制 3 (Frame Rate Control - In Partial Mode)
    /// 控制部分模式下的刷新率
    pub const FRMCTR3: u8 = 0xB3;

    /// 显示反转控制 (Display Inversion Control)
    /// 控制显示反转的模式（如行反转、列反转等）
    pub const INVCTR: u8 = 0xB4;

    /// 显示功能设置 (Display Function Control)
    /// 控制扫描方向、源极/栅极驱动波形等
    pub const DISSET5: u8 = 0xB6;

    /// 电源控制 1 (Power Control 1)
    /// 控制 GVDD（伽马电压）等级，用于调节伽马参考电压
    /// 范围：0x00-0x1F，默认 0x17
    pub const PWCTR1: u8 = 0xC0;

    /// 电源控制 2 (Power Control 2)
    /// 控制 VGH（栅极高电压）和 VGL（栅极低电压）的幅值
    pub const PWCTR2: u8 = 0xC1;

    /// 电源控制 3 (Power Control 3)
    /// 在正常模式下控制源极驱动器相关的运算放大器
    pub const PWCTR3: u8 = 0xC2;

    /// 电源控制 4 (Power Control 4)
    /// 在空闲模式下控制源极驱动器相关的运算放大器
    pub const PWCTR4: u8 = 0xC3;

    /// 电源控制 5 (Power Control 5)
    /// 在部分模式下控制源极驱动器相关的运算放大器
    pub const PWCTR5: u8 = 0xC4;

    /// VCOM 控制 1 (VCOM Control 1)
    /// 控制 VCOM 电压（用于防止像素残留的电压）
    /// 参数决定 VCOMH 和 VCOML 的值
    pub const VMCTR1: u8 = 0xC5;

    /// VCOM 电压偏移控制 (VCOM Offset)
    /// 设置 VCOM 的偏移值
    pub const VMOFCTR: u8 = 0xC7;

    /// 写入 ABC 控制寄存器 (Write Command for External Display)
    /// 用于连接外部显示屏时的控制
    pub const WRABC: u8 = 0xD5;

    /// 读取 ID 1 (Read ID1)
    /// 读取第一个厂商 ID 字节
    pub const RDID1: u8 = 0xDA;

    /// 读取 ID 2 (Read ID2)
    /// 读取第二个厂商 ID 字节
    pub const RDID2: u8 = 0xDB;

    /// 读取 ID 3 (Read ID3)
    /// 读取第三个厂商 ID 字节
    pub const RDID3: u8 = 0xDC;

    /// 读取 ID 4 (Read ID4)
    /// 读取驱动 IC 版本代码
    pub const RDID4: u8 = 0xDD;

    /// 正极伽马校正 (Positive Gamma Correction)
    /// 设置伽马曲线的正值部分，用于调整显示的颜色准确性
    /// 需要发送 15 个参数字节
    pub const GMCTRP1: u8 = 0xE0;

    /// 负极伽马校正 (Negative Gamma Correction)
    /// 设置伽马曲线的负值部分，用于调整显示的颜色准确性
    /// 需要发送 15 个参数字节
    pub const GMCTRN1: u8 = 0xE1;
}

/// 硬件 SPI 显示驱动结构体
///
/// 封装了与 ILI9341 屏幕通信所需的硬件接口
///
/// # 类型参数
/// - `SPI`: 实现 embedded_hal::blocking::spi::Transfer<u8> 的 SPI 设备
/// - `CS`: 实现 OutputPin 的片选引脚（Chip Select，用于选中屏幕）
/// - `DC`: 实现 OutputPin 的数据/命令引脚（Data/Command，区分发送的是命令还是数据）
pub struct DisplaySpi<SPI, CS, DC> {
    /// SPI 外设实例，用于与屏幕进行串行数据传输
    /// 通过 MOSI (PB15) 发送数据，MISO (PB14) 接收数据，SCK (PB13) 提供时钟
    spi: SPI,

    /// 片选引脚 (Chip Select)
    /// 连接到屏幕的 CS 引脚 (PB12)
    /// 低电平有效：拉低表示选中屏幕，开始通信；拉高表示结束通信
    cs: CS,

    /// 数据/命令选择引脚 (Data/Command)
    /// 连接到屏幕的 D/C 引脚 (PB1)
    /// - 低电平：发送命令（如 CASET、PASET、RAMWR 等）
    /// - 高电平：发送数据（如像素颜色值、参数等）
    dc: DC,
}

/// DisplaySpi 的方法实现块
///
/// 实现屏幕初始化、数据传输和绘图功能
impl<SPI, CS, DC> DisplaySpi<SPI, CS, DC>
where
    SPI: Transfer<u8>, // SPI 必须支持 u8 类型的传输操作
    CS: OutputPin,     // CS 必须是输出引脚
    DC: OutputPin,     // DC 必须是输出引脚
{
    /// 创建新的显示驱动实例
    ///
    /// # 参数
    /// - `spi`: SPI 外设实例，用于数据传输
    /// - `cs`: 片选引脚，连接到屏幕的 CS (PB12)
    /// - `dc`: 数据/命令引脚，连接到屏幕的 D/C (PB1)
    ///
    /// # 返回
    /// 返回一个初始化好的 DisplaySpi 实例
    ///
    /// # 注意
    /// 此函数仅创建实例，不会初始化屏幕，需要调用 `init()` 方法
    pub fn new(spi: SPI, cs: CS, dc: DC) -> Self {
        Self { spi, cs, dc }
    }

    /// 通过 SPI 传输单个字节
    ///
    /// # 参数
    /// - `data`: 要发送的字节，同时接收的字节也会存储到这里
    ///
    /// # 返回
    /// 返回接收到的字节（MISO 引脚上的数据）
    ///
    /// # 工作原理
    /// SPI 是全双工通信：发送一个字节的同时也会接收一个字节
    /// 使用单元素数组作为缓冲区来满足 embedded_hal 的 Transfer trait
    fn transfer_byte(&mut self, data: u8) -> u8 {
        let mut buf = [data]; // 创建单元素缓冲区，存放要发送的数据
        self.spi.transfer(&mut buf).ok(); // 执行 SPI 传输
        buf[0] // 返回接收到的数据
    }

    /// 通过 SPI 传输多个字节（批量传输）
    ///
    /// # 参数
    /// - `data`: 要发送的数据缓冲区，同时接收的数据也会写入此缓冲区
    ///
    /// # 注意
    /// 此方法当前未使用，预留给未来可能需要的批量操作
    /// 如果需要一次发送多个字节，使用此方法比循环调用 transfer_byte 更高效
    fn transfer_bytes(&mut self, data: &mut [u8]) {
        self.spi.transfer(data).ok(); // 批量传输整个缓冲区
    }

    /// 向屏幕发送命令（公共接口，供 LVGL 使用）
    ///
    /// # 参数
    /// - `cmd`: 命令代码（如 SWRESET、CASET、RAMWR 等）
    pub fn write_command(&mut self, cmd: u8) {
        self.cs.set_low().ok(); // 拉低 CS，选中屏幕
        self.dc.set_low().ok(); // 拉低 DC，表示发送的是命令
        let _ = self.transfer_byte(cmd); // 发送命令字节
        self.cs.set_high().ok(); // 拉高 CS，结束通信
    }

    /// 向屏幕发送单个数据字节（公共接口，供 LVGL 使用）
    ///
    /// # 参数
    /// - `data`: 数据字节（如参数值、颜色分量等）
    pub fn write_data(&mut self, data: u8) {
        self.cs.set_low().ok(); // 拉低 CS，选中屏幕
        self.dc.set_high().ok(); // 拉高 DC，表示发送的是数据
        let _ = self.transfer_byte(data); // 发送数据字节
        self.cs.set_high().ok(); // 拉高 CS，结束通信
    }

    /// 向屏幕连续发送多个数据字节
    ///
    /// # 参数
    /// - `data`: 要发送的数据字节数组
    ///
    /// # 注意
    /// CS 在整个数据传输期间保持低电平，只切换一次
    /// 这比多次调用 write_data 更高效，适合发送大量数据（如伽马表）
    fn write_data_bytes(&mut self, data: &[u8]) {
        self.cs.set_low().ok(); // 拉低 CS，选中屏幕
        self.dc.set_high().ok(); // 拉高 DC，表示发送的是数据
        for &byte in data {
            let _ = self.transfer_byte(byte); // 逐字节发送
        }
        self.cs.set_high().ok(); // 拉高 CS，结束通信
    }

    /// 发送命令后立即紧跟一个数据字节
    ///
    /// # 参数
    /// - `cmd`: 命令代码
    /// - `data`: 紧跟命令后的参数数据
    ///
    /// # 用途
    /// 适用于只需要一个参数的命令，比分别调用 write_command 和 write_data 更高效
    /// 整个过程 CS 保持低电平，只切换 DC 引脚
    fn write_cmd_data(&mut self, cmd: u8, data: u8) {
        self.cs.set_low().ok(); // 拉低 CS，选中屏幕
        self.dc.set_low().ok(); // 拉低 DC，准备发送命令
        let _ = self.transfer_byte(cmd); // 发送命令字节
        self.dc.set_high().ok(); // 拉高 DC，切换到数据模式
        let _ = self.transfer_byte(data); // 发送数据字节
        self.cs.set_high().ok(); // 拉高 CS，结束通信
    }

    /// 向屏幕发送命令（公共版本，供外部模块使用）
    ///
    /// # 参数
    /// - `cmd`: 命令代码（如 SWRESET、CASET、RAMWR 等）
    pub fn send_command(&mut self, cmd: u8) {
        self.write_command(cmd);
    }

    /// 向屏幕发送单个数据字节（公共版本）
    ///
    /// # 参数
    /// - `data`: 数据字节
    pub fn send_data(&mut self, data: u8) {
        self.write_data(data);
    }

    /// 批量发送像素数据到屏幕（用于 LVGL 等图形库）
    ///
    /// # 参数
    /// - `x`: 起始 X 坐标
    /// - `y`: 起始 Y 坐标
    /// - `w`: 宽度
    /// - `h`: 高度
    /// - `pixels`: RGB565 像素数据（每个像素 2 字节，高位在前）
    pub fn write_pixels_raw(&mut self, x: u16, y: u16, w: u16, h: u16, pixels: &[u8]) {
        let x1 = (x + w - 1).min((DISPLAY_WIDTH - 1) as u16);
        let y1 = (y + h - 1).min((DISPLAY_HEIGHT - 1) as u16);

        // 设置地址窗口
        self.set_address_window(x, y, x1, y1);

        // 发送内存写入命令并开始传输数据
        let _ = self.cs.set_low();
        let _ = self.dc.set_low();
        let _ = self.transfer_byte(commands::RAMWR);
        let _ = self.dc.set_high();

        // 批量发送像素数据（使用缓冲区）
        let mut buf = [0u8; 256];
        let mut buf_idx = 0;

        for &byte in pixels {
            buf[buf_idx] = byte;
            buf_idx += 1;

            if buf_idx >= buf.len() {
                self.transfer_bytes(&mut buf);
                buf_idx = 0;
            }
        }

        // 发送剩余数据
        if buf_idx > 0 {
            self.transfer_bytes(&mut buf[..buf_idx]);
        }

        let _ = self.cs.set_high();
    }
    ///
    /// # 参数
    /// - `tx`: 可选的串口发送器，用于输出调试信息
    ///
    /// # 初始化流程
    /// 1. 软件复位 → 恢复所有寄存器到默认状态
    /// 2. 退出睡眠 → 唤醒屏幕
    /// 3. 设置颜色格式 → 16 位 RGB565
    /// 4. 设置内存访问控制 → 屏幕方向
    /// 5. 设置帧率 → 刷新频率
    /// 6. 设置电源控制 → 电压配置
    /// 7. 设置伽马校正 → 颜色准确性
    /// 8. 开启显示 → 开始显示图像
    ///
    /// # 注意
    /// 某些命令执行后需要延时，等待屏幕内部处理完成
    pub fn init(&mut self, tx: &mut impl embedded_hal::serial::Write<u8>) {
        // === 辅助函数：通过串口发送调试字符串 ===
        let mut write_str = |s: &str| {
            for b in s.bytes() {
                let _ = block!(tx.write(b)); // 阻塞式发送每个字节
            }
        };

        write_str("[Display] Start init...\r\n");

        // === 初始引脚状态 ===
        // 确保 CS 和 DC 为高电平（未选中状态）
        let _ = self.cs.set_high();
        let _ = self.dc.set_high();

        // === 1. 软件复位 ===
        // 将所有寄存器恢复到默认值
        write_str("[Display] Software reset...\r\n");
        self.write_command(commands::SWRESET);
        // 等待约 5ms，让屏幕完成复位操作
        // 使用 NOP 指令延时，避免依赖定时器
        for _ in 0..500000 {
            cortex_m::asm::nop();
        }

        // === 2. 退出睡眠模式 ===
        // 屏幕默认处于睡眠模式，需要唤醒才能正常工作
        write_str("[Display] Exit sleep mode...\r\n");
        self.write_command(commands::SLPOUT);
        // 等待约 5ms，让屏幕完成唤醒操作
        for _ in 0..500000 {
            cortex_m::asm::nop();
        }

        // === 3. 设置颜色格式 ===
        // 0x55 表示 16 位/像素，RGB565 格式（红 5 位，绿 6 位，蓝 5 位）
        // RGB565 每个像素占 2 字节，共 65536 种颜色
        write_str("[Display] Set color format (16-bit RGB565)...\r\n");
        self.write_cmd_data(commands::COLMOD, 0x55);

        // === 4. 设置内存访问控制 ===
        // 0x48 = 0100 1000 (二进制)
        //   - bit 7 (MY) = 0: 行地址顺序正常
        //   - bit 6 (MX) = 1: 列地址顺序反转
        //   - bit 5 (MV) = 0: 不交换行列
        //   - bit 4 (ML) = 0: 从上到下刷新
        //   - bit 3 (BGR) = 0: RGB 顺序
        // 这个配置将屏幕设置为竖屏模式
        write_str("[Display] Set memory access control...\r\n");
        self.write_cmd_data(commands::MADCTL, 0x48);

        // === 5. 设置帧率控制 ===
        // 控制屏幕刷新率，影响显示流畅度和功耗
        write_str("[Display] Set frame rate control...\r\n");
        self.write_cmd_data(commands::FRMCTR1, 0x00); // 第一个参数
        self.write_data(0x1B); // 第二个参数

        // === 6. 设置电源控制 ===
        // 配置屏幕内部电源电压，确保稳定显示
        write_str("[Display] Set power control...\r\n");
        self.write_command(commands::PWCTR1); // 电源控制 1：GVDD 电压
        self.write_data(0x23); // GVDD 电压 = 4.6V
        self.write_data(0x10); // 第二个参数

        self.write_cmd_data(commands::PWCTR2, 0x10); // 电源控制 2：VGH/VGL 电压

        self.write_cmd_data(commands::VMCTR1, 0x3E); // VCOM 控制：VCOMH 电压
        self.write_data(0x28); // VCOML 电压

        // === 7. 关闭显示反转 ===
        // 使用正常的颜色显示，不进行反转
        write_str("[Display] Set inversion off...\r\n");
        self.write_command(commands::INVOFF);

        // === 8. 设置伽马校正 ===
        // 伽马校正用于调整显示的颜色准确性，使颜色更自然
        // 正极伽马：控制亮色区域
        write_str("[Display] Set Gamma correction...\r\n");
        self.write_command(commands::GMCTRP1);
        self.write_data_bytes(&[
            0x0F, 0x31, 0x2B, 0x0C, 0x0E, 0x08, 0x4E, 0xF1, 0x37, 0x07, 0x10, 0x03, 0x0E, 0x09,
            0x00, // 15 个参数字节
        ]);

        // 负极伽马：控制暗色区域
        self.write_command(commands::GMCTRN1);
        self.write_data_bytes(&[
            0x00, 0x0E, 0x14, 0x03, 0x11, 0x07, 0x31, 0xC1, 0x48, 0x08, 0x0F, 0x0C, 0x31, 0x36,
            0x0F, // 15 个参数字节
        ]);

        // === 9. 开启显示 ===
        // 最后一步：开启显示输出，屏幕开始显示图像
        write_str("[Display] Turn on display...\r\n");
        self.write_command(commands::DISPON);
        // 等待显示电路稳定
        for _ in 0..100000 {
            cortex_m::asm::nop();
        }

        write_str("[Display] Init complete!\r\n");
    }

    /// 设置像素写入的矩形区域（地址窗口）
    ///
    /// # 参数
    /// - `x0`: 矩形左上角的 X 坐标（列起始位置，0-239）
    /// - `y0`: 矩形左上角的 Y 坐标（行起始位置，0-319）
    /// - `x1`: 矩形右下角的 X 坐标（列结束位置，0-239）
    /// - `y1`: 矩形右下角的 Y 坐标（行结束位置，0-319）
    ///
    /// # 工作原理
    /// ILI9341 不支持直接随机写入像素，必须先设置一个"窗口"，
    /// 然后连续发送像素数据填充这个窗口。
    ///
    /// # 通信协议
    /// ```text
    /// CASET 命令格式：
    ///   [CMD=0x2A][x0_H][x0_L][x1_H][x1_L]
    ///   例如设置 x=10 到 x=100：
    ///   [0x2A][0x00][0x0A][0x00][0x64]
    ///
    /// PASET 命令格式：
    ///   [CMD=0x2B][y0_H][y0_L][y1_H][y1_L]
    /// ```
    ///
    /// # 注意
    /// 设置窗口后，后续发送的像素数据会按顺序填充，
    /// 从左到右、从上到下，一行填满后自动换行。
    fn set_address_window(&mut self, x0: u16, y0: u16, x1: u16, y1: u16) {
        let _ = self.cs.set_low(); // 拉低 CS，选中屏幕，整个函数期间保持低电平

        // === 第一步：设置列地址（X 轴范围）CASET ===
        let _ = self.dc.set_low(); // 拉低 DC，发送命令模式
        let _ = self.transfer_byte(commands::CASET); // 发送 CASET 命令
        let _ = self.dc.set_high(); // 拉高 DC，切换到数据模式
                                    // 批量发送 x0 和 x1 的四个字节（高字节在前）
        let mut x_buf = [
            (x0 >> 8) as u8,   // x0 高字节
            (x0 & 0xFF) as u8, // x0 低字节
            (x1 >> 8) as u8,   // x1 高字节
            (x1 & 0xFF) as u8, // x1 低字节
        ];
        self.transfer_bytes(&mut x_buf);

        // === 第二步：设置页地址（Y 轴范围）PASET ===
        let _ = self.dc.set_low(); // 拉低 DC，发送命令模式
        let _ = self.transfer_byte(commands::PASET); // 发送 PASET 命令
        let _ = self.dc.set_high(); // 拉高 DC，切换到数据模式
                                    // 批量发送 y0 和 y1 的四个字节（高字节在前）
        let mut y_buf = [
            (y0 >> 8) as u8,   // y0 高字节
            (y0 & 0xFF) as u8, // y0 低字节
            (y1 >> 8) as u8,   // y1 高字节
            (y1 & 0xFF) as u8, // y1 低字节
        ];
        self.transfer_bytes(&mut y_buf);

        let _ = self.cs.set_high(); // 拉高 CS，结束通信
    }

    /// 填充整个屏幕为指定颜色
    ///
    /// # 参数
    /// - `color`: 填充颜色（Rgb565 格式）
    ///
    /// # 工作原理
    /// 设置窗口为全屏 (0, 0) 到 (239, 319)，然后发送 240×320 个像素
    pub fn fill_screen(&mut self, color: Rgb565) {
        self.fill_rect(0, 0, DISPLAY_WIDTH as u16, DISPLAY_HEIGHT as u16, color);
    }

    /// 填充矩形区域为指定颜色
    ///
    /// # 参数
    /// - `x`: 矩形左上角的 X 坐标
    /// - `y`: 矩形左上角的 Y 坐标
    /// - `w`: 矩形宽度（像素数）
    /// - `h`: 矩形高度（像素数）
    /// - `color`: 填充颜色（Rgb565 格式）
    ///
    /// # 工作流程
    /// 1. 计算矩形结束坐标（并进行边界检查）
    /// 2. 设置地址窗口
    /// 3. 发送 RAMWR 命令
    /// 4. 连续发送像素数据
    ///
    /// # 注意
    /// 整个填充过程中 CS 保持低电平，只切换一次，
    /// 这样可以大幅提高传输速度。
    pub fn fill_rect(&mut self, x: u16, y: u16, w: u16, h: u16, color: Rgb565) {
        // === 第一步：计算结束坐标并进行边界检查 ===
        // 结束坐标 = 起始坐标 + 宽度/高度 - 1
        // 使用 min() 确保不超过屏幕边界
        let x1 = (x + w - 1).min((DISPLAY_WIDTH - 1) as u16);
        let y1 = (y + h - 1).min((DISPLAY_HEIGHT - 1) as u16);

        // === 第二步：设置地址窗口 ===
        self.set_address_window(x, y, x1, y1);

        // === 第三步：准备颜色数据 ===
        // Rgb565 转 u16：16 位颜色值
        let pixel_color = color.into_storage();
        // 将 16 位颜色拆分为两个字节
        let high_byte = (pixel_color >> 8) as u8; // 高字节
        let low_byte = (pixel_color & 0xFF) as u8; // 低字节

        // 计算需要发送的像素总数
        let num_pixels = (w as u32) * (h as u32);

        // === 第四步：发送 RAMWR 命令并连续传输像素数据 ===
        // 关键优化：在整个填充过程中保持 CS 低电平
        // 避免频繁切换 CS 引脚，大幅提高速度

        // 批量传输：使用 512 字节缓冲区
        const BUF_SIZE: usize = 512;
        let mut buf = [0u8; BUF_SIZE];
        let buf_pixels = BUF_SIZE / 2; // 每个像素 2 字节
        let mut remaining = num_pixels as usize;

        let _ = self.cs.set_low();
        let _ = self.dc.set_low();
        let _ = self.transfer_byte(commands::RAMWR);
        let _ = self.dc.set_high();

        while remaining > 0 {
            let chunk = remaining.min(buf_pixels);
            // 填充缓冲区
            for i in 0..chunk {
                buf[i * 2] = high_byte;
                buf[i * 2 + 1] = low_byte;
            }
            // 批量发送
            self.transfer_bytes(&mut buf[..chunk * 2]);
            remaining -= chunk;
        }

        let _ = self.cs.set_high();
    }

    /// 测试原始 SPI 传输速度
    /// 发送 10000 字节，测量实际吞吐量
    pub fn test_spi_speed(&mut self) -> u32 {
        let test_bytes = 10000u32;

        let _ = self.cs.set_low();
        let _ = self.dc.set_high();

        for _ in 0..test_bytes {
            let _ = self.transfer_byte(0x55);
        }

        let _ = self.cs.set_high();

        test_bytes
    }

    /// 使用批量传输测试 SPI 速度（绕过逐字节开销）
    pub fn test_spi_speed_bulk(&mut self) -> u32 {
        let test_bytes = 10000u32;
        let mut buf = [0x55u8; 100];

        let _ = self.cs.set_low();
        let _ = self.dc.set_high();

        for _ in 0..(test_bytes / 100) {
            self.transfer_bytes(&mut buf);
        }

        let _ = self.cs.set_high();

        test_bytes
    }
}

/// 为 DisplaySpi 实现 embedded_graphics::draw_target::DrawTarget trait
///
/// 这使得屏幕可以与 embedded_graphics 库的所有绘图功能兼容，
/// 包括绘制文本、形状、图像等。
///
/// # 使用示例
/// ```ignore
/// use embedded_graphics::prelude::*;
/// use embedded_graphics::primitives::Circle;
///
/// let circle = Circle::new(Point::new(120, 160), 50);
/// circle.into_styled(PrimitiveStyle::with_fill(Rgb565::RED))
///      .draw(&mut display)?;
/// ```
impl<SPI, CS, DC> DrawTarget for DisplaySpi<SPI, CS, DC>
where
    SPI: Transfer<u8>,
    CS: OutputPin,
    DC: OutputPin,
{
    /// 关联类型：颜色格式为 Rgb565
    type Color = Rgb565;

    /// 关联类型：错误类型
    /// 使用 Infallible 表示此操作不会失败（忽略硬件错误）
    type Error = core::convert::Infallible;

    /// 逐像素绘制（最基础的绘制方法）
    ///
    /// # 参数
    /// - `pixels`: 像素迭代器，每个元素包含坐标和颜色
    ///
    /// # 工作原理
    /// 收集像素到缓冲区，批量设置地址窗口，一次性发送所有数据。
    /// 这比逐像素绘制快 10-100 倍。
    ///
    /// # 优化说明
    /// - 使用 512 字节的缓冲区批量发送
    /// - 只设置一次地址窗口（覆盖所有像素的边界框）
    /// - 跳过边界框外的像素位置（保持背景）
    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        // 收集像素到临时缓冲区
        let mut pixel_buffer: [(u16, u16, Rgb565); 256] = [(0, 0, Rgb565::BLACK); 256];
        let mut count = 0;
        let mut min_x = DISPLAY_WIDTH as u16;
        let mut min_y = DISPLAY_HEIGHT as u16;
        let mut max_x = 0u16;
        let mut max_y = 0u16;

        for Pixel(point, color) in pixels.into_iter() {
            let x = point.x as u16;
            let y = point.y as u16;

            // 边界检查
            if x < DISPLAY_WIDTH as u16 && y < DISPLAY_HEIGHT as u16 && count < 256 {
                pixel_buffer[count] = (x, y, color);
                count += 1;

                // 更新边界框
                if x < min_x {
                    min_x = x;
                }
                if y < min_y {
                    min_y = y;
                }
                if x > max_x {
                    max_x = x;
                }
                if y > max_y {
                    max_y = y;
                }
            }
        }

        if count == 0 {
            return Ok(());
        }

        // 批量绘制：设置覆盖所有像素的地址窗口
        self.set_address_window(min_x, min_y, max_x, max_y);

        // 发送 RAMWR 命令
        self.write_command(commands::RAMWR);

        // 批量发送像素数据
        let _ = self.cs.set_low();
        let _ = self.dc.set_high();

        // 使用缓冲区批量传输
        let mut buf = [0u8; 256]; // 128 像素 × 2 字节
        let mut buf_idx = 0;

        // 遍历边界框内的每个位置
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                // 查找这个位置是否有像素
                let mut pixel_color: Option<u16> = None;
                for i in 0..count {
                    if pixel_buffer[i].0 == x && pixel_buffer[i].1 == y {
                        pixel_color = Some(pixel_buffer[i].2.into_storage());
                        break;
                    }
                }

                // 写入缓冲区（如果没有像素，发送黑色）
                let color = pixel_color.unwrap_or(0);
                buf[buf_idx] = (color >> 8) as u8;
                buf[buf_idx + 1] = (color & 0xFF) as u8;
                buf_idx += 2;

                // 缓冲区满时发送
                if buf_idx >= buf.len() {
                    self.transfer_bytes(&mut buf);
                    buf_idx = 0;
                }
            }
        }

        // 发送剩余数据
        if buf_idx > 0 {
            self.transfer_bytes(&mut buf[..buf_idx]);
        }

        let _ = self.cs.set_high();

        Ok(())
    }

    /// 填充连续区域的像素（每个像素可以有不同的颜色）
    ///
    /// # 参数
    /// - `area`: 要填充的矩形区域
    /// - `colors`: 颜色迭代器，按从左到右、从上到下的顺序提供颜色
    ///
    /// # 用途
    /// 适用于绘制图像、渐变等每个像素颜色不同的场景。
    ///
    /// # 工作原理
    /// 1. 计算区域与屏幕边界的交集（裁剪超出部分）
    /// 2. 设置地址窗口
    /// 3. 连续发送所有像素的颜色数据
    fn fill_contiguous<I>(&mut self, area: &Rectangle, colors: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Self::Color>,
    {
        // === 第一步：计算有效区域（与屏幕边界取交集） ===
        // 这会裁剪掉超出屏幕范围的部分
        let area = area.intersection(&Rectangle::new(
            Point::zero(),
            Size::new(DISPLAY_WIDTH as u32, DISPLAY_HEIGHT as u32),
        ));

        // 如果区域为空，直接返回
        if area.size.width == 0 || area.size.height == 0 {
            return Ok(());
        }

        // === 第二步：计算窗口坐标 ===
        let x0 = area.top_left.x as u16;
        let y0 = area.top_left.y as u16;
        let x1 = x0 + area.size.width as u16 - 1;
        let y1 = y0 + area.size.height as u16 - 1;

        // === 第三步：设置窗口并发送数据 ===
        self.set_address_window(x0, y0, x1, y1);
        self.write_command(commands::RAMWR);

        let _ = self.cs.set_low();
        let _ = self.dc.set_high();

        // 使用缓冲区批量发送像素数据
        let mut buf = [0u8; 256]; // 128 像素 × 2 字节
        let mut buf_idx = 0;

        for color in colors {
            let pixel_color = color.into_storage();
            buf[buf_idx] = (pixel_color >> 8) as u8;
            buf[buf_idx + 1] = (pixel_color & 0xFF) as u8;
            buf_idx += 2;

            // 缓冲区满时发送
            if buf_idx >= buf.len() {
                self.transfer_bytes(&mut buf);
                buf_idx = 0;
            }
        }

        // 发送剩余数据
        if buf_idx > 0 {
            self.transfer_bytes(&mut buf[..buf_idx]);
        }

        let _ = self.cs.set_high();

        Ok(())
    }

    /// 用单一颜色填充矩形区域
    ///
    /// # 参数
    /// - `area`: 要填充的矩形区域
    /// - `color`: 填充颜色
    ///
    /// # 用途
    /// 适用于绘制纯色矩形、清除区域等场景。
    /// 这是最快的填充方法，因为所有像素颜色相同。
    fn fill_solid(&mut self, area: &Rectangle, color: Self::Color) -> Result<(), Self::Error> {
        // === 第一步：计算有效区域 ===
        let area = area.intersection(&Rectangle::new(
            Point::zero(),
            Size::new(DISPLAY_WIDTH as u32, DISPLAY_HEIGHT as u32),
        ));

        // 如果区域为空，直接返回
        if area.size.width == 0 || area.size.height == 0 {
            return Ok(());
        }

        // === 第二步：调用 fill_rect 方法 ===
        // 使用优化的 fill_rect 方法进行填充
        self.fill_rect(
            area.top_left.x as u16,
            area.top_left.y as u16,
            area.size.width as u16,
            area.size.height as u16,
            color,
        );

        Ok(())
    }

    /// 清空整个屏幕（用指定颜色填充）
    ///
    /// # 参数
    /// - `color`: 填充颜色
    ///
    /// # 用途
    /// 适用于清除屏幕、设置背景色等场景。
    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.fill_screen(color);
        Ok(())
    }
}

/// 为 DisplaySpi 实现 OriginDimensions trait
///
/// 这个 trait 允许外部代码查询屏幕的尺寸，
/// 用于居中显示、边界检查等场景。
impl<SPI, CS, DC> OriginDimensions for DisplaySpi<SPI, CS, DC>
where
    SPI: Transfer<u8>,
    CS: OutputPin,
    DC: OutputPin,
{
    /// 返回屏幕的尺寸
    ///
    /// # 返回
    /// 返回一个 Size 对象，包含宽度和高度（单位：像素）
    fn size(&self) -> Size {
        Size::new(DISPLAY_WIDTH as u32, DISPLAY_HEIGHT as u32)
    }
}
