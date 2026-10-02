use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use chrono::{DateTime, Duration, NaiveDate, TimeZone, Utc};
use csv::StringRecord;
use reqwest::Client;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};
use tauri::State;
use uuid::Uuid;

use crate::{
    backup,
    database::{AppState, get_tushare_token, now_utc, set_tushare_token},
    error::{CommandError, CommandResult},
    models::*,
    repository,
};

const MAX_ICS_BYTES: u64 = 20 * 1024 * 1024;
const MAX_CSV_BYTES: u64 = 50 * 1024 * 1024;

fn validate_input_file(path: &str, extension: &str, max_bytes: u64) -> CommandResult<()> {
    let candidate = Path::new(path);
    if candidate
        .extension()
        .and_then(|value| value.to_str())
        .is_none_or(|value| !value.eq_ignore_ascii_case(extension))
    {
        return Err(CommandError::new(
            "INVALID_FILE_TYPE",
            format!("请选择 .{extension} 文件"),
        ));
    }
    let metadata = fs::metadata(candidate)?;
    if !metadata.is_file() {
        return Err(CommandError::new("INVALID_FILE", "所选路径不是普通文件"));
    }
    if metadata.len() > max_bytes {
        return Err(CommandError::new(
            "FILE_TOO_LARGE",
            format!("所选文件超过 {} MB 安全限制", max_bytes / 1024 / 1024),
        ));
    }
    Ok(())
}

#[tauri::command(async)]
pub fn security_status(state: State<'_, AppState>) -> SecurityStatus {
    state.status()
}

#[tauri::command(async)]
pub fn security_unlock(state: State<'_, AppState>) -> CommandResult<SecurityStatus> {
    state.unlock()
}

#[tauri::command(async)]
pub fn security_initialize_with_password(
    state: State<'_, AppState>,
    password: String,
) -> CommandResult<SecurityStatus> {
    state.initialize_with_password(&password)
}

#[tauri::command(async)]
pub fn security_unlock_with_password(
    state: State<'_, AppState>,
    password: String,
) -> CommandResult<SecurityStatus> {
    state.unlock_with_password(&password)
}

#[tauri::command(async)]
pub fn security_change_password(
    state: State<'_, AppState>,
    current_password: String,
    new_password: String,
) -> CommandResult<SecurityStatus> {
    state.change_local_password(&current_password, &new_password)
}

#[tauri::command(async)]
pub fn security_lock(state: State<'_, AppState>) -> SecurityStatus {
    state.lock()
}

#[tauri::command(async)]
pub fn security_activity(state: State<'_, AppState>) -> CommandResult<()> {
    state.session.activity()
}

#[tauri::command(async)]
pub fn get_dashboard(state: State<'_, AppState>, date: String) -> CommandResult<Dashboard> {
    state.with_connection(|connection| repository::dashboard(connection, &date))
}

#[tauri::command(async)]
pub fn list_tasks(state: State<'_, AppState>) -> CommandResult<Vec<Task>> {
    state.with_connection(repository::list_tasks)
}

#[tauri::command(async)]
pub fn upsert_task(state: State<'_, AppState>, task: Task) -> CommandResult<Task> {
    state.with_connection(|connection| repository::upsert_task(connection, task))
}

#[tauri::command(async)]
pub fn toggle_task(state: State<'_, AppState>, id: String, completed: bool) -> CommandResult<Task> {
    state.with_connection(|connection| repository::toggle_task(connection, &id, completed))
}

#[tauri::command(async)]
pub fn list_calendar_items(
    state: State<'_, AppState>,
    start: Option<String>,
    end: Option<String>,
) -> CommandResult<Vec<CalendarItem>> {
    state.with_connection(|connection| {
        repository::list_calendar(connection, start.as_deref(), end.as_deref())
    })
}

#[tauri::command(async)]
pub fn upsert_calendar_item(
    state: State<'_, AppState>,
    item: CalendarItem,
) -> CommandResult<CalendarItem> {
    state.with_connection(|connection| repository::upsert_calendar(connection, item))
}

#[tauri::command(async)]
pub fn list_projects(state: State<'_, AppState>) -> CommandResult<Vec<Project>> {
    state.with_connection(repository::list_projects)
}

#[tauri::command(async)]
pub fn upsert_project(state: State<'_, AppState>, project: Project) -> CommandResult<Project> {
    state.with_connection(|connection| repository::upsert_project(connection, project))
}

#[tauri::command(async)]
pub fn list_work_logs(state: State<'_, AppState>) -> CommandResult<Vec<WorkLog>> {
    state.with_connection(repository::list_work_logs)
}

#[tauri::command(async)]
pub fn upsert_work_log(state: State<'_, AppState>, log: WorkLog) -> CommandResult<WorkLog> {
    state.with_connection(|connection| repository::upsert_work_log(connection, log))
}

#[tauri::command(async)]
pub fn list_goals(state: State<'_, AppState>) -> CommandResult<Vec<Goal>> {
    state.with_connection(repository::list_goals)
}

