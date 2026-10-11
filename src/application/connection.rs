//! 应用持有的连接与上传状态，由展示适配转换为底栏输入。
#[derive(Clone, Copy)]
pub enum UploadState {
    Disabled,
    Idle,
    Ok,
    Failed,
}

#[derive(Clone, Copy)]
pub enum EspLinkState {
    SerialReady,
    WifiConnected,
    Failed,
}
