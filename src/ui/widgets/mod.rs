//! UI 控件：按钮、标签、进度条、温湿度卡片、气压卡片、历史记录条
//!
//! 注意：部分控件为预留功能，暂未在主程序中使用

mod button;
mod card;
mod history;
mod label;
mod pressure_card;
mod progress;

pub use button::Button;
pub use card::TempHumidCard;
pub use history::HistoryBar;
pub use label::Label;
pub use pressure_card::PressureCard;
pub use progress::ProgressBar;

use super::BoundingBox;
use embedded_graphics::{pixelcolor::Rgb565, prelude::*};

/// UI 控件枚举（用于存储不同类型的控件）
#[allow(dead_code)]
pub enum Widget {
    Button(Button),
    Label(Label),
    ProgressBar(ProgressBar),
    TempHumidCard(TempHumidCard),
    PressureCard(PressureCard),
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
            Widget::PressureCard(c) => c.draw(display),
            Widget::HistoryBar(h) => h.draw(display),
        }
    }

    /// 获取控件的边界框
    #[allow(dead_code)]
    pub fn bounding_box(&self) -> BoundingBox {
        match self {
            Widget::Button(b) => b.bounding_box(),
            Widget::Label(l) => l.bounding_box(),
            Widget::ProgressBar(p) => p.bounding_box(),
            Widget::TempHumidCard(c) => c.bounding_box(),
            Widget::PressureCard(c) => c.bounding_box(),
            Widget::HistoryBar(h) => h.bounding_box(),
        }
    }
}
