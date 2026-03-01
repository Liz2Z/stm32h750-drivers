//! 温湿度传感器数据

/// 温湿度传感器数据
#[derive(Clone, Copy, Default)]
pub struct TempHumidSensor {
    /// 当前温度 (°C)
    pub temp_current: f32,
    /// 最高温度 (°C)
    pub temp_high: f32,
    /// 最低温度 (°C)
    pub temp_low: f32,
    /// 当前湿度 (%)
    pub humid_current: f32,
    /// 最高湿度 (%)
    pub humid_high: f32,
    /// 最低湿度 (%)
    pub humid_low: f32,
    /// 历史记录（最多 6 次）
    pub history: [f32; 6],
    /// 历史记录计数
    pub history_count: usize,
}

impl TempHumidSensor {
    /// 创建新的传感器数据
    pub fn new() -> Self {
        Self {
            temp_current: 0.0,
            temp_high: 0.0,
            temp_low: 0.0,
            humid_current: 0.0,
            humid_high: 0.0,
            humid_low: 0.0,
            history: [0.0; 6],
            history_count: 0,
        }
    }

    /// 更新温度数据
    pub fn update_temp(&mut self, temp: f32) {
        self.temp_current = temp;
        if temp > self.temp_high || self.temp_high == 0.0 {
            self.temp_high = temp;
        }
        if temp < self.temp_low || self.temp_low == 0.0 {
            self.temp_low = temp;
        }
        self.add_history(temp);
    }

    /// 更新湿度数据
    pub fn update_humid(&mut self, humid: f32) {
        self.humid_current = humid;
        if humid > self.humid_high || self.humid_high == 0.0 {
            self.humid_high = humid;
        }
        if humid < self.humid_low || self.humid_low == 0.0 {
            self.humid_low = humid;
        }
        self.add_history(humid);
    }

    /// 添加历史记录
    fn add_history(&mut self, value: f32) {
        if self.history_count < 6 {
            self.history[self.history_count] = value;
            self.history_count += 1;
        } else {
            // 滚动更新：移除最旧的，添加新的
            for i in 0..5 {
                self.history[i] = self.history[i + 1];
            }
            self.history[5] = value;
        }
    }

    /// 格式化温度字符串
    pub fn temp_str(&self) -> heapless::String<16> {
        use core::fmt::Write;
        let mut s = heapless::String::new();
        if write!(s, "{:.1}°C", self.temp_current).is_ok() {
            s
        } else {
            heapless::String::from("--.-°C")
        }
    }

    /// 格式化湿度字符串
    pub fn humid_str(&self) -> heapless::String<16> {
        use core::fmt::Write;
        let mut s = heapless::String::new();
        if write!(s, "{}%", self.humid_current as i32).is_ok() {
            s
        } else {
            heapless::String::from("--%")
        }
    }
}