#[tauri::command(async)]
pub fn upsert_goal(state: State<'_, AppState>, goal: Goal) -> CommandResult<Goal> {
    state.with_connection(|connection| repository::upsert_goal(connection, goal))
}

#[tauri::command(async)]
pub fn list_habits(state: State<'_, AppState>, date: String) -> CommandResult<Vec<Habit>> {
    state.with_connection(|connection| repository::list_habits(connection, &date))
}

#[tauri::command(async)]
pub fn upsert_habit(state: State<'_, AppState>, habit: Habit) -> CommandResult<Habit> {
    state.with_connection(|connection| repository::upsert_habit(connection, habit))
}

#[tauri::command(async)]
pub fn check_habit(
    state: State<'_, AppState>,
    habit_id: String,
    date: String,
    checked: bool,
) -> CommandResult<Habit> {
    state.with_connection(|connection| {
        repository::check_habit(connection, &habit_id, &date, checked)
    })
}

#[tauri::command(async)]
pub fn list_learning_items(state: State<'_, AppState>) -> CommandResult<Vec<LearningItem>> {
    state.with_connection(repository::list_learning)
}

#[tauri::command(async)]
pub fn upsert_learning_item(
    state: State<'_, AppState>,
    item: LearningItem,
) -> CommandResult<LearningItem> {
    state.with_connection(|connection| repository::upsert_learning(connection, item))
}

#[tauri::command(async)]
pub fn list_content_items(state: State<'_, AppState>) -> CommandResult<Vec<ContentItem>> {
    state.with_connection(repository::list_content)
}

#[tauri::command(async)]
pub fn upsert_content_item(
    state: State<'_, AppState>,
    item: ContentItem,
) -> CommandResult<ContentItem> {
    state.with_connection(|connection| repository::upsert_content(connection, item))
}

#[tauri::command(async)]
pub fn add_content_metric(
    state: State<'_, AppState>,
    metric: ContentMetricInput,
) -> CommandResult<ContentItem> {
    state.with_connection(|connection| repository::add_content_metric(connection, metric))
}

#[tauri::command(async)]
pub fn list_investment_accounts(
    state: State<'_, AppState>,
) -> CommandResult<Vec<InvestmentAccount>> {
    state.with_connection(repository::list_accounts)
}

#[tauri::command(async)]
pub fn upsert_investment_account(
    state: State<'_, AppState>,
    account: InvestmentAccount,
) -> CommandResult<InvestmentAccount> {
    state.with_connection(|connection| repository::upsert_account(connection, account))
}

#[tauri::command(async)]
pub fn list_instruments(state: State<'_, AppState>) -> CommandResult<Vec<Instrument>> {
    state.with_connection(repository::list_instruments)
}

#[tauri::command(async)]
pub fn upsert_instrument(
    state: State<'_, AppState>,
    instrument: Instrument,
) -> CommandResult<Instrument> {
    state.with_connection(|connection| repository::upsert_instrument(connection, instrument))
}

#[tauri::command(async)]
pub fn list_portfolio_transactions(
    state: State<'_, AppState>,
) -> CommandResult<Vec<PortfolioTransaction>> {
    state.with_connection(repository::list_transactions)
}

#[tauri::command(async)]
pub fn upsert_portfolio_transaction(
    state: State<'_, AppState>,
    transaction: PortfolioTransaction,
) -> CommandResult<PortfolioTransaction> {
    state.with_connection(|connection| repository::upsert_transaction(connection, transaction))
}

#[tauri::command(async)]
pub fn upsert_price_point(state: State<'_, AppState>, price: PricePointInput) -> CommandResult<()> {
    state.with_connection(|connection| repository::upsert_price(connection, price))
}

#[tauri::command(async)]
pub fn get_portfolio_snapshot(state: State<'_, AppState>) -> CommandResult<PortfolioSnapshot> {
    state.with_connection(repository::portfolio_snapshot)
}

#[tauri::command(async)]
pub fn list_reviews(state: State<'_, AppState>) -> CommandResult<Vec<ReviewSnapshot>> {
    state.with_connection(repository::list_reviews)
}

#[tauri::command(async)]
pub fn upsert_review(
    state: State<'_, AppState>,
    review: ReviewSnapshot,
) -> CommandResult<ReviewSnapshot> {
    state.with_connection(|connection| repository::upsert_review(connection, review))
}

#[tauri::command(async)]
pub fn generate_weekly_summary(
    state: State<'_, AppState>,
    start_date: String,
    end_date: String,
) -> CommandResult<String> {
    state.with_connection(|connection| {
        repository::weekly_summary(connection, &start_date, &end_date)
    })
}

#[tauri::command(async)]
pub fn delete_record(state: State<'_, AppState>, kind: String, id: String) -> CommandResult<()> {
    state.with_connection(|connection| repository::delete_record(connection, &kind, &id))
}

