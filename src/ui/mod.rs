//! # 纯 Rust UI 框架 (DMA 优化版本)
//!
//! 轻量级嵌入式 UI 库，针对 DMA 传输优化。
//! 提供基础控件：按钮、标签、进度条等。
//!
//! ## DMA 优化特性
//! - 脏矩形跟踪，只重绘变化区域
//! - 批量 DMA 传输
//! - 控件使用 fill_solid 进行大面积填充

mod bounding_box;
mod icons;
mod screen;
mod sensor;
mod theme;
mod widgets;

// 重新导出所有公共类型
pub use bounding_box::BoundingBox;
pub use icons::PixelIcon;
pub use screen::Screen;
pub use sensor::{PressureSensor, TempHumidSensor};
pub use theme::GrayTheme;
pub use widgets::{Button, HistoryBar, Label, PressureCard, ProgressBar, TempHumidCard, Widget};
