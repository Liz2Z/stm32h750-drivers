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
    pixelcolor::Rgb565,
    prelude::*,
    primitives::Rectangle,
    mono_font::{ascii::FONT_10X20, MonoTextStyle},
    text::{Text, Baseline},
};

/// 颜色主题
#[derive(Clone, Copy)]
pub struct Theme {
    /// 背景色
    pub background: Rgb565,
    /// 主要颜色（按钮等）
    pub primary: Rgb565,
    /// 次要颜色
    pub secondary: Rgb565,
    /// 文字颜色
    pub text: Rgb565,
    /// 边框颜色
    pub border: Rgb565,
    /// 阴影颜色
    pub shadow: Rgb565,
    /// 高亮颜色
    pub highlight: Rgb565,
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

impl Theme {
    /// 深色主题
    pub fn dark() -> Self {
        Self {
            background: Rgb565::new(20, 20, 25),
            primary: Rgb565::new(0, 76, 255),
            secondary: Rgb565::new(60, 60, 80),
            text: Rgb565::WHITE,
            border: Rgb565::new(40, 40, 50),
            shadow: Rgb565::new(10, 10, 15),
            highlight: Rgb565::new(0, 122, 255),
        }
    }

    /// 浅色主题
    pub fn light() -> Self {
        Self {
            background: Rgb565::new(240, 240, 245),
            primary: Rgb565::new(0, 122, 255),
            secondary: Rgb565::new(200, 200, 210),
            text: Rgb565::BLACK,
            border: Rgb565::new(180, 180, 190),
            shadow: Rgb565::new(200, 200, 210),
            highlight: Rgb565::new(0, 150, 255),
        }
    }
}

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
        Self { x, y, width, height }
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
    pub theme: Theme,
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
            theme: Theme::dark(),
            pressed: false,
            id,
            enabled: true,
        }
    }

    /// 设置主题
    pub fn with_theme(mut self, theme: Theme) -> Self {
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
    pub fn draw<D>(&self, display: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        let style = MonoTextStyle::new(&FONT_10X20, self.theme.text);

        // 绘制阴影 - 使用 fill_solid 进行大面积填充（DMA 友好）
        if self.pressed {
            display.fill_solid(
                &Rectangle::new(
                    Point::new(self.x, self.y + 2),
                    Size::new(self.width, self.height),
                ),
                self.theme.shadow,
            )?;
        } else {
            display.fill_solid(
                &Rectangle::new(
                    Point::new(self.x + 2, self.y + 2),
                    Size::new(self.width, self.height),
                ),
                self.theme.shadow,
            )?;
        }

        // 绘制按钮主体 - 使用 fill_solid
        let btn_color = if self.pressed || !self.enabled {
            self.theme.secondary
        } else {
            self.theme.primary
        };

        display.fill_solid(
            &Rectangle::new(Point::new(self.x, self.y), Size::new(self.width, self.height)),
            btn_color,
        )?;

        // 绘制边框 - 简化为细线边框，减少 DMA 传输次数
        let border_color = if self.pressed {
            self.theme.highlight
        } else {
            self.theme.border
        };

        // 合并边框绘制 - 上下左右作为一个区域
        // 上边框
        display.fill_solid(
            &Rectangle::new(Point::new(self.x, self.y), Size::new(self.width, 2)),
            border_color,
        )?;
        // 下边框
        display.fill_solid(
            &Rectangle::new(
                Point::new(self.x, self.y + self.height as i32 - 2),
                Size::new(self.width, 2),
            ),
            self.theme.border,
        )?;
        // 左边框
        display.fill_solid(
            &Rectangle::new(Point::new(self.x, self.y), Size::new(2, self.height)),
            border_color,
        )?;
        // 右边框
        display.fill_solid(
            &Rectangle::new(
                Point::new(self.x + self.width as i32 - 2, self.y),
                Size::new(2, self.height),
            ),
            self.theme.border,
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
    pub theme: Theme,
    pub centered: bool,
}

impl Label {
    pub fn new(x: i32, y: i32, text: &'static str) -> Self {
        Self {
            x,
            y,
            text,
            theme: Theme::dark(),
            centered: false,
        }
    }

    pub fn with_theme(mut self, theme: Theme) -> Self {
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
        let style = MonoTextStyle::new(&FONT_10X20, self.theme.text);

        let text = if self.centered {
            Text::with_baseline(
                self.text,
                Point::new(
                    self.x - (self.text.len() * 6) as i32 / 2,
                    self.y,
                ),
                style,
                Baseline::Top,
            )
        } else {
            Text::with_baseline(
                self.text,
                Point::new(self.x, self.y),
                style,
                Baseline::Top,
            )
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
    pub theme: Theme,
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
            theme: Theme::dark(),
            id,
            last_fill_width: 0,
        }
    }

    pub fn with_theme(mut self, theme: Theme) -> Self {
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
            self.theme.background,
        )?;

        // 边框 - 简化为填充
        display.fill_solid(
            &Rectangle::new(
                Point::new(self.x, self.y),
                Size::new(self.width, 1),
            ),
            self.theme.border,
        )?;
        display.fill_solid(
            &Rectangle::new(
                Point::new(self.x, self.y + self.height as i32 - 1),
                Size::new(self.width, 1),
            ),
            self.theme.border,
        )?;
        display.fill_solid(
            &Rectangle::new(
                Point::new(self.x, self.y),
                Size::new(1, self.height),
            ),
            self.theme.border,
        )?;
        display.fill_solid(
            &Rectangle::new(
                Point::new(self.x + self.width as i32 - 1, self.y),
                Size::new(1, self.height),
            ),
            self.theme.border,
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
                self.theme.primary,
            )?;
        }

        Ok(())
    }

    /// 仅绘制变化部分（DMA 局部更新）
    ///
    /// 如果进度条值变化，只重绘填充区域
    pub fn draw_incremental<D>(&self, display: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        let current_fill = self.fill_width();

        if current_fill == self.last_fill_width {
            return Ok(()); // 无变化，跳过
        }

        // 绘制整个进度条（简化实现）
        self.draw(display)
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
    pub theme: Theme,
    pub width: u32,
    pub height: u32,
    /// 脏矩形列表（需要重绘的区域）
    pub dirty_rects: heapless::Vec<BoundingBox, 8>,
}

impl Screen {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            widgets: heapless::Vec::new(),
            theme: Theme::dark(),
            width,
            height,
            dirty_rects: heapless::Vec::new(),
        }
    }

    pub fn with_theme(mut self, theme: Theme) -> Self {
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
        self.widgets.push(Widget::ProgressBar(progress)).map_err(|_| ())
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
        let _ = self.dirty_rects.push(BoundingBox::new(
            0,
            0,
            self.width,
            self.height,
        ));
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
            self.theme.background,
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
            display.fill_solid(&dirty.to_rectangle(), self.theme.background)?;

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
        display.clear(self.theme.background);

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
                        pb.theme.background,
                    ))
                    .draw(display)?;

                    // 绘制进度条
                    pb.draw(display)?;

                    // 局部刷新
                    display.flush_rect(
                        bb.x as u16,
                        bb.y as u16,
                        bb.width as u16,
                        bb.height as u16,
                    );

                    return Ok(());
                }
            }
        }
        Ok(())
    }
}