#[tauri::command(async)]
pub fn open_saved_file(state: State<'_, AppState>, path: String) -> CommandResult<()> {
    let allowed = state
        .with_connection(|connection| repository::is_saved_file_reference(connection, &path))?;
    if !allowed {
        return Err(CommandError::new(
            "FILE_REFERENCE_NOT_ALLOWED",
            "只能打开已经保存为工作附件或自媒体素材的文件",
        ));
    }
    let candidate = Path::new(&path);
    if !candidate.is_absolute() {
        return Err(CommandError::new(
            "INVALID_FILE_REFERENCE",
            "本机文件引用必须使用绝对路径",
        ));
    }
    let metadata = fs::metadata(candidate).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            CommandError::new("FILE_NOT_FOUND", "引用的本机文件已移动或删除")
                .with_recovery("请编辑记录并重新选择文件。")
        } else {
            error.into()
        }
    })?;
    if !metadata.is_file() {
        return Err(CommandError::new(
            "INVALID_FILE_REFERENCE",
            "引用路径不是可打开的普通文件",
        ));
    }
    Command::new("/usr/bin/open")
        .arg("--")
        .arg(candidate)
        .spawn()
        .map_err(|error| CommandError::new("FILE_OPEN_FAILED", format!("无法打开文件：{error}")))?;
    Ok(())
}

#[tauri::command(async)]
pub fn search_records(
    state: State<'_, AppState>,
    query: String,
) -> CommandResult<Vec<SearchResult>> {
    state.with_connection(|connection| repository::search(connection, &query))
}

#[tauri::command(async)]
pub fn get_settings(state: State<'_, AppState>) -> CommandResult<AppSettings> {
    state.with_connection(repository::settings)
}

#[tauri::command(async)]
pub fn update_settings(
    state: State<'_, AppState>,
    settings: AppSettings,
) -> CommandResult<AppSettings> {
    let updated =
        state.with_connection(|connection| repository::update_settings(connection, settings))?;
    state.session.configure(updated.lock_minutes)?;
    Ok(updated)
}

#[tauri::command(async)]
pub fn configure_tushare_token(token: String) -> CommandResult<()> {
    set_tushare_token(&token)
}

#[tauri::command(async)]
pub fn has_tushare_token() -> bool {
    get_tushare_token().is_ok()
}

#[derive(Debug, Deserialize)]
struct TushareResponse {
    code: i64,
    msg: Option<String>,
    data: Option<TushareData>,
}

#[derive(Debug, Deserialize)]
struct TushareData {
    fields: Vec<String>,
    items: Vec<Vec<Value>>,
}

fn tushare_code(instrument: &Instrument) -> String {
    let code = instrument.code.trim().to_uppercase();
    if code.contains('.') {
        return code;
    }
    let suffix = if code.starts_with('6') || code.starts_with('5') {
        "SH"
    } else if code.starts_with('8') || code.starts_with('4') || code.starts_with('9') {
        "BJ"
    } else {
        "SZ"
    };
    format!("{code}.{suffix}")
}

async fn fetch_tushare_price(
    client: &Client,
    token: &str,
    instrument: &Instrument,
) -> CommandResult<(String, String)> {
    let (api_name, price_field, date_field) = match instrument.kind.as_str() {
        "fund" => ("fund_nav", "unit_nav", "nav_date"),
        "etf" => ("fund_daily", "close", "trade_date"),
        _ => ("daily", "close", "trade_date"),
    };
    let end = Utc::now().format("%Y%m%d").to_string();
    let start = (Utc::now() - Duration::days(14))
        .format("%Y%m%d")
        .to_string();
    let response = client
        .post("https://api.tushare.pro")
        .json(&json!({
            "api_name": api_name,
            "token": token,
            "params": {"ts_code": tushare_code(instrument), "start_date": start, "end_date": end},
            "fields": format!("{date_field},{price_field}")
        }))
        .send()
        .await
        .map_err(|error| {
            CommandError::new("QUOTE_NETWORK_ERROR", format!("行情网络请求失败：{error}"))
                .with_recovery("已保留最近一次价格，可稍后重试或使用手工价格。")
        })?;
    if !response.status().is_success() {
        return Err(CommandError::new(
            "QUOTE_HTTP_ERROR",
            format!("行情服务返回 HTTP {}", response.status()),
        ));
    }
    let payload: TushareResponse = response.json().await.map_err(|error| {
        CommandError::new("QUOTE_RESPONSE_ERROR", format!("行情响应格式无效：{error}"))
    })?;
    if payload.code != 0 {
        return Err(CommandError::new(
            "QUOTE_PROVIDER_ERROR",
            payload
                .msg
                .unwrap_or_else(|| format!("Tushare 错误代码 {}", payload.code)),
        ));
    }
    let data = payload.data.ok_or_else(|| {
        CommandError::new("QUOTE_EMPTY", format!("{} 没有可用行情", instrument.code))
    })?;
    let date_index = data
        .fields
        .iter()
        .position(|field| field == date_field)
        .ok_or_else(|| CommandError::new("QUOTE_RESPONSE_ERROR", "行情缺少日期字段"))?;
    let price_index = data
        .fields
        .iter()
        .position(|field| field == price_field)
        .ok_or_else(|| CommandError::new("QUOTE_RESPONSE_ERROR", "行情缺少价格字段"))?;
    let mut values = data.items;
    values.sort_by(|a, b| {
        b.get(date_index)
            .and_then(Value::as_str)
            .cmp(&a.get(date_index).and_then(Value::as_str))
    });
    let row = values.first().ok_or_else(|| {
        CommandError::new("QUOTE_EMPTY", format!("{} 没有可用行情", instrument.code))
    })?;
    let raw_date = row
        .get(date_index)
        .and_then(Value::as_str)
        .unwrap_or_default();
    let price = row
        .get(price_index)
        .and_then(|value| match value {
            Value::Number(number) => Some(number.to_string()),
            Value::String(value) => Some(value.clone()),
            _ => None,
        })
        .ok_or_else(|| CommandError::new("QUOTE_RESPONSE_ERROR", "行情价格字段无效"))?;
    let date = NaiveDate::parse_from_str(raw_date, "%Y%m%d")
        .map(|value| value.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|_| raw_date.to_owned());
    Ok((date, price))
}

