//! 淡蓝色主题
//!
//! 注意：此主题为预留功能，暂未在主程序中使用

use embedded_graphics::pixelcolor::Rgb565;

/// 淡蓝色主题
#[derive(Clone, Copy)]
pub struct LightBlueTheme {
    /// g0 - 深海蓝 - 线条、文字
    pub g0: Rgb565,
    /// g1 - 深蓝 - 按下按钮
    pub g1: Rgb565,
    /// g2 - 中深蓝 - 进度条填充
    pub g2: Rgb565,
    /// g3 - 中蓝 - 边框阴影
    #[allow(dead_code)]
    pub g3: Rgb565,
    /// g4 - 中浅蓝 - 普通按钮
    pub g4: Rgb565,
    /// g5 - 浅蓝 - 禁用状态、扫描线
    pub g5: Rgb565,
    /// g6 - 淡蓝 - 辅助背景
    pub g6: Rgb565,
    /// g7 - 天蓝白 - 主背景
    pub g7: Rgb565,
}

impl Default for LightBlueTheme {
    fn default() -> Self {
        Self::new()
    }
}

impl LightBlueTheme {
    /// 创建默认淡蓝色主题
    pub fn new() -> Self {
        Self {
            g0: Rgb565::new(2, 10, 24),  // 深海蓝 #1A3C66
            g1: Rgb565::new(4, 16, 34),  // 深蓝 #2B5B8C
            g2: Rgb565::new(6, 22, 44),  // 中深蓝 #3D7AB2
            g3: Rgb565::new(9, 30, 52),  // 中蓝 #4F8CD4
            g4: Rgb565::new(12, 38, 58), // 中浅蓝 #6BA3E0
            g5: Rgb565::new(16, 48, 68), // 浅蓝 #8DBAE8
            g6: Rgb565::new(22, 58, 76), // 淡蓝 #B3D4F0
            g7: Rgb565::new(28, 62, 80), // 天蓝白 #E0F0FF
        }
    }

    /// 获取文字颜色（深海蓝）
    pub fn text(&self) -> Rgb565 {
        self.g0
    }

    /// 获取边框颜色（深海蓝）
    pub fn border(&self) -> Rgb565 {
        self.g0
    }

    /// 获取背景颜色（天蓝白）
    pub fn background(&self) -> Rgb565 {
        self.g7
    }

    /// 获取主填充颜色（中浅蓝）
    pub fn primary(&self) -> Rgb565 {
        self.g4
    }

    /// 获取按下状态颜色（深蓝）
    pub fn pressed(&self) -> Rgb565 {
        self.g1
    }

    /// 获取禁用状态颜色（浅蓝）
    pub fn disabled(&self) -> Rgb565 {
        self.g5
    }
}

/// 类型别名，保持兼容性
pub type GrayTheme = LightBlueTheme;
