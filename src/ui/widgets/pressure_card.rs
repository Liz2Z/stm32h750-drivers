//! 气压显示卡片
//!
//! 注意：部分方法为预留功能，暂未在主程序中使用

#![allow(dead_code)]

use super::{BoundingBox, ValueRange, ValueStatus};
use crate::ui::{sensor_format, GrayTheme, PixelIcon};
use embedded_graphics::{
    mono_font::{ascii::FONT_10X20, MonoTextStyle},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::Rectangle,
    text::{Baseline, Text},
};

/// 气压卡片声明的输入：以 hPa 为单位的数值范围和提示状态。
/// None 显示占位符；控件不接收历史或采样时间戳。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PressureCardData {
    pub values: Option<ValueRange>,
    pub status: ValueStatus,
}

/// 气压显示卡片（100x200）
#[derive(Clone, Copy)]
pub struct PressureCard {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub data: PressureCardData,
    pub theme: GrayTheme,
}

impl PressureCard {
    pub fn new(x: i32, y: i32) -> Self {
        Self {
            x,
            y,
            width: 100,
            height: 200,
            data: PressureCardData::default(),
            theme: GrayTheme::new(),
        }
    }

    /// 设置主题
    pub fn with_theme(mut self, theme: GrayTheme) -> Self {
        self.theme = theme;
        self
    }

    /// 更新控件输入，不执行采样或统计。
    pub fn update(&mut self, data: PressureCardData) {
        self.data = data;
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

        // 4. 绘制气压图标（32x32）
        PixelIcon::Barometer.draw(
            display,
            self.x + self.width as i32 / 2 - 16,
            self.y + 30,
            2,
            self.theme.g0,
        )?;

        self.draw_values(display)
    }

    /// Static titles, icons and borders stay in the framebuffer.
    pub fn values_bounds(&self) -> BoundingBox {
        BoundingBox::new(self.x + 1, self.y + 75, self.width - 2, 115)
    }

    pub fn draw_values<D>(&self, display: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        display.fill_solid(&self.values_bounds().into_rectangle(), self.theme.g7)?;
        let style = MonoTextStyle::new(&FONT_10X20, self.theme.g0);
        // 5. 绘制当前气压值
        let value_str = sensor_format::pressure(self.data.values);
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
            if let Some(values) = self.data.values {
                let _ = write!(s, "hi:{:.0}", values.high);
            } else {
                let _ = s.push_str("hi:--");
            }
            s
        };
        let low = {
            let mut s = heapless::String::<16>::new();
            if let Some(values) = self.data.values {
                let _ = write!(s, "lo:{:.0}", values.low);
            } else {
                let _ = s.push_str("lo:--");
            }
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

        Text::with_baseline(
            sensor_format::state_label(self.data.status),
            Point::new(self.x + 10, self.y + 170),
            style,
            Baseline::Top,
        )
        .draw(display)?;

        Ok(())
    }
}
