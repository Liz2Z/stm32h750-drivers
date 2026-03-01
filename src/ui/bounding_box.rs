//! 边界框结构

use embedded_graphics::{prelude::*, primitives::Rectangle};

/// 边界框结构
#[derive(Clone, Copy, Debug)]
pub struct BoundingBox {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl BoundingBox {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// 转换为 Rectangle
    pub fn to_rectangle(&self) -> Rectangle {
        Rectangle::new(
            Point::new(self.x, self.y),
            Size::new(self.width, self.height),
        )
    }

    /// 检查点是否在框内
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x
            && x < (self.x + self.width as i32)
            && y >= self.y
            && y < (self.y + self.height as i32)
    }
}
