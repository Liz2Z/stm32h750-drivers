//! # 纯 Rust UI 框架 (DMA 优化版本)
//!
//! 轻量级嵌入式 UI 库，针对 DMA 传输优化。
//! 提供基础控件：按钮、标签、进度条等。
//!
//! ## DMA 优化特性
//! - 脏矩形跟踪，只重绘变化区域
//! - 批量 DMA 传输
//! - 控件使用 fill_solid 进行大面积填充

use embedded_graphics::{
    mono_font::{ascii::FONT_10X20, MonoTextStyle},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::Rectangle,
    text::{Baseline, Text},
};

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

/// 8x8 像素图标
#[derive(Clone, Copy)]
pub enum PixelIcon {
    Thermo,   // 温度计 🌡
    Humid,    // 湿度计 💧
    Home,     // 主页 🏠
    Settings, // 设置 ⚙
}

impl PixelIcon {
    /// 获取 8x8 像素数据（每个 bit 代表一个像素）
    pub fn data(&self) -> &[u8; 8] {
        match self {
            Self::Thermo => &THERMO_ICON,
            Self::Humid => &HUMID_ICON,
            Self::Home => &HOME_ICON,
            Self::Settings => &SETTINGS_ICON,
        }
    }

    /// 绘制图标到显示设备
    ///
    /// x, y: 左上角坐标
    /// scale: 缩放倍数（1=原始8x8, 2=16x16）
    pub fn draw<D>(
        &self,
        display: &mut D,
        x: i32,
        y: i32,
        scale: u32,
        color: Rgb565,
    ) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        let data = self.data();
        let pixel_size = Size::new(scale, scale);

        for row in 0..8 {
            for col in 0..8 {
                if (data[row] >> (7 - col)) & 1 == 1 {
                    let px = x + col as i32 * scale as i32;
                    let py = y + row as i32 * scale as i32;
                    display.fill_solid(
                        &Rectangle::new(Point::new(px, py), pixel_size),
                        color,
                    )?;
                }
            }
        }
        Ok(())
    }
}

// 图标像素数据（8x8）
const THERMO_ICON: [u8; 8] = [
    0b00111100,  //   ██
    0b01000010,  //  ██
    0b01000010,  //  ██
    0b00111100,  //   ██
    0b00111100,  //   ██
    0b00100100,  //   ▐▌
    0b00100100,  //   ▐▌
    0b00100100,  //   ▐▌
];

const HUMID_ICON: [u8; 8] = [
    0b00100100,  //   ▐▌
    0b00100100,  //   ▐▌
    0b00100100,  //   ▐▌
    0b01111110,  //  █████
    0b11111111,  // ███████
    0b11111111,  // ███████
    0b11111111,  // ███████
    0b01101110,  //  ██ ██
];

const HOME_ICON: [u8; 8] = [
    0b00011000,  //    ██
    0b00111100,  //   ████
    0b01111110,  //  ██████
    0b11111111,  // ███████
    0b10011001,  // ██  ██
    0b10011001,  // ██  ██
    0b10011001,  // ██  ██
    0b10000001,  // ██    ██
];

const SETTINGS_ICON: [u8; 8] = [
    0b00100100,  //   ▐▌
    0b01111110,  //  █████
    0b11111111,  // ███████
    0b11111111,  // ███████
    0b00111100,  //   ████
    0b00100100,  //   ▐▌
    0b00100100,  //   ▐▌
    0b00100100,  //   ▐▌
];

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

/// 标签控件
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

/// UI 控件枚举（用于存储不同类型的控件）
pub enum Widget {
    Button(Button),
    Label(Label),
    ProgressBar(ProgressBar),
}

impl Widget {
    pub fn draw<D>(&self, display: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        match self {
            Widget::Button(b) => b.draw(display),
            Widget::Label(l) => l.draw(display),
            Widget::ProgressBar(p) => p.draw(display),
        }
    }

    /// 获取控件的边界框
    pub fn bounding_box(&self) -> BoundingBox {
        match self {
            Widget::Button(b) => b.bounding_box(),
            Widget::Label(l) => l.bounding_box(),
            Widget::ProgressBar(p) => p.bounding_box(),
        }
    }
}

/// 屏幕/容器
pub struct Screen {
    pub widgets: heapless::Vec<Widget, 8>,
    pub theme: GrayTheme,
    pub width: u32,
    pub height: u32,
    /// 脏矩形列表（需要重绘的区域）
    pub dirty_rects: heapless::Vec<BoundingBox, 8>,
}

