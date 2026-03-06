//! 历史记录条
//!
//! 注意：此控件为预留功能，暂未在主程序中使用

#![allow(dead_code)]

use super::BoundingBox;
use crate::ui::GrayTheme;
use embedded_graphics::{
    mono_font::{ascii::FONT_10X20, MonoTextStyle},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::Rectangle,
    text::{Baseline, Text},
};

/// 历史记录条（280x60）
#[derive(Clone, Copy)]
pub struct HistoryBar {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub data: [f32; 6],
    pub count: usize,
    pub theme: GrayTheme,
}

impl HistoryBar {
    /// 创建新的历史记录条
    pub fn new(x: i32, y: i32) -> Self {
        Self {
            x,
            y,
            width: 280,
            height: 60,
            data: [0.0; 6],
            count: 0,
            theme: GrayTheme::new(),
        }
    }

    /// 设置主题
    pub fn with_theme(mut self, theme: GrayTheme) -> Self {
        self.theme = theme;
        self
    }

    /// 更新数据
    pub fn update(&mut self, data: &[f32]) {
        let len = data.len().min(6);
        self.data[..len].copy_from_slice(&data[..len]);
        self.count = len;
    }

    /// 获取边界框
    pub fn bounding_box(&self) -> BoundingBox {
        BoundingBox::new(self.x, self.y, self.width, self.height)
    }

    /// 绘制历史记录条
    pub fn draw<D>(&self, display: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        // 1. 绘制背景
        display.fill_solid(
            &Rectangle::new(
                Point::new(self.x, self.y),
                Size::new(self.width, self.height),
            ),
            self.theme.g7,
        )?;

        // 2. 绘制边框
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
        let style = MonoTextStyle::new(&FONT_10X20, self.theme.g0);
        let text = Text::with_baseline(
            "HISTORY (24h)",
            Point::new(self.x + 10, self.y + 5),
            style,
            Baseline::Top,
        );
        text.draw(display)?;

        // 4. 绘制数据格子
        let cell_width = 40;
        let cell_height = 30;
        let start_x = self.x + 15;
        let start_y = self.y + 25;

        for i in 0..self.count {
            let cell_x = start_x + i as i32 * (cell_width as i32 + 5);

            // 格子边框
            display.fill_solid(
                &Rectangle::new(
                    Point::new(cell_x, start_y),
                    Size::new(cell_width, cell_height),
                ),
                self.theme.g6,
            )?;
            display.fill_solid(
                &Rectangle::new(Point::new(cell_x, start_y), Size::new(cell_width, 1)),
                self.theme.g0,
            )?;
            display.fill_solid(
                &Rectangle::new(
                    Point::new(cell_x, start_y + cell_height as i32 - 1),
                    Size::new(cell_width, 1),
                ),
                self.theme.g0,
            )?;
            display.fill_solid(
                &Rectangle::new(Point::new(cell_x, start_y), Size::new(1, cell_height)),
                self.theme.g0,
            )?;
            display.fill_solid(
                &Rectangle::new(
                    Point::new(cell_x + cell_width as i32 - 1, start_y),
                    Size::new(1, cell_height),
                ),
                self.theme.g0,
            )?;

            // 数值
            use core::fmt::Write;
            let mut value_str = heapless::String::<16>::new();
            let _ = write!(value_str, "{:.0}", self.data[i]);
            let value_text = Text::with_baseline(
                &value_str,
                Point::new(
                    cell_x + cell_width as i32 / 2 - value_str.len() as i32 * 5,
                    start_y + 5,
                ),
                style,
                Baseline::Top,
            );
            value_text.draw(display)?;
        }

        Ok(())
    }
}