#[tauri::command]
pub async fn refresh_tushare_quotes(
    state: State<'_, AppState>,
) -> CommandResult<QuoteRefreshReport> {
    let epoch = state.session.require_active()?;
    let token = get_tushare_token()?;
    let (settings, instruments) = state.with_connection(|connection| {
        Ok((
            repository::settings(connection)?,
            repository::list_instruments(connection)?,
        ))
    })?;
    if !settings.quote_enabled {
        return Err(CommandError::new("QUOTES_DISABLED", "联网行情尚未启用"));
    }
    let client = Client::builder()
        .https_only(true)
        .timeout(std::time::Duration::from_secs(20))
        .user_agent("PersonalWorkbench/0.1")
        .build()
        .map_err(|error| CommandError::new("QUOTE_CLIENT_ERROR", error.to_string()))?;
    let mut prices = vec![];
    let mut errors = vec![];
    for instrument in instruments
        .into_iter()
        .filter(|instrument| instrument.kind != "cash")
        .take(100)
    {
        state.session.require_epoch(epoch)?;
        match fetch_tushare_price(&client, &token, &instrument).await {
            Ok((date, price)) => prices.push(PricePointInput {
                instrument_id: instrument.id,
                price_date: date,
                price,
                source: "tushare".into(),
            }),
            Err(error) => errors.push(format!("{}：{}", instrument.code, error.message)),
        }
        state.session.require_epoch(epoch)?;
    }
    let refreshed_at = now_utc();
    let updated = prices.len();
    state.with_connection(|connection| {
        state.session.require_epoch(epoch)?;
        for price in prices {
            repository::upsert_price(connection, price)?;
        }
        let mut settings = repository::settings(connection)?;
        settings.last_quote_refresh = Some(refreshed_at.clone());
        repository::update_settings(connection, settings)?;
        Ok(())
    })?;
    Ok(QuoteRefreshReport {
        updated,
        failed: errors.len(),
        errors,
        refreshed_at,
    })
}

fn csv_index(headers: &StringRecord, names: &[&str]) -> Option<usize> {
    headers
        .iter()
        .position(|header| names.iter().any(|name| header.trim() == *name))
}

fn csv_value(record: &StringRecord, index: Option<usize>) -> &str {
    index
        .and_then(|value| record.get(value))
        .unwrap_or("")
        .trim()
}

fn transaction_kind(value: &str) -> Option<&'static str> {
    match value.trim().to_lowercase().as_str() {
        "买入" | "buy" => Some("buy"),
        "卖出" | "sell" => Some("sell"),
        "申购" | "subscribe" => Some("subscribe"),
        "赎回" | "redeem" => Some("redeem"),
        "分红" | "dividend" => Some("dividend"),
        "费用" | "fee" => Some("fee"),
        "税费" | "tax" => Some("tax"),
        "调整" | "adjustment" => Some("adjustment"),
        _ => None,
    }
}

type PortfolioCsvIndexes = (
    Option<usize>,
    Option<usize>,
    Option<usize>,
    Option<usize>,
    Option<usize>,
    Option<usize>,
    Option<usize>,
    Option<usize>,
    Option<usize>,
    Option<usize>,
    Option<usize>,
);

fn portfolio_csv_indexes(headers: &StringRecord) -> PortfolioCsvIndexes {
    (
        csv_index(headers, &["证券代码", "代码", "code"]),
        csv_index(headers, &["证券名称", "名称", "name"]),
        csv_index(headers, &["资产类型", "类型", "instrument_type"]),
        csv_index(headers, &["交易类型", "transaction_type", "kind"]),
        csv_index(headers, &["日期", "交易日期", "date"]),
        csv_index(headers, &["数量", "份额", "quantity"]),
        csv_index(headers, &["单价", "净值", "unit_price"]),
        csv_index(headers, &["金额", "amount"]),
        csv_index(headers, &["手续费", "fee"]),
        csv_index(headers, &["税费", "tax"]),
        csv_index(headers, &["备注", "notes"]),
    )
}

