//! A small time-based animation above the sensor cards.
use super::{FlushTarget, GrayTheme};
use embedded_graphics::{
    prelude::*,
    primitives::{Circle, Line, PrimitiveStyle, Rectangle},
};

pub struct FlowAnimation {
    last_us: u32,
    phase_us: u32,
    theme: GrayTheme,
}

impl FlowAnimation {
    const PERIOD_US: u32 = 6_000_000;

    pub fn new(now: u32) -> Self {
        Self {
            last_us: now,
            phase_us: 0,
            theme: GrayTheme::new(),
        }
    }

    fn position(phase: u32) -> i32 {
        let half = Self::PERIOD_US / 2;
        let distance = if phase <= half {
            phase
        } else {
            Self::PERIOD_US - phase
        };
        13 + (distance * 294 / half) as i32
    }

    /// Advance by elapsed hardware time; late frames are skipped, never replayed.
    pub fn refresh<D: FlushTarget>(
        &mut self,
        display: &mut D,
        now: u32,
    ) -> Result<(), D::FlushError> {
        let elapsed = now.wrapping_sub(self.last_us) % Self::PERIOD_US;
        self.phase_us = (self.phase_us + elapsed) % Self::PERIOD_US;
        self.last_us = now;

        Rectangle::new(Point::new(5, 3), Size::new(310, 14))
            .into_styled(PrimitiveStyle::with_fill(self.theme.background()))
            .draw(display)?;
        Line::new(Point::new(13, 10), Point::new(307, 10))
            .into_styled(PrimitiveStyle::with_stroke(self.theme.g6, 1))
            .draw(display)?;
        // Delayed positions keep the trail inside the strip even at a turn.
        for i in (0..4).rev() {
            let phase = (self.phase_us + Self::PERIOD_US - i * 100_000) % Self::PERIOD_US;
            let radius = if i == 0 { 3 } else { 2 };
            let color = match i {
                0 => self.theme.text(),
                1 => self.theme.g2,
                2 => self.theme.g4,
                _ => self.theme.g5,
            };
            Circle::new(
                Point::new(Self::position(phase) - radius, 10 - radius),
                (radius * 2 + 1) as u32,
            )
            .into_styled(PrimitiveStyle::with_fill(color))
            .draw(display)?;
        }
        display.flush_rect(5, 3, 310, 14)
    }
}
