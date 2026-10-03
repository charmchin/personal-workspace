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
            .with_recovery("请使用工作台当前配置的认证方式重新解锁。")
    }
}

impl From<rusqlite::Error> for CommandError {
    fn from(_error: rusqlite::Error) -> Self {
        // SQLite errors can contain SQL, parameters or key-bearing statements.
        Self::new("DATABASE_ERROR", "本地数据库操作未完成")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_errors_do_not_expose_sql_or_parameter_details() {
        let connection = rusqlite::Connection::open_in_memory().unwrap();
        let raw = connection
            .execute_batch("invalid SQL 'synthetic-database-key-and-private-value'")
            .unwrap_err();
        let converted = CommandError::from(raw);
        let serialized = serde_json::to_string(&converted).unwrap();
        assert_eq!(converted.code, "DATABASE_ERROR");
        assert!(!serialized.contains("synthetic"));
        assert!(!serialized.contains("invalid SQL"));
        assert!(converted.recovery.is_some());
    }
}