#[tauri::command(async)]
pub fn import_portfolio_csv(
    state: State<'_, AppState>,
    path: String,
    account_id: String,
) -> CommandResult<ImportReport> {
    validate_input_file(&path, "csv", MAX_CSV_BYTES)?;
    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .from_path(&path)
        .map_err(|error| CommandError::new("CSV_READ_FAILED", format!("无法读取 CSV：{error}")))?;
    let headers = reader.headers()?.clone();
    let indexes = portfolio_csv_indexes(&headers);
    if indexes.0.is_none() || indexes.1.is_none() || indexes.3.is_none() || indexes.4.is_none() {
        return Err(CommandError::new(
            "CSV_HEADERS_INVALID",
            "CSV 至少需要证券代码、证券名称、交易类型和日期列",
        ));
    }
    let records = reader
        .records()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| CommandError::new("CSV_PARSE_FAILED", format!("CSV 格式错误：{error}")))?;
    state.with_connection_mut(|connection| {
        import_portfolio_records(connection, indexes, &records, &account_id)
    })
}

fn import_portfolio_records(
    connection: &mut Connection,
    indexes: PortfolioCsvIndexes,
    records: &[StringRecord],
    account_id: &str,
) -> CommandResult<ImportReport> {
    let mut transaction = connection.transaction()?;
    let mut report = ImportReport {
        imported: 0,
        updated: 0,
        skipped: 0,
        errors: vec![],
    };
    for (offset, record) in records.iter().enumerate() {
        let line = offset + 2;
        let code = csv_value(record, indexes.0).to_uppercase();
        let name = csv_value(record, indexes.1);
        let Some(kind) = transaction_kind(csv_value(record, indexes.3)) else {
            report.skipped += 1;
            report.errors.push(format!("第 {line} 行：未知交易类型"));
            continue;
        };
        let savepoint = transaction.savepoint()?;
        let instrument_id: Option<String> = savepoint
            .query_row("SELECT id FROM instruments WHERE code=?1", [&code], |row| {
                row.get(0)
            })
            .optional()?;
        let instrument = Instrument {
            id: instrument_id.clone().unwrap_or_default(),
            code: code.clone(),
            name: name.to_owned(),
            kind: match csv_value(record, indexes.2) {
                "基金" => "fund",
                "ETF" => "etf",
                "现金" => "cash",
                _ => "stock",
            }
            .into(),
            market: "CN".into(),
            currency: "CNY".into(),
            manual_price: None,
            created_at: String::new(),
            updated_at: String::new(),
        };
        let instrument = match repository::upsert_instrument(&savepoint, instrument) {
            Ok(value) => value,
            Err(error) => {
                report.skipped += 1;
                report
                    .errors
                    .push(format!("第 {line} 行：{}", error.message));
                continue;
            }
        };
        let item = PortfolioTransaction {
            id: String::new(),
            account_id: account_id.to_owned(),
            instrument_id: instrument.id,
            kind: kind.into(),
            trade_date: csv_value(record, indexes.4).into(),
            quantity: value_or_zero(csv_value(record, indexes.5)),
            unit_price: value_or_zero(csv_value(record, indexes.6)),
            amount: value_or_zero(csv_value(record, indexes.7)),
            fee: value_or_zero(csv_value(record, indexes.8)),
            tax: value_or_zero(csv_value(record, indexes.9)),
            notes: csv_value(record, indexes.10).into(),
            created_at: String::new(),
            updated_at: String::new(),
        };
        match repository::upsert_transaction(&savepoint, item) {
            Ok(_) => {
                savepoint.commit()?;
                report.imported += 1;
            }
            Err(error) => {
                report.skipped += 1;
                report
                    .errors
                    .push(format!("第 {line} 行：{}", error.message));
            }
        }
    }
    transaction.commit()?;
    Ok(report)
}

fn value_or_zero(value: &str) -> String {
    if value.trim().is_empty() {
        "0".into()
    } else {
        value.trim().replace(',', "")
    }
}

#[tauri::command(async)]
pub fn export_portfolio_csv_template(path: String) -> CommandResult<()> {
    if Path::new(&path)
        .extension()
        .and_then(|value| value.to_str())
        .is_none_or(|value| !value.eq_ignore_ascii_case("csv"))
    {
        return Err(CommandError::new(
            "INVALID_FILE_TYPE",
            "CSV 模板必须使用 .csv 扩展名",
        ));
    }
    let mut writer = csv::Writer::from_path(path).map_err(|error| {
        CommandError::new("CSV_WRITE_FAILED", format!("无法创建 CSV 模板：{error}"))
    })?;
    writer.write_record([
        "证券代码",
        "证券名称",
        "资产类型",
        "交易类型",
        "日期",
        "数量",
        "单价",
        "金额",
        "手续费",
        "税费",
        "备注",
    ])?;
    writer.write_record([
        "510300",
        "沪深300ETF",
        "ETF",
        "买入",
        "2026-01-02",
        "1000",
        "4.125",
        "4125",
        "5",
        "0",
        "示例行，可删除",
    ])?;
    writer.flush()?;
    Ok(())
}

