//! 8x8 像素图标

use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::*,
    primitives::Rectangle,
};

/// 8x8 像素图标
#[derive(Clone, Copy)]
pub enum PixelIcon {
    Thermo,   // 温度计 🌡
    Humid,    // 湿度计 💧
    Home,     // 主页 🏠
    Settings, // 设置 ⚙
}

impl PixelIcon {
    /// 获取 8x8 像素数据（每个 bit 代表一个像素）
    pub fn data(&self) -> &[u8; 8] {
        match self {
            Self::Thermo => &THERMO_ICON,
            Self::Humid => &HUMID_ICON,
            Self::Home => &HOME_ICON,
            Self::Settings => &SETTINGS_ICON,
        }
    }

    /// 绘制图标到显示设备
    ///
    /// x, y: 左上角坐标
    /// scale: 缩放倍数（1=原始8x8, 2=16x16）
    pub fn draw<D>(
        &self,
        display: &mut D,
        x: i32,
        y: i32,
        scale: u32,
        color: Rgb565,
    ) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        let data = self.data();
        let pixel_size = Size::new(scale, scale);

        for row in 0..8 {
            for col in 0..8 {
                if (data[row] >> (7 - col)) & 1 == 1 {
                    let px = x + col as i32 * scale as i32;
                    let py = y + row as i32 * scale as i32;
                    display.fill_solid(
                        &Rectangle::new(Point::new(px, py), pixel_size),
                        color,
                    )?;
                }
            }
        }
        Ok(())
    }
}

// 图标像素数据（8x8）
const THERMO_ICON: [u8; 8] = [
    0b00111100,  //   ██
    0b01000010,  //  ██
    0b01000010,  //  ██
    0b00111100,  //   ██
    0b00111100,  //   ██
    0b00100100,  //   ▐▌
    0b00100100,  //   ▐▌
    0b00100100,  //   ▐▌
];

const HUMID_ICON: [u8; 8] = [
    0b00100100,  //   ▐▌
    0b00100100,  //   ▐▌
    0b00100100,  //   ▐▌
    0b01111110,  //  █████
    0b11111111,  // ███████
    0b11111111,  // ███████
    0b11111111,  // ███████
    0b01101110,  //  ██ ██
];

const HOME_ICON: [u8; 8] = [
    0b00011000,  //    ██
    0b00111100,  //   ████
    0b01111110,  //  ██████
    0b11111111,  // ███████
    0b10011001,  // ██  ██
    0b10011001,  // ██  ██
    0b10011001,  // ██  ██
    0b10000001,  // ██    ██
];

const SETTINGS_ICON: [u8; 8] = [
    0b00100100,  //   ▐▌
    0b01111110,  //  █████
    0b11111111,  // ███████
    0b11111111,  // ███████
    0b00111100,  //   ████
    0b00100100,  //   ▐▌
    0b00100100,  //   ▐▌
    0b00100100,  //   ▐▌
];
