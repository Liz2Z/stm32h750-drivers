use stm32h7xx_hal::{pac::TIM2, timer::Timer};

/// TIM2 is configured at 1 MHz. Its 32-bit counter wraps every ~71 minutes.
pub struct Clock(pub Timer<TIM2>);

impl Clock {
    pub fn now_us(&self) -> u32 {
        self.0.counter()
    }

    pub fn expired(&self, start: u32, timeout_us: u32) -> bool {
        self.now_us().wrapping_sub(start) >= timeout_us
    }

    pub fn delay_ms(&self, ms: u32) {
        // Chunking also avoids overflowing the microsecond conversion.
        for _ in 0..ms {
            let start = self.now_us();
            while !self.expired(start, 1_000) {
                core::hint::spin_loop();
            }
        }
    }
}

/// 软件延时函数
///
/// 用于现有的系统启动和 ESP8266 阻塞流程。
/// STM32H750 主频 400MHz，1ms = 400,000 个 CPU 周期
/// 启动与 ESP 代码沿用周期延时；传感器和调度使用 TIM2。
pub fn delay_ms(ms: u32) {
    cortex_m::asm::delay(ms * 400_000);
}
