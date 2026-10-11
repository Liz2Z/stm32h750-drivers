//! 核心板用户键：K1=PE3、K2=PC5，按下接地，需要内部上拉；RST 不属于 GPIO。
use stm32h7xx_hal::gpio::{Input, PC5, PE3};

pub struct Buttons {
    k1: PE3<Input>,
    k2: PC5<Input>,
}

impl Buttons {
    pub fn new(k1: PE3<Input>, k2: PC5<Input>) -> Self {
        Self { k1, k2 }
    }

    /// 返回原始按下电平，按键消抖与动作映射交给应用。
    pub fn pressed(&self) -> (bool, bool) {
        (self.k1.is_low(), self.k2.is_low())
    }
}