fn unfold_ics(input: &str) -> Vec<String> {
    let mut lines: Vec<String> = vec![];
    for raw in input.replace("\r\n", "\n").lines() {
        if raw.starts_with(' ') || raw.starts_with('\t') {
            if let Some(last) = lines.last_mut() {
                last.push_str(raw.trim_start());
            }
        } else {
            lines.push(raw.to_owned());
        }
    }
    lines
}
fn unescape_ics(value: &str) -> String {
    value
        .replace("\\n", "\n")
        .replace("\\,", ",")
        .replace("\\;", ";")
        .replace("\\\\", "\\")
}
fn escape_ics(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace(',', "\\,")
        .replace(';', "\\;")
}
fn ics_datetime(value: &str) -> CommandResult<(String, bool)> {
    if value.len() == 8 {
        let date = NaiveDate::parse_from_str(value, "%Y%m%d")
            .map_err(|_| CommandError::new("ICS_DATE_INVALID", "ICS 日期格式无效"))?;
        return Ok((format!("{}T00:00:00+08:00", date.format("%Y-%m-%d")), true));
    }
    if value.ends_with('Z') {
        let date = chrono::NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%SZ")
            .map_err(|_| CommandError::new("ICS_DATE_INVALID", "ICS UTC 时间格式无效"))?;
        return Ok((date.and_utc().to_rfc3339(), false));
    }
    let naive = chrono::NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%S")
        .map_err(|_| CommandError::new("ICS_DATE_INVALID", "ICS 本地时间格式无效"))?;
    let fixed = chrono::FixedOffset::east_opt(8 * 3600)
        .unwrap()
        .from_local_datetime(&naive)
        .single()
        .ok_or_else(|| CommandError::new("ICS_DATE_INVALID", "ICS 本地时间无效"))?;
    Ok((fixed.to_rfc3339(), false))
}

#[tauri::command(async)]
pub fn import_ics(
    state: State<'_, AppState>,
    path: String,
    source: String,
) -> CommandResult<ImportReport> {
    validate_input_file(&path, "ics", MAX_ICS_BYTES)?;
    let input = fs::read_to_string(&path)?;
    state.with_connection_mut(|connection| import_ics_text(connection, &input, &source))
}

fn import_ics_text(
    connection: &mut Connection,
    input: &str,
    source: &str,
) -> CommandResult<ImportReport> {
    let lines = unfold_ics(input);
    let mut events: Vec<HashMap<String, String>> = vec![];
    let mut current: Option<HashMap<String, String>> = None;
    for line in lines {
        if line == "BEGIN:VEVENT" {
            current = Some(HashMap::new());
            continue;
        }
        if line == "END:VEVENT" {
            if let Some(event) = current.take() {
                events.push(event);
            }
            continue;
        }
        if let Some(event) = current.as_mut()
            && let Some((key, value)) = line.split_once(':')
        {
            let base = key.split(';').next().unwrap_or(key).to_owned();
            event.insert(base, value.to_owned());
        }
    }
    let transaction = connection.transaction()?;
    let mut report = ImportReport {
        imported: 0,
        updated: 0,
        skipped: 0,
        errors: vec![],
    };
    for (index, event) in events.into_iter().enumerate() {
        let uid = event
            .get("UID")
            .cloned()
            .unwrap_or_else(|| format!("generated-{}", Uuid::now_v7()));
        let Some(start_raw) = event.get("DTSTART") else {
            report.skipped += 1;
            report
                .errors
                .push(format!("第 {} 个事件缺少开始时间", index + 1));
            continue;
        };
        let (start, all_day) = match ics_datetime(start_raw) {
            Ok(value) => value,
            Err(error) => {
                report.skipped += 1;
                report
                    .errors
                    .push(format!("第 {} 个事件：{}", index + 1, error.message));
                continue;
            }
        };
        let end = if let Some(raw) = event.get("DTEND") {
            ics_datetime(raw)
                .map(|value| value.0)
                .unwrap_or_else(|_| start.clone())
        } else if all_day {
            let next_day = NaiveDate::parse_from_str(&start[..10], "%Y-%m-%d")
                .map_err(|_| CommandError::new("ICS_DATE_INVALID", "ICS 全天事件日期无效"))?
                + Duration::days(1);
            format!("{}T00:00:00+08:00", next_day.format("%Y-%m-%d"))
        } else {
            DateTime::parse_from_rfc3339(&start)
                .map(|value| (value + Duration::hours(1)).to_rfc3339())
                .unwrap_or_else(|_| start.clone())
        };
        let exists = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM calendar_items WHERE source=?1 AND external_uid=?2)",
                params![source, uid],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0)
            != 0;
        let recurrence = event
            .get("RRULE")
            .and_then(|rule| rule.split(';').find_map(|part| part.strip_prefix("FREQ=")))
            .map(|frequency| match frequency {
                "DAILY" => "daily",
                "WEEKLY" => "weekly",
                "MONTHLY" => "monthly",
                _ => "none",
            })
            .unwrap_or("none");
        let now = now_utc();
        transaction.execute(
                r#"INSERT INTO calendar_items(id,kind,title,notes,start_at,end_at,all_day,recurrence,source,external_uid,project_id,created_at,updated_at)
                   VALUES(?1,'event',?2,?3,?4,?5,?6,?7,?8,?9,NULL,?10,?10)
                   ON CONFLICT(source,external_uid) DO UPDATE SET title=excluded.title,notes=excluded.notes,start_at=excluded.start_at,end_at=excluded.end_at,all_day=excluded.all_day,recurrence=excluded.recurrence,updated_at=excluded.updated_at"#,
                params![Uuid::now_v7().to_string(), unescape_ics(event.get("SUMMARY").map(String::as_str).unwrap_or("未命名日程")), unescape_ics(event.get("DESCRIPTION").map(String::as_str).unwrap_or("")), start, end, all_day as i64, recurrence, source, uid, now],
            )?;
        if exists {
            report.updated += 1;
        } else {
            report.imported += 1;
        }
    }
    transaction.commit()?;
    Ok(report)
}

