use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: String,
    pub message: String,
    pub recovery: Option<String>,
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for CommandError {}

impl CommandError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            recovery: None,
        }
    }

    pub fn with_recovery(mut self, recovery: impl Into<String>) -> Self {
        self.recovery = Some(recovery.into());
        self
    }

    pub fn locked() -> Self {
        Self::new("APP_LOCKED", "工作台已锁定")
            .with_recovery("请使用 Touch ID 或系统密码重新解锁。")
    }
}

impl From<rusqlite::Error> for CommandError {
    fn from(error: rusqlite::Error) -> Self {
        Self::new("DATABASE_ERROR", format!("本地数据库操作失败：{error}"))
            .with_recovery("请重试；若问题持续，请先导出备份再检查数据完整性。")
    }
}

impl From<std::io::Error> for CommandError {
    fn from(error: std::io::Error) -> Self {
        Self::new("FILE_ERROR", format!("本地文件操作失败：{error}"))
            .with_recovery("请检查文件位置和访问权限后重试。")
    }
}

impl From<serde_json::Error> for CommandError {
    fn from(error: serde_json::Error) -> Self {
        Self::new("DATA_FORMAT_ERROR", format!("数据格式无效：{error}"))
    }
}

impl From<csv::Error> for CommandError {
    fn from(error: csv::Error) -> Self {
        Self::new("CSV_ERROR", format!("CSV 处理失败：{error}"))
    }
}

pub type CommandResult<T> = Result<T, CommandError>;
