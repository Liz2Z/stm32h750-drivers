//! 独立读卡页面：只接收格式化文字和提示色，不引用读卡应用或驱动。
use super::{status::StatusMessage, GrayTheme};
use embedded_graphics::{
    mono_font::{
        ascii::{FONT_10X20, FONT_6X10},
        MonoTextStyle,
    },
    pixelcolor::Rgb565,
    prelude::*,
    primitives::Rectangle,
    text::{Alignment, Baseline, Text, TextStyleBuilder},
};

/// uid 为最多 29 个 ASCII 字符的十六进制文字（最长 10 字节 UID）。
pub struct RfidPageData<'a> {
    pub status: StatusMessage<'a>,
    pub uid: &'a str,
    pub uid_caption: &'a str,
}

/// 重画 y=20..219 的页面主体，保留顶部动画和底栏；调用方负责刷新该区域。
pub fn draw_rfid_page<D: DrawTarget<Color = Rgb565>>(
    display: &mut D,
    data: RfidPageData<'_>,
) -> Result<(), D::Error> {
    let theme = GrayTheme::new();
    display.fill_solid(
        &Rectangle::new(Point::new(0, 20), Size::new(320, 200)),
        theme.background(),
    )?;
    let centered = TextStyleBuilder::new()
        .alignment(Alignment::Center)
        .baseline(Baseline::Top)
        .build();
    let normal = MonoTextStyle::new(&FONT_6X10, theme.text());
    let large = MonoTextStyle::new(&FONT_10X20, theme.text());
    Text::with_text_style("RFID / RC522", Point::new(160, 32), large, centered).draw(display)?;
    Text::with_text_style(
        data.status.text,
        Point::new(160, 68),
        MonoTextStyle::new(&FONT_6X10, data.status.tone.color()),
        centered,
    )
    .draw(display)?;
    Text::with_text_style(data.uid_caption, Point::new(160, 99), normal, centered).draw(display)?;
    Text::with_text_style(data.uid, Point::new(160, 120), large, centered).draw(display)?;
    Text::with_text_style(
        "Hold one card near the antenna",
        Point::new(160, 170),
        normal,
        centered,
    )
    .draw(display)?;
    Text::with_text_style(
        "13.56 MHz / ISO14443A",
        Point::new(160, 189),
        normal,
        centered,
    )
    .draw(display)?;
    Ok(())
}
