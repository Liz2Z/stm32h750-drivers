//! 灰度主题（8 级灰度）

use embedded_graphics::pixelcolor::Rgb565;

/// 灰度主题（8 级灰度）
#[derive(Clone, Copy)]
pub struct GrayTheme {
    /// g0 - 黑色 - 线条、文字
    pub g0: Rgb565,
    /// g1 - 最深灰 - 按下按钮
    pub g1: Rgb565,
    /// g2 - 深灰 - 进度条填充
    pub g2: Rgb565,
    /// g3 - 中深灰 - 边框阴影
    pub g3: Rgb565,
    /// g4 - 中浅灰 - 普通按钮
    pub g4: Rgb565,
    /// g5 - 浅灰 - 禁用状态、扫描线
    pub g5: Rgb565,
    /// g6 - 最浅灰 - 辅助背景
    pub g6: Rgb565,
    /// g7 - 白色 - 主背景
    pub g7: Rgb565,
}

impl Default for GrayTheme {
    fn default() -> Self {
        Self::new()
    }
}

impl GrayTheme {
    /// 创建默认灰度主题
    pub fn new() -> Self {
        Self {
            g0: Rgb565::new(0, 0, 0),       // 黑色
            g1: Rgb565::new(40, 40, 40),   // 最深灰
            g2: Rgb565::new(80, 80, 80),   // 深灰
            g3: Rgb565::new(120, 120, 120), // 中深灰
            g4: Rgb565::new(160, 160, 160), // 中浅灰
            g5: Rgb565::new(200, 200, 200), // 浅灰
            g6: Rgb565::new(230, 230, 230), // 最浅灰
            g7: Rgb565::new(255, 255, 255), // 白色
        }
    }

    /// 获取文字颜色（黑色）
    pub fn text(&self) -> Rgb565 {
        self.g0
    }

    /// 获取边框颜色（黑色）
    pub fn border(&self) -> Rgb565 {
        self.g0
    }

    /// 获取背景颜色（白色）
    pub fn background(&self) -> Rgb565 {
        self.g7
    }

    /// 获取主填充颜色（中浅灰）
    pub fn primary(&self) -> Rgb565 {
        self.g4
    }

    /// 获取按下状态颜色（最深灰）
    pub fn pressed(&self) -> Rgb565 {
        self.g1
    }

    /// 获取禁用状态颜色（浅灰）
    pub fn disabled(&self) -> Rgb565 {
        self.g5
    }
}
