//! 按钮控件
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

/// 按钮控件
pub struct Button {
    /// 按钮位置
    pub x: i32,
    pub y: i32,
    /// 按钮尺寸
    pub width: u32,
    pub height: u32,
    /// 按钮文字
    pub text: &'static str,
    /// 主题
    pub theme: GrayTheme,
    /// 是否被按下
    pub pressed: bool,
    /// 按钮ID
    pub id: u32,
    /// 是否启用
    pub enabled: bool,
}

impl Button {
    /// 创建新按钮
    pub fn new(id: u32, x: i32, y: i32, width: u32, height: u32, text: &'static str) -> Self {
        Self {
            x,
            y,
            width,
            height,
            text,
            theme: GrayTheme::new(),
            pressed: false,
            id,
            enabled: true,
        }
    }

    /// 设置主题
    pub fn with_theme(mut self, theme: GrayTheme) -> Self {
        self.theme = theme;
        self
    }

    /// 获取边界框
    pub fn bounding_box(&self) -> BoundingBox {
        BoundingBox::new(self.x, self.y, self.width, self.height)
    }

    /// 检查点是否在按钮内
    pub fn contains(&self, x: i32, y: i32) -> bool {
        self.bounding_box().contains(x, y)
    }

    /// 绘制按钮（DMA 优化版本）
    ///
    /// 使用 fill_solid 进行大面积填充，适合 DMA 批量传输
    /// 极简线框风格：1px 边框，无阴影
    pub fn draw<D>(&self, display: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        let style = MonoTextStyle::new(&FONT_10X20, self.theme.text());

        // 绘制按钮主体 - 使用 fill_solid
        let btn_color = if !self.enabled {
            self.theme.disabled() // 浅灰 - 禁用状态
        } else if self.pressed {
            self.theme.pressed() // 最深灰 - 按下状态
        } else {
            self.theme.primary() // 中浅灰 - 普通按钮
        };

        display.fill_solid(
            &Rectangle::new(
                Point::new(self.x, self.y),
                Size::new(self.width, self.height),
            ),
            btn_color,
        )?;

        // 绘制 1px 边框 - 极简线框风格
        let border_color = if self.pressed {
            self.theme.g2 // 深灰 - 按下时的高亮边框
        } else {
            self.theme.border() // 黑色 - 普通边框
        };

        // 上边框
        display.fill_solid(
            &Rectangle::new(Point::new(self.x, self.y), Size::new(self.width, 1)),
            border_color,
        )?;
        // 下边框
        display.fill_solid(
            &Rectangle::new(
                Point::new(self.x, self.y + self.height as i32 - 1),
                Size::new(self.width, 1),
            ),
            self.theme.border(),
        )?;
        // 左边框
        display.fill_solid(
            &Rectangle::new(Point::new(self.x, self.y), Size::new(1, self.height)),
            border_color,
        )?;
        // 右边框
        display.fill_solid(
            &Rectangle::new(
                Point::new(self.x + self.width as i32 - 1, self.y),
                Size::new(1, self.height),
            ),
            self.theme.border(),
        )?;

        // 绘制文字（居中）
        let text = Text::with_baseline(
            self.text,
            Point::new(
                self.x + self.width as i32 / 2 - (self.text.len() * 6) as i32 / 2,
                self.y + self.height as i32 / 2 - 5,
            ),
            style,
            Baseline::Top,
        );
        text.draw(display)?;

        Ok(())
    }
}