fn iso_to_ics(value: &str, all_day: bool) -> String {
    if all_day {
        return value.get(..10).unwrap_or(value).replace('-', "");
    }
    DateTime::parse_from_rfc3339(value)
        .map(|date| {
            date.with_timezone(&Utc)
                .format("%Y%m%dT%H%M%SZ")
                .to_string()
        })
        .unwrap_or_else(|_| value.into())
}

#[tauri::command(async)]
pub fn export_ics(state: State<'_, AppState>, path: String) -> CommandResult<usize> {
    if Path::new(&path)
        .extension()
        .and_then(|value| value.to_str())
        .is_none_or(|value| !value.eq_ignore_ascii_case("ics"))
    {
        return Err(CommandError::new(
            "INVALID_FILE_TYPE",
            "日程文件必须使用 .ics 扩展名",
        ));
    }
    let items =
        state.with_connection(|connection| repository::list_calendar(connection, None, None))?;
    fs::write(path, render_ics(&items))?;
    Ok(items.len())
}

fn render_ics(items: &[CalendarItem]) -> String {
    let mut output = String::from(
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Personal Workbench//CN\r\nCALSCALE:GREGORIAN\r\n",
    );
    for item in items {
        output.push_str("BEGIN:VEVENT\r\n");
        output.push_str(&format!(
            "UID:{}@personal-workbench\r\n",
            item.external_uid.as_deref().unwrap_or(&item.id)
        ));
        output.push_str(&format!(
            "DTSTAMP:{}\r\n",
            Utc::now().format("%Y%m%dT%H%M%SZ")
        ));
        if item.all_day {
            output.push_str(&format!(
                "DTSTART;VALUE=DATE:{}\r\nDTEND;VALUE=DATE:{}\r\n",
                iso_to_ics(&item.start_at, true),
                iso_to_ics(&item.end_at, true)
            ));
        } else {
            output.push_str(&format!(
                "DTSTART:{}\r\nDTEND:{}\r\n",
                iso_to_ics(&item.start_at, false),
                iso_to_ics(&item.end_at, false)
            ));
        }
        output.push_str(&format!(
            "SUMMARY:{}\r\nDESCRIPTION:{}\r\n",
            escape_ics(&item.title),
            escape_ics(&item.notes)
        ));
        let freq = match item.recurrence.as_str() {
            "daily" => Some("DAILY"),
            "weekly" => Some("WEEKLY"),
            "monthly" => Some("MONTHLY"),
            _ => None,
        };
        if let Some(freq) = freq {
            output.push_str(&format!("RRULE:FREQ={freq}\r\n"));
        }
        output.push_str("END:VEVENT\r\n");
    }
    output.push_str("END:VCALENDAR\r\n");
    output
}

#[tauri::command(async)]
pub fn export_backup(
    state: State<'_, AppState>,
    path: String,
    password: String,
) -> CommandResult<BackupManifest> {
    backup::export_backup(&state, &PathBuf::from(path), &password)
}

#[tauri::command(async)]
pub fn restore_backup(
    state: State<'_, AppState>,
    path: String,
    password: String,
) -> CommandResult<BackupManifest> {
    backup::restore_backup(&state, &PathBuf::from(path), &password)
}
#[tauri::command(async)]
pub fn list_backups(state: State<'_, AppState>) -> CommandResult<Vec<BackupInfo>> {
    backup::list_backups(&state)
}

#[tauri::command(async)]
pub fn delete_backup(state: State<'_, AppState>, name: String) -> CommandResult<()> {
    backup::delete_backup(&state, &name)
}

