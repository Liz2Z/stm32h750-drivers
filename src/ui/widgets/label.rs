//! 标签控件

use super::BoundingBox;
use crate::ui::GrayTheme;
use embedded_graphics::{
    mono_font::{ascii::FONT_10X20, MonoTextStyle},
    pixelcolor::Rgb565,
    prelude::*,
    text::{Baseline, Text},
};

/// 标签控件
#[derive(Clone, Copy)]
pub struct Label {
    pub x: i32,
    pub y: i32,
    pub text: &'static str,
    pub theme: GrayTheme,
    pub centered: bool,
}

impl Label {
    pub fn new(x: i32, y: i32, text: &'static str) -> Self {
        Self {
            x,
            y,
            text,
            theme: GrayTheme::new(),
            centered: false,
        }
    }

    pub fn with_theme(mut self, theme: GrayTheme) -> Self {
        self.theme = theme;
        self
    }

    pub fn centered(mut self) -> Self {
        self.centered = true;
        self
    }

    /// 获取边界框（估算）
    pub fn bounding_box(&self) -> BoundingBox {
        let width = (self.text.len() * 12) as u32;
        let height = 20u32;
        if self.centered {
            BoundingBox::new(self.x - width as i32 / 2, self.y, width, height)
        } else {
            BoundingBox::new(self.x, self.y, width, height)
        }
    }

    pub fn draw<D>(&self, display: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        let style = MonoTextStyle::new(&FONT_10X20, self.theme.text());

        let text = if self.centered {
            Text::with_baseline(
                self.text,
                Point::new(self.x - (self.text.len() * 6) as i32 / 2, self.y),
                style,
                Baseline::Top,
            )
        } else {
            Text::with_baseline(self.text, Point::new(self.x, self.y), style, Baseline::Top)
        };

        text.draw(display)?;
        Ok(())
    }
}
