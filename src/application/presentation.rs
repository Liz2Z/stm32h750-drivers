//! 将采样记录和应用状态转换为控件声明的输入，转换逻辑不放进 UI。
use super::{
    acquisition::{ReadingSnapshot, SampleState},
    connection::{EspLinkState, UploadState},
};
use crate::ui::{
    status::{ConnectionLabels, StatusData, StatusLabel, StatusTone},
    PressureCardData, TempHumidCardData, ValueRange, ValueStatus,
};

fn values(series: &ReadingSnapshot) -> Option<ValueRange> {
    series.values().map(|values| ValueRange {
        current: values.current,
        high: values.high,
        low: values.low,
    })
}

fn status(series: &ReadingSnapshot) -> ValueStatus {
    match series.state() {
        SampleState::Pending => ValueStatus::Waiting,
        SampleState::Fresh => ValueStatus::Normal,
        SampleState::Stale => ValueStatus::Stale,
        SampleState::Error => ValueStatus::Error,
    }
}

/// 摄氏度或湿度百分比的记录转为卡片输入，不传递历史与时间戳。
pub fn climate_card(series: &ReadingSnapshot) -> TempHumidCardData {
    TempHumidCardData {
        values: values(series),
        status: status(series),
    }
}

/// 气压记录使用 hPa 单位，控件自己负责文字格式与布局。
pub fn pressure_card(series: &ReadingSnapshot) -> PressureCardData {
    PressureCardData {
        values: values(series),
        status: status(series),
    }
}

/// 将连接流程状态转为底栏标签；UI 无需认识 ESP 或上传状态枚举。
pub fn status_bar(
    esp_enabled: bool,
    link: EspLinkState,
    upload: UploadState,
    sample_count: u32,
    refresh_centihz: Option<u32>,
) -> StatusData {
    let connection = esp_enabled.then(|| {
        let (text, tone) = match link {
            EspLinkState::SerialReady => ("ESP: AT ready", StatusTone::Normal),
            EspLinkState::WifiConnected => ("WiFi: connected", StatusTone::Success),
            EspLinkState::Failed => ("ESP: failed", StatusTone::Failure),
        };
        let link = StatusLabel { text, tone };
        let (text, tone) = match upload {
            UploadState::Disabled => ("Upload: disabled", StatusTone::Muted),
            UploadState::Idle => ("Upload: idle", StatusTone::Normal),
            UploadState::Ok => ("Upload: ok", StatusTone::Success),
            UploadState::Failed => ("Upload: failed", StatusTone::Failure),
        };
        ConnectionLabels {
            link,
            upload: StatusLabel { text, tone },
        }
    });
    StatusData {
        connection,
        sample_count,
        refresh_centihz,
    }
}
