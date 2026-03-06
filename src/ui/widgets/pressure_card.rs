//! 气压显示卡片

use super::BoundingBox;
use crate::ui::{GrayTheme, PressureSensor};
use embedded_graphics::{
    mono_font::{ascii::FONT_10X20, MonoTextStyle},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::Rectangle,
    text::{Baseline, Text},
};

/// 气压显示卡片（100x200）
#[derive(Clone, Copy)]
pub struct PressureCard {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub sensor: PressureSensor,
    pub theme: GrayTheme,
}

impl PressureCard {
    pub fn new(x: i32, y: i32) -> Self {
        Self {
            x,
            y,
            width: 100,
            height: 200,
            sensor: PressureSensor::new(),
            theme: GrayTheme::new(),
        }
    }

    /// 设置主题
    pub fn with_theme(mut self, theme: GrayTheme) -> Self {
        self.theme = theme;
        self
    }

    /// 更新传感器数据
    pub fn update(&mut self, data: PressureSensor) {
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
        let title = "PRESS";
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

        // 4. 绘制气压图标（使用简单的文字代替）
        let icon_text = "P";
        let icon_style = MonoTextStyle::new(&FONT_10X20, self.theme.g0);
        let icon = Text::with_baseline(
            icon_text,
            Point::new(self.x + self.width as i32 / 2 - 5, self.y + 35),
            icon_style,
            Baseline::Top,
        );
        icon.draw(display)?;

        // 5. 绘制当前气压值
        let value_str = self.sensor.pressure_str();
        let value_text = Text::with_baseline(
            &value_str,
            Point::new(
                self.x + self.width as i32 / 2 - value_str.len() as i32 * 5,
                self.y + 80,
            ),
            style,
            Baseline::Top,
        );
        value_text.draw(display)?;

        // 6. 绘制最高/最低值
        use core::fmt::Write;
        let high = {
            let mut s = heapless::String::<16>::new();
            let _ = write!(s, "hi:{:.0}", self.sensor.pressure_high);
            s
        };
        let low = {
            let mut s = heapless::String::<16>::new();
            let _ = write!(s, "lo:{:.0}", self.sensor.pressure_low);
            s
        };

        let high_text = Text::with_baseline(
            &high,
            Point::new(self.x + 10, self.y + 120),
            style,
            Baseline::Top,
        );
        high_text.draw(display)?;

        let low_text = Text::with_baseline(
            &low,
            Point::new(self.x + 10, self.y + 145),
            style,
            Baseline::Top,
        );
        low_text.draw(display)?;

        Ok(())
    }
}
