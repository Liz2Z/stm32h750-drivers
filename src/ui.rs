//! # 纯 Rust UI 框架
//!
//! 轻量级嵌入式 UI 库，不依赖 C 代码。
//! 提供基础控件：按钮、标签、进度条等。

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

    /// 检查点是否在按钮内
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < (self.x + self.width as i32)
            && y >= self.y && y < (self.y + self.height as i32)
    }

    /// 绘制按钮
    pub fn draw<D>(&self, display: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        let style = MonoTextStyle::new(&FONT_10X20, self.theme.text);

        // 绘制阴影
        if self.pressed {
            Rectangle::new(
                Point::new(self.x, self.y + 2),
                Size::new(self.width, self.height),
            )
            .into_styled(embedded_graphics::primitives::PrimitiveStyle::with_fill(
                self.theme.shadow,
            ))
            .draw(display)?;
        } else {
            Rectangle::new(
                Point::new(self.x + 2, self.y + 2),
                Size::new(self.width, self.height),
            )
            .into_styled(embedded_graphics::primitives::PrimitiveStyle::with_fill(
                self.theme.shadow,
            ))
            .draw(display)?;
        }

        // 绘制按钮主体
        let btn_color = if self.pressed || !self.enabled {
            self.theme.secondary
        } else {
            self.theme.primary
        };

        Rectangle::new(Point::new(self.x, self.y), Size::new(self.width, self.height))
            .into_styled(embedded_graphics::primitives::PrimitiveStyle::with_fill(
                btn_color,
            ))
            .draw(display)?;

        // 绘制边框
        let border_color = if self.pressed {
            self.theme.highlight
        } else {
            self.theme.border
        };

        // 上边框（高光）
        Rectangle::new(
            Point::new(self.x, self.y),
            Size::new(self.width, 2),
        )
        .into_styled(embedded_graphics::primitives::PrimitiveStyle::with_fill(
            border_color,
        ))
        .draw(display)?;

        // 下边框
        Rectangle::new(
            Point::new(self.x, self.y + self.height as i32 - 2),
            Size::new(self.width, 2),
        )
        .into_styled(embedded_graphics::primitives::PrimitiveStyle::with_fill(
            self.theme.border,
        ))
        .draw(display)?;

        // 左边框
        Rectangle::new(
            Point::new(self.x, self.y),
            Size::new(2, self.height),
        )
        .into_styled(embedded_graphics::primitives::PrimitiveStyle::with_fill(
            border_color,
        ))
        .draw(display)?;

        // 右边框
        Rectangle::new(
            Point::new(self.x + self.width as i32 - 2, self.y),
            Size::new(2, self.height),
        )
        .into_styled(embedded_graphics::primitives::PrimitiveStyle::with_fill(
            self.theme.border,
        ))
        .draw(display)?;

        // 绘制文字（居中）
        let text = Text::with_baseline(
            self.text,
            Point::new(
                self.x + self.width as i32 / 2
                    - (self.text.len() * 6) as i32 / 2,
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

    fn fill_width(&self) -> u32 {
        if self.max <= self.min {
            return 0;
        }
        let ratio = (self.value - self.min) as f32 / (self.max - self.min) as f32;
        (ratio.clamp(0.0, 1.0) * self.width as f32) as u32
    }

    pub fn draw<D>(&self, display: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        // 背景槽
        Rectangle::new(
            Point::new(self.x, self.y),
            Size::new(self.width, self.height),
        )
        .into_styled(embedded_graphics::primitives::PrimitiveStyle::with_fill(
            self.theme.background,
        ))
        .draw(display)?;

        // 边框
        Rectangle::new(
            Point::new(self.x, self.y),
            Size::new(self.width, self.height),
        )
        .into_styled(
            embedded_graphics::primitives::PrimitiveStyle::with_stroke(
                self.theme.border,
                1,
            ),
        )
        .draw(display)?;

        // 填充条
        let fill_w = self.fill_width();
        if fill_w > 2 {
            Rectangle::new(
                Point::new(self.x + 1, self.y + 1),
                Size::new((fill_w - 2).min(self.width - 2), self.height.saturating_sub(2)),
            )
            .into_styled(embedded_graphics::primitives::PrimitiveStyle::with_fill(
                self.theme.primary,
            ))
            .draw(display)?;
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
}

/// 屏幕/容器
pub struct Screen {
    pub widgets: heapless::Vec<Widget, 8>,
    pub theme: Theme,
    pub width: u32,
    pub height: u32,
}

impl Screen {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            widgets: heapless::Vec::new(),
            theme: Theme::dark(),
            width,
            height,
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

    /// 绘制整个屏幕（包括清屏）
    pub fn draw<D>(&self, display: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        // 清屏
        Rectangle::new(
            Point::new(0, 0),
            Size::new(self.width, self.height),
        )
        .into_styled(embedded_graphics::primitives::PrimitiveStyle::with_fill(
            self.theme.background,
        ))
        .draw(display)?;

        // 绘制所有控件
        for widget in &self.widgets {
            widget.draw(display)?;
        }

        Ok(())
    }

    /// 只绘制指定类型的控件（用于局部更新）
    pub fn draw_widgets_by_type<D>(&self, display: &mut D, widget_type: WidgetType) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        for widget in &self.widgets {
            let should_draw = match (widget_type, widget) {
                (WidgetType::Button, Widget::Button(_)) => true,
                (WidgetType::Label, Widget::Label(_)) => true,
                (WidgetType::ProgressBar, Widget::ProgressBar(_)) => true,
                _ => false,
            };
            if should_draw {
                widget.draw(display)?;
            }
        }
        Ok(())
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
}

/// 控件类型枚举
#[derive(Clone, Copy, Debug)]
pub enum WidgetType {
    Button,
    Label,
    ProgressBar,
}
