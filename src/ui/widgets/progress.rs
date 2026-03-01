//! 进度条控件

use super::BoundingBox;
use crate::ui::GrayTheme;
use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::*,
    primitives::Rectangle,
};

/// 进度条控件
pub struct ProgressBar {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub value: i32,
    pub min: i32,
    pub max: i32,
    pub theme: GrayTheme,
    pub id: u32,
    /// 上次绘制的填充宽度（用于脏矩形检测）
    pub last_fill_width: u32,
}

impl ProgressBar {
    pub fn new(id: u32, x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
            value: 0,
            min: 0,
            max: 100,
            theme: GrayTheme::new(),
            id,
            last_fill_width: 0,
        }
    }

    pub fn with_theme(mut self, theme: GrayTheme) -> Self {
        self.theme = theme;
        self
    }

    pub fn with_range(mut self, min: i32, max: i32) -> Self {
        self.min = min;
        self.max = max;
        self
    }

    /// 设置进度值（会被限制在 min-max 范围内）
    pub fn set_value(&mut self, value: i32) {
        self.value = value.clamp(self.min, self.max);
    }

    /// 获取当前填充宽度
    pub fn fill_width(&self) -> u32 {
        if self.max <= self.min {
            return 0;
        }
        let ratio = (self.value - self.min) as f32 / (self.max - self.min) as f32;
        (ratio.clamp(0.0, 1.0) * self.width as f32) as u32
    }

    /// 获取边界框
    pub fn bounding_box(&self) -> BoundingBox {
        BoundingBox::new(self.x, self.y, self.width, self.height)
    }

    /// 获取脏矩形（变化区域）
    ///
    /// 返回需要重绘的区域，用于 DMA 局部更新
    pub fn dirty_rect(&self) -> Option<BoundingBox> {
        let current_fill = self.fill_width();
        if current_fill == self.last_fill_width {
            return None; // 无变化
        }

        // 返回整个进度条区域（简化处理）
        Some(self.bounding_box())
    }

    /// 标记为已绘制
    pub fn mark_drawn(&mut self) {
        self.last_fill_width = self.fill_width();
    }

    /// 绘制进度条（DMA 优化版本）
    ///
    /// 使用 fill_solid 进行大面积填充
    pub fn draw<D>(&self, display: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        // 背景槽 - 使用 fill_solid
        display.fill_solid(
            &Rectangle::new(
                Point::new(self.x, self.y),
                Size::new(self.width, self.height),
            ),
            self.theme.background(),
        )?;

        // 边框 - 简化为填充
        display.fill_solid(
            &Rectangle::new(Point::new(self.x, self.y), Size::new(self.width, 1)),
            self.theme.border(),
        )?;
        display.fill_solid(
            &Rectangle::new(
                Point::new(self.x, self.y + self.height as i32 - 1),
                Size::new(self.width, 1),
            ),
            self.theme.border(),
        )?;
        display.fill_solid(
            &Rectangle::new(Point::new(self.x, self.y), Size::new(1, self.height)),
            self.theme.border(),
        )?;
        display.fill_solid(
            &Rectangle::new(
                Point::new(self.x + self.width as i32 - 1, self.y),
                Size::new(1, self.height),
            ),
            self.theme.border(),
        )?;

        // 填充条 - 使用 fill_solid（DMA 友好）
        let fill_w = self.fill_width();
        if fill_w > 2 {
            display.fill_solid(
                &Rectangle::new(
                    Point::new(self.x + 1, self.y + 1),
                    Size::new(
                        (fill_w - 2).min(self.width - 2),
                        self.height.saturating_sub(2),
                    ),
                ),
                self.theme.g2, // 深灰 - 进度条填充
            )?;
        }

        Ok(())
    }
}
