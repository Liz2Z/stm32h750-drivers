//! 数值控件共用的显示输入，不包含采样历史或业务状态转换。

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ValueRange {
    pub current: f32,
    pub high: f32,
    pub low: f32,
}

/// 控件显示哪种提示；业务状态到提示的映射由应用完成。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ValueStatus {
    #[default]
    Waiting,
    Normal,
    Stale,
    Error,
}
