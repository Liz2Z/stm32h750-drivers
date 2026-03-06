//! 屏幕/容器

use super::{BoundingBox, GrayTheme, ProgressBar, Widget};
use embedded_graphics::{pixelcolor::Rgb565, prelude::*};

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

    pub fn add_button(&mut self, btn: super::Button) -> Result<(), ()> {
        self.widgets.push(Widget::Button(btn)).map_err(|_| ())
    }

    pub fn add_label(&mut self, label: super::Label) -> Result<(), ()> {
        self.widgets.push(Widget::Label(label)).map_err(|_| ())
    }

    pub fn add_progress(&mut self, progress: ProgressBar) -> Result<(), ()> {
        self.widgets
            .push(Widget::ProgressBar(progress))
            .map_err(|_| ())
    }

    pub fn add_temp_humd_card(&mut self, card: super::TempHumidCard) -> Result<(), ()> {
        self.widgets.push(Widget::TempHumidCard(card)).map_err(|_| ())
    }

    pub fn add_pressure_card(&mut self, card: super::PressureCard) -> Result<(), ()> {
        self.widgets.push(Widget::PressureCard(card)).map_err(|_| ())
    }

    pub fn add_history_bar(&mut self, bar: super::HistoryBar) -> Result<(), ()> {
        self.widgets.push(Widget::HistoryBar(bar)).map_err(|_| ())
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
            &embedded_graphics::primitives::Rectangle::new(
                embedded_graphics::geometry::Point::new(0, 0),
                embedded_graphics::geometry::Size::new(self.width, self.height),
            ),
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
