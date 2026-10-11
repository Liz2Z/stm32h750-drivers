//! 底栏声明自身的文字、颜色提示与计数输入，不引用连接业务状态。
use embedded_graphics::{pixelcolor::Rgb565, prelude::RgbColor};
use heapless::String;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusTone {
    Normal,
    Success,
    Failure,
    Muted,
}

impl StatusTone {
    fn color(self) -> Rgb565 {
        match self {
            Self::Normal => Rgb565::BLACK,
            Self::Success => Rgb565::new(0, 40, 0),
            Self::Failure => Rgb565::new(31, 8, 8),
            Self::Muted => Rgb565::new(14, 18, 20),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StatusLabel {
    pub text: &'static str,
    pub tone: StatusTone,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConnectionLabels {
    pub link: StatusLabel,
    pub upload: StatusLabel,
}

/// 无连接提示时，底栏显示采样次数和刷新率；否则显示连接、上传文字与次数。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StatusData {
    pub connection: Option<ConnectionLabels>,
    pub sample_count: u32,
    pub refresh_centihz: Option<u32>,
}

pub fn draw_status<D>(display: &mut D, data: StatusData)
where
    D: embedded_graphics::prelude::DrawTarget<Color = Rgb565>,
{
    use embedded_graphics::{
        mono_font::{ascii::FONT_6X10, MonoTextStyle},
        prelude::*,
        primitives::{PrimitiveStyle, Rectangle},
        text::{Baseline, Text},
    };

    let status_bg = Rgb565::new(26, 58, 74);
    let _ = Rectangle::new(Point::new(0, 220), Size::new(320, 20))
        .into_styled(PrimitiveStyle::with_fill(status_bg))
        .draw(display);

    let frame_style = MonoTextStyle::new(&FONT_6X10, Rgb565::BLACK);

    let Some(connection) = data.connection else {
        let mut text: String<24> = String::new();
        let _ = core::fmt::write(&mut text, format_args!("Samples: {}", data.sample_count));
        let _ = Text::with_baseline(
            text.as_str(),
            Point::new(4, 225),
            frame_style,
            Baseline::Top,
        )
        .draw(display);
        let mut rate_text: String<32> = String::new();
        if let Some(rate) = data.refresh_centihz {
            let _ = core::fmt::write(
                &mut rate_text,
                format_args!("Refresh: {}.{:02} Hz", rate / 100, rate % 100),
            );
        } else {
            let _ = rate_text.push_str("Refresh: -- Hz");
        }
        let _ = Text::with_baseline(
            rate_text.as_str(),
            Point::new(190, 225),
            frame_style,
            Baseline::Top,
        )
        .draw(display);
        return;
    };

    let wifi_style = MonoTextStyle::new(&FONT_6X10, connection.link.tone.color());
    let upload_style = MonoTextStyle::new(&FONT_6X10, connection.upload.tone.color());
    let mut frame_buf: String<24> = String::new();
    let _ = core::fmt::write(
        &mut frame_buf,
        format_args!("Samples: {}", data.sample_count),
    );

    let _ = Text::with_baseline(
        connection.link.text,
        Point::new(4, 225),
        wifi_style,
        Baseline::Top,
    )
    .draw(display);
    let _ = Text::with_baseline(
        connection.upload.text,
        Point::new(110, 225),
        upload_style,
        Baseline::Top,
    )
    .draw(display);
    let _ = Text::with_baseline(
        frame_buf.as_str(),
        Point::new(210, 225),
        frame_style,
        Baseline::Top,
    )
    .draw(display);
}
