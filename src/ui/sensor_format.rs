//! 格式化控件输入，不访问采样记录。
use super::{ValueRange, ValueStatus};

pub(crate) fn state_label(state: ValueStatus) -> &'static str {
    match state {
        ValueStatus::Waiting => "WAIT",
        ValueStatus::Normal => "",
        ValueStatus::Stale => "STALE",
        ValueStatus::Error => "ERROR",
    }
}

/// 格式化温度字符串
pub(crate) fn temperature(values: Option<ValueRange>) -> heapless::String<16> {
    let Some(values) = values else {
        return heapless::String::from("--.-C");
    };
    use core::fmt::Write;
    let mut s = heapless::String::new();
    if write!(s, "{:.1}C", values.current).is_ok() {
        s
    } else {
        heapless::String::from("--.-C")
    }
}

/// 格式化湿度字符串
pub(crate) fn humidity(values: Option<ValueRange>) -> heapless::String<16> {
    let Some(values) = values else {
        return heapless::String::from("--%");
    };
    use core::fmt::Write;
    let mut s = heapless::String::new();
    if write!(s, "{}%", values.current as i32).is_ok() {
        s
    } else {
        heapless::String::from("--%")
    }
}

/// 格式化气压字符串
pub(crate) fn pressure(values: Option<ValueRange>) -> heapless::String<16> {
    let Some(values) = values else {
        return heapless::String::from("----hPa");
    };
    use core::fmt::Write;
    let mut s = heapless::String::new();
    if write!(s, "{:.0}hPa", values.current).is_ok() {
        s
    } else {
        heapless::String::from("----hPa")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_pressure_uses_placeholder() {
        assert_eq!(pressure(None).as_str(), "----hPa");
    }
}
