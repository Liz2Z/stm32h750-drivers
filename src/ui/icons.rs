//! 16x16 像素图标

use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::*,
    primitives::Rectangle,
};

/// 16x16 像素图标
#[derive(Clone, Copy)]
pub enum PixelIcon {
    Thermo,   // 温度计 🌡
    Humid,    // 湿度计 💧
    Home,     // 主页 🏠
    Settings, // 设置 ⚙
}

impl PixelIcon {
    /// 获取 16x16 像素数据
    pub fn data(&self) -> &[u8; 32] {
        match self {
            Self::Thermo => &THERMO_ICON,
            Self::Humid => &HUMID_ICON,
            Self::Home => &HOME_ICON,
            Self::Settings => &SETTINGS_ICON,
        }
    }

    /// 绘制图标到显示设备
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

        for row in 0..16 {
            let row_data = ((data[row * 2] as u16) << 8) | (data[row * 2 + 1] as u16);

            for col in 0..16 {
                if (row_data >> (15 - col)) & 1 == 1 {
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

const THERMO_ICON: [u8; 32] = [
    0x03, 0xC0,
    0x04, 0x20,
    0x04, 0x20,
    0x04, 0xA0,
    0x04, 0xA0,
    0x04, 0xA0,
    0x04, 0xA0,
    0x04, 0xA0,
    0x08, 0x10,
    0x11, 0x88,
    0x13, 0xC8,
    0x13, 0xC8,
    0x11, 0x88,
    0x08, 0x10,
    0x07, 0xE0,
    0x00, 0x00,
];

const HUMID_ICON: [u8; 32] = [
    0x01, 0x80,
    0x02, 0x40,
    0x02, 0x40,
    0x04, 0x20,
    0x04, 0x20,
    0x08, 0x10,
    0x10, 0x08,
    0x20, 0x04,
    0x40, 0x02,
    0x40, 0xC2,
    0x42, 0x02,
    0x20, 0x04,
    0x10, 0x08,
    0x0F, 0xF0,
    0x00, 0x00,
    0x00, 0x00,
];

const HOME_ICON: [u8; 32] = [
    0x01, 0x80,
    0x02, 0x40,
    0x04, 0x20,
    0x08, 0x10,
    0x10, 0x08,
    0x20, 0x04,
    0x7F, 0xFE,
    0x08, 0x10,
    0x08, 0x10,
    0x0B, 0xD0,
    0x0A, 0x50,
    0x0A, 0x50,
    0x0A, 0xD0,
    0x0A, 0x50,
    0x0F, 0xF0,
    0x00, 0x00,
];

const SETTINGS_ICON: [u8; 32] = [
    0x01, 0x80,
    0x02, 0x40,
    0x38, 0x1C,
    0x40, 0x02,
    0x43, 0xC2,
    0x84, 0x21,
    0x88, 0x11,
    0x88, 0x11,
    0x88, 0x11,
    0x88, 0x11,
    0x84, 0x21,
    0x43, 0xC2,
    0x40, 0x02,
    0x38, 0x1C,
    0x02, 0x40,
    0x01, 0x80,
];
