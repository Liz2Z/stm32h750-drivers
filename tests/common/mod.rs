//! 集成测试共用的模拟屏幕和三卡片布局。
use embedded_graphics::{pixelcolor::Rgb565, prelude::*};
use std::{vec, vec::Vec};
use stm32h750_drivers::ui::{FlushTarget, PressureCard, Screen, TempHumidCard};

#[derive(Debug)]
pub enum FlushError {
    Failed,
}
impl From<core::convert::Infallible> for FlushError {
    fn from(e: core::convert::Infallible) -> Self {
        match e {}
    }
}
pub struct Panel {
    pub framebuffer: Vec<Rgb565>,
    pub visible: Vec<Rgb565>,
    pub windows: Vec<(u16, u16, u16, u16)>,
    pub fail: bool,
}
impl Panel {
    pub fn new() -> Self {
        Self {
            framebuffer: vec![Rgb565::BLACK; 320 * 240],
            visible: vec![Rgb565::BLACK; 320 * 240],
            windows: Vec::new(),
            fail: false,
        }
    }
}
impl OriginDimensions for Panel {
    fn size(&self) -> Size {
        Size::new(320, 240)
    }
}
impl DrawTarget for Panel {
    type Color = Rgb565;
    type Error = core::convert::Infallible;
    fn draw_iter<I: IntoIterator<Item = Pixel<Rgb565>>>(
        &mut self,
        pixels: I,
    ) -> Result<(), Self::Error> {
        for Pixel(p, c) in pixels {
            if p.x >= 0 && p.x < 320 && p.y >= 0 && p.y < 240 {
                self.framebuffer[p.y as usize * 320 + p.x as usize] = c;
            }
        }
        Ok(())
    }
}
impl FlushTarget for Panel {
    type FlushError = FlushError;
    fn flush(&mut self) -> Result<(), FlushError> {
        self.flush_rect(0, 0, 320, 240)
    }
    fn flush_rect(&mut self, x: u16, y: u16, w: u16, h: u16) -> Result<(), FlushError> {
        if self.fail {
            self.fail = false;
            return Err(FlushError::Failed);
        }
        self.windows.push((x, y, w, h));
        for row in y..y + h {
            let start = row as usize * 320 + x as usize;
            self.visible[start..start + w as usize]
                .copy_from_slice(&self.framebuffer[start..start + w as usize]);
        }
        Ok(())
    }
}
pub fn screen() -> Screen {
    let mut screen = Screen::new(320, 240);
    screen
        .add_temp_humd_card(TempHumidCard::new(5, 20, true))
        .unwrap();
    screen
        .add_temp_humd_card(TempHumidCard::new(110, 20, false))
        .unwrap();
    screen
        .add_pressure_card(PressureCard::new(215, 20))
        .unwrap();
    screen
}