impl Screen {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            widgets: heapless::Vec::new(),
            theme: GrayTheme::new(),
            width,
            height,
            dirty_rects: heapless::Vec::new(),
        }
    }

    pub fn with_theme(mut self, theme: GrayTheme) -> Self {
        self.theme = theme;
        self
    }

    pub fn add_button(&mut self, btn: Button) -> Result<(), ()> {
        self.widgets.push(Widget::Button(btn)).map_err(|_| ())
    }

    pub fn add_label(&mut self, label: Label) -> Result<(), ()> {
        self.widgets.push(Widget::Label(label)).map_err(|_| ())
    }

    pub fn add_progress(&mut self, progress: ProgressBar) -> Result<(), ()> {
        self.widgets
            .push(Widget::ProgressBar(progress))
            .map_err(|_| ())
    }

    /// 获取指定 ID 的进度条可变引用
    pub fn get_progress_bar(&mut self, id: u32) -> Option<&mut ProgressBar> {
        for widget in &mut self.widgets {
            if let Widget::ProgressBar(pb) = widget {
                if pb.id == id {
                    return Some(pb);
                }
            }
        }
        None
    }

    /// 标记整个屏幕为脏（需要全屏重绘）
    pub fn mark_full_dirty(&mut self) {
        self.dirty_rects.clear();
        let _ = self
            .dirty_rects
            .push(BoundingBox::new(0, 0, self.width, self.height));
    }

    /// 添加脏矩形
    pub fn add_dirty_rect(&mut self, rect: BoundingBox) {
        // 简单实现：最多 8 个脏矩形
        if self.dirty_rects.len() < 8 {
            let _ = self.dirty_rects.push(rect);
        }
    }

    /// 清除脏矩形
    pub fn clear_dirty(&mut self) {
        self.dirty_rects.clear();
    }

    /// 绘制整个屏幕（包括清屏）- DMA 优化版本
    pub fn draw<D>(&self, display: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        // 清屏 - 使用 fill_solid 进行 DMA 优化的大面积填充
        display.fill_solid(
            &Rectangle::new(Point::new(0, 0), Size::new(self.width, self.height)),
            self.theme.background(),
        )?;

        // 绘制所有控件
        for widget in &self.widgets {
            widget.draw(display)?;
        }

        Ok(())
    }

    /// 仅绘制脏矩形区域（DMA 局部更新）
    ///
    /// 用于只需要更新屏幕部分内容的情况
    pub fn draw_dirty<D>(&self, display: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        for dirty in &self.dirty_rects {
            // 绘制该区域的背景
            display.fill_solid(&dirty.to_rectangle(), self.theme.background())?;

            // 绘制与该区域相交的控件
            for widget in &self.widgets {
                let widget_box = widget.bounding_box();
                if Self::rects_intersect(dirty, &widget_box) {
                    widget.draw(display)?;
                }
            }
        }
        Ok(())
    }

    /// 检查两个矩形是否相交
    fn rects_intersect(a: &BoundingBox, b: &BoundingBox) -> bool {
        a.x < b.x + b.width as i32
            && a.x + a.width as i32 > b.x
            && a.y < b.y + b.height as i32
            && a.y + a.height as i32 > b.y
    }

    /// 直接获取指定 ID 的进度条引用并绘制（不经过 Screen）
    pub fn draw_progress_bar_only<D>(&self, display: &mut D, id: u32) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        for widget in &self.widgets {
            if let Widget::ProgressBar(pb) = widget {
                if pb.id == id {
                    return pb.draw(display);
                }
            }
        }
        Ok(())
    }

    /// 使用 DMA 绘制整个屏幕
    pub fn draw_with_dma(
        &self,
        display: &mut crate::display::DisplayDriver,
    ) -> Result<(), core::convert::Infallible> {
        // 清屏 - 使用帧缓冲
        display.clear(self.theme.background());

        // 绘制所有控件（绘制到帧缓冲）
        for widget in &self.widgets {
            widget.draw(display)?;
        }

        // 刷新到屏幕
        display.flush();

        Ok(())
    }

    /// 使用 DMA 只绘制指定 ID 的进度条
    pub fn draw_progress_bar_only_with_dma(
        &self,
        display: &mut crate::display::DisplayDriver,
        id: u32,
    ) -> Result<(), core::convert::Infallible> {
        for widget in &self.widgets {
            if let Widget::ProgressBar(pb) = widget {
                if pb.id == id {
                    return pb.draw(display);
                }
            }
        }
        Ok(())
    }

    /// 使用 DMA 局部更新进度条（优化版本）
    ///
    /// 只重绘进度条区域，不清屏
    pub fn update_progress_bar_with_dma(
        &self,
        display: &mut crate::display::DisplayDriver,
        id: u32,
    ) -> Result<(), core::convert::Infallible> {
        for widget in &self.widgets {
            if let Widget::ProgressBar(pb) = widget {
                if pb.id == id {
                    let bb = pb.bounding_box();

                    // 先清空区域（绘制背景）
                    embedded_graphics::primitives::Rectangle::new(
                        embedded_graphics::geometry::Point::new(bb.x, bb.y),
                        embedded_graphics::geometry::Size::new(bb.width, bb.height),
                    )
                    .into_styled(embedded_graphics::primitives::PrimitiveStyle::with_fill(
                        pb.theme.background(),
                    ))
                    .draw(display)?;

                    // 绘制进度条
                    pb.draw(display)?;

                    // 局部刷新
                    display.flush_rect(bb.x as u16, bb.y as u16, bb.width as u16, bb.height as u16);

                    return Ok(());
                }
            }
        }
        Ok(())
    }
}
