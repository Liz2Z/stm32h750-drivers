//! 将采样记录和应用状态转换为控件声明的输入，转换逻辑不放进 UI。
use super::{
    acquisition::{ReadingSnapshot, SampleState},
    connection::{EspLinkState, UploadState},
    rfid::ReaderState,
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

/// 在应用侧格式化卡号；UI 只接收文字和提示色，不依赖 RC522 驱动类型。
pub fn reader_status(state: ReaderState) -> StatusLabel {
    let (label, tone) = match state {
        ReaderState::Starting => ("Starting reader", StatusTone::Muted),
        ReaderState::Waiting => ("Waiting for card", StatusTone::Normal),
        ReaderState::Present => ("Card detected", StatusTone::Success),
        ReaderState::Collision => ("Use one card at a time", StatusTone::Failure),
        ReaderState::InitFailed => ("RC522: check wiring", StatusTone::Failure),
        ReaderState::ReadFailed => ("Read error / retrying", StatusTone::Failure),
    };
    StatusLabel { text: label, tone }
}

/// 驱动的 UID 转为 UI 所需文字，32 字节容量覆盖最长 10 字节卡号及分隔符。
pub fn uid_text(uid: Option<&crate::drivers::rc522::Uid>) -> heapless::String<32> {
    use core::fmt::Write;
    let mut text = heapless::String::new();
    if let Some(uid) = uid {
        for (index, byte) in uid.as_bytes().iter().enumerate() {
            if index != 0 {
                let _ = text.push(' ');
            }
            let _ = write!(text, "{byte:02X}");
        }
    } else {
        let _ = text.push_str("-- -- -- --");
    }
    text
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

#[cfg(test)]
mod tests {
    use super::status_bar;
    #[test]
    fn footer_mapping_preserves_labels_and_hides_disabled_connection() {
        use crate::{
            application::connection::{EspLinkState, UploadState},
            ui::status::StatusTone,
        };
        for (link, text, tone) in [
            (
                EspLinkState::SerialReady,
                "ESP: AT ready",
                StatusTone::Normal,
            ),
            (
                EspLinkState::WifiConnected,
                "WiFi: connected",
                StatusTone::Success,
            ),
            (EspLinkState::Failed, "ESP: failed", StatusTone::Failure),
        ] {
            for (upload, upload_text, upload_tone) in [
                (UploadState::Disabled, "Upload: disabled", StatusTone::Muted),
                (UploadState::Idle, "Upload: idle", StatusTone::Normal),
                (UploadState::Ok, "Upload: ok", StatusTone::Success),
                (UploadState::Failed, "Upload: failed", StatusTone::Failure),
            ] {
                let hidden = status_bar(false, link, upload, 42, Some(5920));
                assert_eq!(hidden.connection, None);
                assert_eq!(hidden.sample_count, 42);
                assert_eq!(hidden.refresh_centihz, Some(5920));
                let shown = status_bar(true, link, upload, 42, None);
                let labels = shown.connection.unwrap();
                assert_eq!((labels.link.text, labels.link.tone), (text, tone));
                assert_eq!(
                    (labels.upload.text, labels.upload.tone),
                    (upload_text, upload_tone)
                );
            }
        }
    }
}
