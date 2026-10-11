//! # 纯 Rust 帧缓冲 UI 框架
//!
//! 轻量级嵌入式 UI 库，将绘制与硬件传输分离。
//! 提供基础控件：按钮、标签、进度条等。
//!
//! ## 刷新优化特性
//! - 脏矩形跟踪，只重绘变化区域
//! - 通过 FlushTarget 进行批量传输
//! - 控件使用 fill_solid 进行大面积填充

mod bounding_box;
mod flow_animation;
mod icons;
mod screen;
pub(crate) mod sensor_format;
pub mod status;
mod theme;
mod widgets;

// 重新导出所有公共类型
pub use bounding_box::BoundingBox;
pub use flow_animation::FlowAnimation;
pub use icons::PixelIcon;
pub use screen::{FlushTarget, Screen};
pub use theme::GrayTheme;
pub use widgets::{
    Button, HistoryBar, Label, PressureCard, PressureCardData, ProgressBar, TempHumidCard,
    TempHumidCardData, ValueRange, ValueStatus, Widget,
};
