//! 温湿度显示卡片

use super::BoundingBox;
use crate::ui::{GrayTheme, PixelIcon, TempHumidSensor};
use embedded_graphics::{
    mono_font::{ascii::FONT_10X20, MonoTextStyle},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::Rectangle,
    text::{Baseline, Text},
};

/// 温湿度显示卡片（145x120）
#[derive(Clone, Copy)]
pub struct TempHumidCard {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub sensor: TempHumidSensor,
    pub theme: GrayTheme,
    pub show_temp: bool,
}

impl TempHumidCard {
    pub fn new(x: i32, y: i32, show_temp: bool) -> Self {
        Self {
            x,
            y,
            width: 145,
            height: 120,
            sensor: TempHumidSensor::new(),
            theme: GrayTheme::new(),
            show_temp,
        }
    }

    /// 设置主题
    pub fn with_theme(mut self, theme: GrayTheme) -> Self {
        self.theme = theme;
        self
    }

    /// 更新传感器数据
    pub fn update(&mut self, data: TempHumidSensor) {
        self.sensor = data;
    }

    /// 获取边界框
    pub fn bounding_box(&self) -> BoundingBox {
        BoundingBox::new(self.x, self.y, self.width, self.height)
    }

    /// 绘制卡片
    pub fn draw<D>(&self, display: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        // 1. 绘制卡片背景（白色）
        display.fill_solid(
            &Rectangle::new(
                Point::new(self.x, self.y),
                Size::new(self.width, self.height),
            ),
            self.theme.g7,
        )?;

        // 2. 绘制黑色边框
        display.fill_solid(
            &Rectangle::new(Point::new(self.x, self.y), Size::new(self.width, 1)),
            self.theme.g0,
        )?;
        display.fill_solid(
            &Rectangle::new(
                Point::new(self.x, self.y + self.height as i32 - 1),
                Size::new(self.width, 1),
            ),
            self.theme.g0,
        )?;
        display.fill_solid(
            &Rectangle::new(Point::new(self.x, self.y), Size::new(1, self.height)),
            self.theme.g0,
        )?;
        display.fill_solid(
            &Rectangle::new(
                Point::new(self.x + self.width as i32 - 1, self.y),
                Size::new(1, self.height),
            ),
            self.theme.g0,
        )?;

        // 3. 绘制标题
        let title = if self.show_temp { "THERMO" } else { "HUMID" };
        let style = MonoTextStyle::new(&FONT_10X20, self.theme.g0);
        let text = Text::with_baseline(
            title,
            Point::new(
                self.x + self.width as i32 / 2 - title.len() as i32 * 5,
                self.y + 5,
            ),
            style,
            Baseline::Top,
        );
        text.draw(display)?;

        // 4. 绘制图标（居中，16x16 放大）
        let icon = if self.show_temp {
            PixelIcon::Thermo
        } else {
            PixelIcon::Humid
        };
        let icon_x = self.x + self.width as i32 / 2 - 8;
        let icon_y = self.y + 30;
        icon.draw(display, icon_x, icon_y, 2, self.theme.g0)?;

        // 5. 绘制当前数值
        let value_str = if self.show_temp {
            self.sensor.temp_str()
        } else {
            self.sensor.humid_str()
        };
        let value_text = Text::with_baseline(
            &value_str,
            Point::new(
                self.x + self.width as i32 / 2 - value_str.len() as i32 * 5,
                self.y + 60,
            ),
            style,
            Baseline::Top,
        );
        value_text.draw(display)?;

        // 6. 绘制最高/最低值
        use core::fmt::Write;
        let (high, low) = if self.show_temp {
            (
                {
                    let mut s = heapless::String::<16>::new();
                    let _ = write!(s, "hi:{:.0}C", self.sensor.temp_high);
                    s
                },
                {
                    let mut s = heapless::String::<16>::new();
                    let _ = write!(s, "lo:{:.0}C", self.sensor.temp_low);
                    s
                },
            )
        } else {
            (
                {
                    let mut s = heapless::String::<16>::new();
                    let _ = write!(s, "hi:{}%", self.sensor.humid_high as i32);
                    s
                },
                {
                    let mut s = heapless::String::<16>::new();
                    let _ = write!(s, "lo:{}%", self.sensor.humid_low as i32);
                    s
                },
            )
        };

        let high_text = Text::with_baseline(
            &high,
            Point::new(self.x + 10, self.y + 90),
            style,
            Baseline::Top,
        );
        high_text.draw(display)?;

        let low_text = Text::with_baseline(
            &low,
            Point::new(self.x + 70, self.y + 90),
            style,
            Baseline::Top,
        );
        low_text.draw(display)?;

        Ok(())
    }
}
