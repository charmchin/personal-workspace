use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityStatus {
    pub initialized: bool,
    pub unlocked: bool,
    pub database_path: String,
    pub keychain_mode: String,
    pub snapshot_warning: Option<String>,
    pub recovery_notice: Option<String>,
    pub session_epoch: u64,
    pub lock_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub theme: String,
    pub amounts_hidden: bool,
    pub lock_minutes: i64,
    pub quote_enabled: bool,
    pub quote_auto_refresh: bool,
    pub last_quote_refresh: Option<String>,
    pub timezone: String,
    pub currency: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: "system".into(),
            amounts_hidden: false,
            lock_minutes: 15,
            quote_enabled: false,
            quote_auto_refresh: true,
            last_quote_refresh: None,
            timezone: "Asia/Shanghai".into(),
            currency: "CNY".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub title: String,
    pub notes: String,
    pub status: String,
    pub priority: i64,
    pub due_date: Option<String>,
    pub scheduled_start: Option<String>,
    pub scheduled_end: Option<String>,
    pub recurrence: String,
    pub project_id: Option<String>,
    pub goal_id: Option<String>,
    pub content_id: Option<String>,
    pub completed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarItem {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub notes: String,
    pub start_at: String,
    pub end_at: String,
    pub all_day: bool,
    pub recurrence: String,
    pub source: String,
    pub external_uid: Option<String>,
    pub project_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub area: String,
    pub status: String,
    pub color: String,
    pub notes: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkLog {
    pub id: String,
    #[serde(default)]
    pub task_id: Option<String>,
    pub log_date: String,
    pub project_id: Option<String>,
    pub title: String,
    pub completed: String,
    pub blockers: String,
    pub next_steps: String,
    pub minutes: i64,
    pub energy: i64,
    pub markdown: String,
    pub attachment_path: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Goal {
    pub id: String,
    pub title: String,
    pub horizon: String,
    pub status: String,
    pub progress: i64,
    pub notes: String,
    pub start_date: Option<String>,
    pub target_date: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Habit {
    pub id: String,
    pub name: String,
    pub frequency: String,
    pub target_per_week: i64,
    pub color: String,
    pub active: bool,
    pub checked_today: bool,
    pub streak: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearningItem {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub status: String,
    pub progress: i64,
    pub notes: String,
    pub target_date: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentItem {
    pub id: String,
    pub title: String,
    pub platform: String,
    pub format: String,
    pub status: String,
    pub goal: String,
    pub tags: Vec<String>,
    pub publish_at: Option<String>,
    pub notes: String,
    pub asset_path: Option<String>,
    pub views: i64,
    pub likes: i64,
    pub comments: i64,
    pub saves: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentMetricInput {
    pub content_id: String,
    pub recorded_at: String,
    pub views: i64,
    pub likes: i64,
    pub comments: i64,
    pub saves: i64,
    pub followers_delta: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvestmentAccount {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub currency: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Instrument {
    pub id: String,
    pub code: String,
    pub name: String,
    pub kind: String,
    pub market: String,
    pub currency: String,
    pub manual_price: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortfolioTransaction {
    pub id: String,
    pub account_id: String,
    pub instrument_id: String,
    pub kind: String,
    pub trade_date: String,
    pub quantity: String,
    pub unit_price: String,
    pub amount: String,
    pub fee: String,
    pub tax: String,
    pub notes: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Holding {
    pub account_id: String,
    pub instrument_id: String,
    pub code: String,
    pub name: String,
    pub kind: String,
    pub quantity: String,
    pub average_cost: String,
    pub total_cost: String,
    pub current_price: String,
    pub market_value: String,
    pub unrealized_gain: String,
    pub realized_gain: String,
    pub price_date: Option<String>,
    pub price_source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortfolioSnapshot {
    pub total_market_value: String,
    pub total_cost: String,
    pub unrealized_gain: String,
    pub realized_gain: String,
    pub holdings: Vec<Holding>,
    pub updated_at: Option<String>,
    pub valuation_complete: bool,
    pub missing_price_count: usize,
}

impl Default for PortfolioSnapshot {
    fn default() -> Self {
        Self {
            total_market_value: "0".into(),
            total_cost: "0".into(),
            unrealized_gain: "0".into(),
            realized_gain: "0".into(),
            holdings: vec![],
            updated_at: None,
            valuation_complete: true,
            missing_price_count: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PricePointInput {
    pub instrument_id: String,
    pub price_date: String,
    pub price: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewSnapshot {
    pub id: String,
    pub period_type: String,
    pub start_date: String,
    pub end_date: String,
    pub summary: String,
    pub reflection: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dashboard {
    pub portfolio_error: Option<String>,
    pub date: String,
    pub tasks: Vec<Task>,
    pub pending_task_count: usize,
    pub completed_task_count: usize,
    pub next_event: Option<CalendarItem>,
    pub habits: Vec<Habit>,
    pub work_logs: Vec<WorkLog>,
    pub content_items: Vec<ContentItem>,
    pub portfolio: PortfolioSnapshot,
    pub settings: AppSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub subtitle: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub imported: usize,
    pub updated: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteRefreshReport {
    pub updated: usize,
    pub failed: usize,
    pub errors: Vec<String>,
    pub refreshed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupManifest {
    pub format_version: i64,
    pub created_at: String,
    pub app_version: String,
    pub record_count: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_version: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table_record_counts: Option<std::collections::BTreeMap<String, i64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub name: String,
    pub created_at: String,
    pub size_bytes: u64,
    pub kind: String,
}
