//! UI 控件：按钮、标签、进度条、温湿度卡片、历史记录条

use super::{BoundingBox, GrayTheme, PixelIcon, TempHumidSensor};
use embedded_graphics::{
    mono_font::{ascii::FONT_10X20, MonoTextStyle},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::Rectangle,
    text::{Baseline, Text},
};

/// 温湿度显示卡片（130x120）
pub struct TempHumidCard {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub sensor: TempHumidSensor,
    pub theme: GrayTheme,
    pub show_temp: bool, // true=显示温度, false=显示湿度
}

impl TempHumidCard {
    /// 创建新的温湿度卡片
    pub fn new(x: i32, y: i32, show_temp: bool) -> Self {
        Self {
            x,
            y,
            width: 130,
            height: 120,
            sensor: TempHumidSensor::new(),
            theme: GrayTheme::new(),
            show_temp,
        }
    }

    /// 设置主题
    pub fn with_theme(mut self, theme: GrayTheme) -> Self {
        self.theme = theme;
        self
    }

    /// 更新传感器数据
    pub fn update(&mut self, data: TempHumidSensor) {
        self.sensor = data;
    }

    /// 获取边界框
    pub fn bounding_box(&self) -> BoundingBox {
        BoundingBox::new(self.x, self.y, self.width, self.height)
    }

    /// 绘制卡片
    pub fn draw<D>(&self, display: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        // 1. 绘制卡片背景（白色）
        display.fill_solid(
            &Rectangle::new(
                Point::new(self.x, self.y),
                Size::new(self.width, self.height),
            ),
            self.theme.g7,
        )?;

        // 2. 绘制黑色边框
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
        let title = if self.show_temp { "THERMO" } else { "HUMID" };
        let style = MonoTextStyle::new(&FONT_10X20, self.theme.g0);
        let text = Text::with_baseline(
            title,
            Point::new(
                self.x + self.width as i32 / 2 - title.len() as i32 * 5,
                self.y + 5,
            ),
            style,
            Baseline::Top,
        );
        text.draw(display)?;

        // 4. 绘制图标（居中，16x16 放大）
        let icon = if self.show_temp {
            PixelIcon::Thermo
        } else {
            PixelIcon::Humid
        };
        let icon_x = self.x + self.width as i32 / 2 - 8;
        let icon_y = self.y + 30;
        icon.draw(display, icon_x, icon_y, 2, self.theme.g0)?;

        // 5. 绘制当前数值
        let value_str = if self.show_temp {
            self.sensor.temp_str()
        } else {
            self.sensor.humid_str()
        };
        let value_text = Text::with_baseline(
            &value_str,
            Point::new(
                self.x + self.width as i32 / 2 - value_str.len() as i32 * 5,
                self.y + 60,
            ),
            style,
            Baseline::Top,
        );
        value_text.draw(display)?;

        // 6. 绘制最高/最低值
        use core::fmt::Write;
        let (high, low) = if self.show_temp {
            (
                {
                    let mut s = heapless::String::<16>::new();
                    let _ = write!(s, "hi:{:.0}°C", self.sensor.temp_high);
                    s
                },
                {
                    let mut s = heapless::String::<16>::new();
                    let _ = write!(s, "lo:{:.0}°C", self.sensor.temp_low);
                    s
                },
            )
        } else {
            (
                {
                    let mut s = heapless::String::<16>::new();
                    let _ = write!(s, "hi:{}%", self.sensor.humid_high as i32);
                    s
                },
                {
                    let mut s = heapless::String::<16>::new();
                    let _ = write!(s, "lo:{}%", self.sensor.humid_low as i32);
                    s
                },
            )
        };

        let high_text = Text::with_baseline(
            &high,
            Point::new(self.x + 10, self.y + 90),
            style,
            Baseline::Top,
        );
        high_text.draw(display)?;

        let low_text = Text::with_baseline(
            &low,
            Point::new(self.x + 70, self.y + 90),
            style,
            Baseline::Top,
        );
        low_text.draw(display)?;

        Ok(())
    }
}

/// 历史记录条（280x60）
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
        for i in 0..len {
            self.data[i] = data[i];
        }
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
    TempHumidCard(TempHumidCard),
    HistoryBar(HistoryBar),
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
            Widget::TempHumidCard(c) => c.draw(display),
            Widget::HistoryBar(h) => h.draw(display),
        }
    }

    /// 获取控件的边界框
    pub fn bounding_box(&self) -> BoundingBox {
        match self {
            Widget::Button(b) => b.bounding_box(),
            Widget::Label(l) => l.bounding_box(),
            Widget::ProgressBar(p) => p.bounding_box(),
            Widget::TempHumidCard(c) => c.bounding_box(),
            Widget::HistoryBar(h) => h.bounding_box(),
        }
    }
}