#[tauri::command(async)]
pub fn restore_snapshot(state: State<'_, AppState>, name: String) -> CommandResult<i64> {
    backup::restore_snapshot(&state, &name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::open_database;

    #[test]
    fn ics_text_round_trip_and_folded_lines_are_supported() {
        let original = "第一行,带分号;\\\n第二行";
        assert_eq!(unescape_ics(&escape_ics(original)), original);
        let lines = unfold_ics("SUMMARY:很长的\r\n 标题\r\nUID:test");
        assert_eq!(lines, vec!["SUMMARY:很长的标题", "UID:test"]);
    }

    #[test]
    fn ics_dates_support_all_day_local_and_utc_values() {
        assert_eq!(
            ics_datetime("20260829").unwrap(),
            ("2026-08-29T00:00:00+08:00".into(), true)
        );
        assert!(!ics_datetime("20260829T090000").unwrap().1);
        assert!(
            ics_datetime("20260829T010000Z")
                .unwrap()
                .0
                .ends_with("+00:00")
        );
        assert!(ics_datetime("invalid").is_err());
    }

    #[test]
    fn csv_template_and_import_preserve_precision_and_report_bad_rows() {
        let directory = tempfile::tempdir().unwrap();
        let template_path = directory.path().join("template.csv");
        export_portfolio_csv_template(template_path.to_string_lossy().into_owned()).unwrap();
        let template = fs::read_to_string(template_path).unwrap();
        assert!(template.contains("证券代码,证券名称,资产类型,交易类型,日期"));
        assert!(template.contains("510300"));

        let mut connection = open_database(
            &directory.path().join("portfolio-csv.sqlite3"),
            &[31_u8; 32],
        )
        .unwrap();
        let account = repository::upsert_account(
            &connection,
            InvestmentAccount {
                id: String::new(),
                name: "CSV 验证账户".into(),
                kind: "securities".into(),
                currency: "CNY".into(),
                created_at: String::new(),
                updated_at: String::new(),
            },
        )
        .unwrap();
        let csv = concat!(
            "证券代码,证券名称,资产类型,交易类型,日期,数量,单价,金额,手续费,税费,备注\n",
            "510300,精度ETF,ETF,买入,2026-08-30,123.4567,1.2345,,0.12,0,保留小数\n",
            "000001,错误行,股票,未知,2026-08-30,1,10,10,0,0,应跳过\n"
        );
        let mut reader = csv::ReaderBuilder::new().from_reader(csv.as_bytes());
        let headers = reader.headers().unwrap().clone();
        let records = reader.records().collect::<Result<Vec<_>, _>>().unwrap();
        let report = import_portfolio_records(
            &mut connection,
            portfolio_csv_indexes(&headers),
            &records,
            &account.id,
        )
        .unwrap();
        assert_eq!(report.imported, 1, "CSV 导入报告：{report:?}");
        assert_eq!(report.skipped, 1);
        assert!(report.errors[0].contains("未知交易类型"));
        let saved = repository::list_transactions(&connection).unwrap();
        assert_eq!(saved[0].quantity, "123.4567");
        assert_eq!(saved[0].unit_price, "1.2345");
        assert_eq!(saved[0].fee, "0.12");
    }

    #[test]
    fn ics_file_logic_updates_duplicate_uid_and_renders_round_trip_text() {
        let directory = tempfile::tempdir().unwrap();
        let mut connection =
            open_database(&directory.path().join("calendar-ics.sqlite3"), &[32_u8; 32]).unwrap();
        let original = concat!(
            "BEGIN:VCALENDAR\r\n",
            "VERSION:2.0\r\n",
            "BEGIN:VEVENT\r\n",
            "UID:stable-uid\r\n",
            "DTSTART:20260830T090000\r\n",
            "DTEND:20260830T100000\r\n",
            "SUMMARY:原始标题\r\n",
            "DESCRIPTION:第一行\\n第二行\r\n",
            "RRULE:FREQ=WEEKLY\r\n",
            "END:VEVENT\r\n",
            "END:VCALENDAR\r\n"
        );
        let first = import_ics_text(&mut connection, original, "ics:test").unwrap();
        assert_eq!(first.imported, 1);
        assert_eq!(first.updated, 0);

        let updated = original
            .replace("原始标题", "更新后的标题")
            .replace("20260830T090000", "20260830T110000")
            .replace("20260830T100000", "20260830T120000");
        let second = import_ics_text(&mut connection, &updated, "ics:test").unwrap();
        assert_eq!(second.imported, 0);
        assert_eq!(second.updated, 1);
        let items = repository::list_calendar(&connection, None, None).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "更新后的标题");
        assert_eq!(items[0].recurrence, "weekly");
        assert_eq!(items[0].notes, "第一行\n第二行");

        let rendered = render_ics(&items);
        assert!(rendered.starts_with("BEGIN:VCALENDAR\r\n"));
        assert!(rendered.contains("SUMMARY:更新后的标题\r\n"));
        assert!(rendered.contains("DESCRIPTION:第一行\\n第二行\r\n"));
        assert!(rendered.contains("RRULE:FREQ=WEEKLY\r\n"));
        assert!(rendered.ends_with("END:VCALENDAR\r\n"));
    }
}
