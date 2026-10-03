//! Fixed-origin quote transport. Credentials and provider bodies are never
//! included in user-facing errors; redirects and environment proxies are off.
use chrono::{Duration, NaiveDate, Utc};
use reqwest::{Client, ClientBuilder, Response};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    future::{Future, poll_fn},
    sync::{Arc, Mutex, MutexGuard, Weak},
    task::Poll,
};
use zeroize::Zeroizing;

use crate::{
    cancellation::Cancellation,
    database::{AppState, get_tushare_token, now_utc},
    error::{CommandError, CommandResult},
    models::{AppSettings, Instrument, PricePointInput, QuoteRefreshReport},
    repository,
    session::SessionPolicy,
};

const TUSHARE_ENDPOINT: &str = "https://api.tushare.pro";
const MAX_QUOTE_RESPONSE_BYTES: usize = 1024 * 1024;

#[derive(Default)]
struct QuoteState {
    generation: u64,
    restoring: usize,
    running: Weak<Cancellation>,
}

#[derive(Default)]
pub(crate) struct QuoteControl(Mutex<QuoteState>);

impl QuoteControl {
    fn state(&self) -> MutexGuard<'_, QuoteState> {
        self.0.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn invalidate(state: &mut QuoteState) {
        state.generation += 1;
        if let Some(running) = state.running.upgrade() {
            running.cancel();
        }
    }

    fn disable(&self) {
        Self::invalidate(&mut self.state());
    }

    pub(crate) fn restoring(&self) -> RestoreGuard<'_> {
        let mut state = self.state();
        state.restoring += 1;
        Self::invalidate(&mut state);
        RestoreGuard(self)
    }

    fn begin(&self, session: &SessionPolicy, epoch: u64) -> CommandResult<QuoteJob<'_>> {
        let session_cancellation = session.cancellation(epoch)?;
        let mut state = self.state();
        if state.restoring != 0 {
            return Err(CommandError::new(
                "QUOTES_RESTORE_IN_PROGRESS",
                "正在恢复数据库，请稍后刷新行情",
            ));
        }
        if state.running.upgrade().is_some() {
            return Err(CommandError::new(
                "QUOTE_REFRESH_BUSY",
                "行情正在刷新，请等待本次刷新结束",
            ));
        }
        let cancellation = Arc::default();
        state.running = Arc::downgrade(&cancellation);
        Ok(QuoteJob {
            control: self,
            generation: state.generation,
            epoch,
            cancellation,
            session_cancellation,
        })
    }
}

pub(crate) struct RestoreGuard<'a>(&'a QuoteControl);

impl Drop for RestoreGuard<'_> {
    fn drop(&mut self) {
        self.0.state().restoring -= 1;
    }
}

struct QuoteJob<'a> {
    control: &'a QuoteControl,
    generation: u64,
    epoch: u64,
    cancellation: Arc<Cancellation>,
    session_cancellation: Arc<Cancellation>,
}

impl Drop for QuoteJob<'_> {
    fn drop(&mut self) {
        // Only one job is admitted at a time. Do not retain completed task wakers
        // in a long-lived unlocked session (including "never auto-lock").
        self.session_cancellation.clear_waiters();
        self.cancellation.clear_waiters();
    }
}

fn cancelled() -> CommandError {
    CommandError::new("QUOTES_CANCELLED", "行情刷新已取消").with_recovery(
        "未提交的行情结果已丢弃；已提交价格不会撤回。重新启用联网并解锁后可手动刷新。",
    )
}

impl QuoteJob<'_> {
    fn check_state(&self, state: &QuoteState, session: &SessionPolicy) -> CommandResult<()> {
        session.require_epoch(self.epoch)?;
        if self.cancellation.is_cancelled()
            || state.generation != self.generation
            || state.restoring != 0
        {
            return Err(cancelled());
        }
        Ok(())
    }

    fn check(&self, session: &SessionPolicy) -> CommandResult<()> {
        self.check_state(&self.control.state(), session)
    }

    async fn run<T>(
        &self,
        session: &SessionPolicy,
        future: impl Future<Output = CommandResult<T>>,
    ) -> CommandResult<T> {
        let mut future = std::pin::pin!(future);
        let result = poll_fn(|cx| {
            // Register before polling network IO; cancellation wakes stalled headers or bodies.
            if self.session_cancellation.poll_cancelled(cx).is_ready() {
                return Poll::Ready(Err(CommandError::locked()));
            }
            if self.cancellation.poll_cancelled(cx).is_ready() {
                return Poll::Ready(Err(cancelled()));
            }
            if let Err(error) = self.check(session) {
                return Poll::Ready(Err(error));
            }
            future.as_mut().poll(cx)
        })
        .await;
        self.check(session)?;
        result
    }
}

pub(crate) fn update_settings(
    state: &AppState,
    settings: AppSettings,
) -> CommandResult<AppSettings> {
    state.with_connection(|connection| {
        let updated = repository::update_settings(connection, settings)?;
        // Serialized with refresh admission and writes by the database mutex.
        if !updated.quote_enabled {
            state.quotes.disable();
        }
        state.session.configure(updated.lock_minutes)?;
        Ok(updated)
    })
}

fn begin_refresh(state: &AppState) -> CommandResult<(QuoteJob<'_>, Vec<Instrument>)> {
    let epoch = state.session.require_active()?;
    state.with_connection(|connection| {
        state.session.require_epoch(epoch)?;
        if !repository::settings(connection)?.quote_enabled {
            return Err(CommandError::new("QUOTES_DISABLED", "联网行情尚未启用"));
        }
        let job = state.quotes.begin(&state.session, epoch)?;
        let instruments = repository::list_instruments(connection)?;
        Ok((job, instruments))
    })
}

pub(crate) async fn refresh_quotes(state: &AppState) -> CommandResult<QuoteRefreshReport> {
    let (job, instruments) = begin_refresh(state)?;
    // Synchronous OS authentication cannot be interrupted; recheck before any network IO.
    let token = Zeroizing::new(get_tushare_token()?);
    let client = build_quote_client()?;
    run_refresh(state, &job, &client, &token, instruments, TUSHARE_ENDPOINT).await
}

async fn run_refresh(
    state: &AppState,
    job: &QuoteJob<'_>,
    client: &Client,
    token: &str,
    instruments: Vec<Instrument>,
    endpoint: &str,
) -> CommandResult<QuoteRefreshReport> {
    let mut prices = vec![];
    let mut errors = vec![];
    for instrument in instruments
        .into_iter()
        .filter(|item| item.kind != "cash")
        .take(100)
    {
        match job
            .run(
                &state.session,
                fetch_tushare_price(client, token, &instrument, endpoint),
            )
            .await
        {
            Ok((date, price)) => prices.push(PricePointInput {
                instrument_id: instrument.id,
                price_date: date,
                price,
                source: "tushare".into(),
            }),
            Err(error) => {
                // Cancellation is a batch failure, not a provider error to swallow and continue.
                job.check(&state.session)?;
                errors.push(format!("{}：{}", instrument.code, error.message));
            }
        }
    }
    commit_prices(state, job, prices, errors)
}

fn commit_prices(
    state: &AppState,
    job: &QuoteJob<'_>,
    prices: Vec<PricePointInput>,
    errors: Vec<String>,
) -> CommandResult<QuoteRefreshReport> {
    let refreshed_at = now_utc();
    let updated = prices.len();
    state.with_connection_mut(|connection| {
        job.check(&state.session)?;
        if !repository::settings(connection)?.quote_enabled {
            return Err(cancelled());
        }
        let transaction = connection.transaction()?;
        for price in prices {
            job.check(&state.session)?;
            repository::upsert_price(&transaction, price)?;
        }
        let mut settings = repository::settings(&transaction)?;
        settings.last_quote_refresh = Some(refreshed_at.clone());
        repository::update_settings(&transaction, settings)?;
        // The short commit is linearized with restoration/disable. Native session revocation
        // remains independent: an already-started SQLite commit safely finishes, never aborted.
        let control = state.quotes.state();
        job.check_state(&control, &state.session)?;
        transaction.commit()?;
        Ok(())
    })?;
    job.check(&state.session)?;
    Ok(QuoteRefreshReport {
        updated,
        failed: errors.len(),
        errors,
        refreshed_at,
    })
}

fn quote_client_builder() -> ClientBuilder {
    Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .timeout(std::time::Duration::from_secs(20))
        .user_agent("PersonalWorkbench/0.1")
}

pub(crate) fn build_quote_client() -> CommandResult<Client> {
    quote_client_builder().build().map_err(|_| {
        CommandError::new("QUOTE_CLIENT_ERROR", "无法初始化安全行情连接")
            .with_recovery("请检查本机网络环境后重试；最近一次价格未被修改。")
    })
}

#[derive(Debug, Deserialize)]
struct TushareResponse {
    code: i64,
    // Deliberately do not deserialize msg: providers can echo credentials.
    data: Option<TushareData>,
}

#[derive(Debug, Deserialize)]
struct TushareData {
    fields: Vec<String>,
    items: Vec<Vec<Value>>,
}

fn network_error(error: reqwest::Error) -> CommandError {
    let message = if error.is_timeout() {
        "行情请求超时"
    } else {
        "行情网络请求失败"
    };
    CommandError::new("QUOTE_NETWORK_ERROR", message)
        .with_recovery("已保留最近一次价格，可稍后重试或使用手工价格。")
}

fn response_too_large() -> CommandError {
    CommandError::new(
        "QUOTE_RESPONSE_TOO_LARGE",
        "行情响应超过 1 MiB 安全限制，已停止读取",
    )
    .with_recovery("最近一次价格未被修改，请稍后重试或使用手工价格。")
}

async fn read_quote_response(mut response: Response) -> CommandResult<TushareResponse> {
    if response.status().is_redirection() {
        return Err(
            CommandError::new("QUOTE_REDIRECT_BLOCKED", "行情服务要求重定向，已停止请求")
                .with_recovery("仅允许直接访问指定行情域名；最近一次价格未被修改。"),
        );
    }
    if !response.status().is_success() {
        return Err(CommandError::new(
            "QUOTE_HTTP_ERROR",
            format!("行情服务返回 HTTP {}", response.status().as_u16()),
        ));
    }
    if response
        .content_length()
        .is_some_and(|size| size > MAX_QUOTE_RESPONSE_BYTES as u64)
    {
        return Err(response_too_large());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(network_error)? {
        if bytes
            .len()
            .checked_add(chunk.len())
            .is_none_or(|size| size > MAX_QUOTE_RESPONSE_BYTES)
        {
            return Err(response_too_large());
        }
        bytes.extend_from_slice(&chunk);
    }
    let payload: TushareResponse = serde_json::from_slice(&bytes).map_err(|_| {
        CommandError::new("QUOTE_RESPONSE_ERROR", "行情响应格式无效")
            .with_recovery("最近一次价格未被修改，可稍后重试或使用手工价格。")
    })?;
    if payload.code != 0 {
        return Err(
            CommandError::new("QUOTE_PROVIDER_ERROR", "行情服务拒绝请求").with_recovery(
                "请检查 Token、积分与接口权限；基金净值权限不足时可手工录入，最近一次价格已保留。",
            ),
        );
    }
    Ok(payload)
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
    endpoint: &str,
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
        .post(endpoint)
        .json(&json!({
            "api_name": api_name, "token": token,
            "params": {"ts_code": tushare_code(instrument), "start_date": start, "end_date": end},
            "fields": format!("{date_field},{price_field}")
        }))
        .send()
        .await
        .map_err(network_error)?;
    let payload = read_quote_response(response).await?;
    extract_price(payload, price_field, date_field)
}

fn extract_price(
    payload: TushareResponse,
    price_field: &str,
    date_field: &str,
) -> CommandResult<(String, String)> {
    let data = payload
        .data
        .ok_or_else(|| CommandError::new("QUOTE_EMPTY", "没有可用行情"))?;
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
    let row = values
        .first()
        .ok_or_else(|| CommandError::new("QUOTE_EMPTY", "没有可用行情"))?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
        thread,
    };

    struct LocalServer {
        url: String,
        requests: Arc<AtomicUsize>,
        stop: Arc<AtomicBool>,
        worker: Option<thread::JoinHandle<()>>,
    }

    impl LocalServer {
        fn new(response: Vec<u8>) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let url = format!("http://{}", listener.local_addr().unwrap());
            let requests = Arc::new(AtomicUsize::new(0));
            let count = requests.clone();
            let stop = Arc::new(AtomicBool::new(false));
            let stopped = stop.clone();
            let worker = thread::spawn(move || {
                while !stopped.load(Ordering::Acquire) {
                    let (mut stream, _) = match listener.accept() {
                        Ok(pair) => pair,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(std::time::Duration::from_millis(5));
                            continue;
                        }
                        Err(error) => panic!("local test accept: {error}"),
                    };
                    count.fetch_add(1, Ordering::SeqCst);
                    // macOS can inherit O_NONBLOCK from the listening socket.
                    // Keep accept polling nonblocking, but fully send/read each
                    // accepted test connection under its bounded timeouts.
                    stream.set_nonblocking(false).unwrap();
                    stream
                        .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                        .unwrap();
                    stream
                        .set_write_timeout(Some(std::time::Duration::from_secs(2)))
                        .unwrap();
                    let mut request = Vec::new();
                    let mut buffer = [0; 1024];
                    while !request.windows(4).any(|value| value == b"\r\n\r\n") {
                        match stream.read(&mut buffer) {
                            Ok(0) | Err(_) => break,
                            Ok(length) => request.extend_from_slice(&buffer[..length]),
                        }
                    }
                    if let Some(end) = request.windows(4).position(|value| value == b"\r\n\r\n") {
                        let header_end = end + 4;
                        let content_length = String::from_utf8_lossy(&request[..end])
                            .lines()
                            .find_map(|line| {
                                line.split_once(':')
                                    .filter(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                            })
                            .and_then(|(_, value)| value.trim().parse::<usize>().ok())
                            .unwrap_or(0);
                        assert!(content_length < 1024);
                        while request.len() < header_end + content_length {
                            match stream.read(&mut buffer) {
                                Ok(0) | Err(_) => break,
                                Ok(length) => request.extend_from_slice(&buffer[..length]),
                            }
                        }
                    }
                    // A rejected/oversized response can make the client close early.
                    let _ = stream.write_all(&response);
                }
            });
            Self {
                url,
                requests,
                stop,
                worker: Some(worker),
            }
        }

        fn json(body: &[u8], content_length: bool) -> Self {
            let mut response =
                b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n"
                    .to_vec();
            if content_length {
                response
                    .extend_from_slice(format!("Content-Length: {}\r\n", body.len()).as_bytes());
            }
            response.extend_from_slice(b"\r\n");
            response.extend_from_slice(body);
            Self::new(response)
        }
    }

    impl Drop for LocalServer {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Release);
            self.worker.take().unwrap().join().unwrap();
        }
    }

    fn local_response(server: &LocalServer) -> CommandResult<TushareResponse> {
        // Only the test client relaxes HTTPS for loopback. It retains the
        // production redirect, proxy and timeout policies; no provider is used.
        tauri::async_runtime::block_on(async {
            let client = quote_client_builder().https_only(false).build().unwrap();
            let response = client
                .post(&server.url)
                .body("fictional-test-token")
                .send()
                .await
                .map_err(network_error)?;
            read_quote_response(response).await
        })
    }

    fn fixture() -> (tempfile::TempDir, Arc<AppState>) {
        let directory = tempfile::tempdir().unwrap();
        let state = Arc::new(AppState::new(directory.path().join("data")).unwrap());
        let key = vec![73; 32];
        let connection = crate::database::open_database(&state.database_path, &key).unwrap();
        {
            let mut runtime = state.runtime.lock().unwrap();
            runtime.connection = Some(connection);
            runtime.key = Some(Zeroizing::new(key));
        }
        state.session.activate(0, 0).unwrap();
        let mut settings = state.with_connection(repository::settings).unwrap();
        settings.quote_enabled = true;
        settings.lock_minutes = 0;
        update_settings(&state, settings).unwrap();
        state
            .with_connection(|db| {
                for code in ["600001", "600002"] {
                    repository::upsert_instrument(
                        db,
                        Instrument {
                            id: String::new(),
                            code: code.into(),
                            name: code.into(),
                            kind: "stock".into(),
                            market: "CN".into(),
                            currency: "CNY".into(),
                            manual_price: None,
                            created_at: String::new(),
                            updated_at: String::new(),
                        },
                    )?;
                }
                Ok(())
            })
            .unwrap();
        (directory, state)
    }

    fn set_enabled(state: &AppState, enabled: bool) {
        let mut settings = state.with_connection(repository::settings).unwrap();
        settings.quote_enabled = enabled;
        update_settings(state, settings).unwrap();
    }

    fn assert_no_quote_writes(state: &AppState) {
        state
            .with_connection(|db| {
                assert_eq!(
                    db.query_row("SELECT count(*) FROM price_points", [], |row| row
                        .get::<_, i64>(0))?,
                    0
                );
                assert!(repository::settings(db)?.last_quote_refresh.is_none());
                Ok(())
            })
            .unwrap();
    }

    // Completes N requests, then stalls either before headers or mid-body until the client drops IO.
    struct StalledServer {
        url: String,
        arrived: std::sync::mpsc::Receiver<()>,
        closed: std::sync::mpsc::Receiver<()>,
        stop: Arc<AtomicBool>,
        worker: Option<thread::JoinHandle<()>>,
    }

    impl StalledServer {
        fn new(completed: usize, partial_body: bool) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let url = format!("http://{}", listener.local_addr().unwrap());
            let stop = Arc::new(AtomicBool::new(false));
            let stopped = stop.clone();
            let (arrived_tx, arrived) = std::sync::mpsc::channel();
            let (closed_tx, closed) = std::sync::mpsc::channel();
            let worker = thread::spawn(move || {
                let mut seen = 0;
                while !stopped.load(Ordering::Acquire) {
                    let Ok((mut socket, _)) = listener.accept() else {
                        thread::sleep(std::time::Duration::from_millis(5));
                        continue;
                    };
                    socket.set_nonblocking(false).unwrap();
                    socket
                        .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                        .unwrap();
                    socket
                        .set_write_timeout(Some(std::time::Duration::from_secs(2)))
                        .unwrap();
                    let mut request = vec![];
                    let mut buffer = [0; 4096];
                    loop {
                        let size = socket.read(&mut buffer).unwrap();
                        assert!(size != 0);
                        request.extend_from_slice(&buffer[..size]);
                        if let Some(end) =
                            request.windows(4).position(|window| window == b"\r\n\r\n")
                        {
                            let length = String::from_utf8_lossy(&request[..end])
                                .lines()
                                .find_map(|line| {
                                    line.to_lowercase()
                                        .strip_prefix("content-length:")
                                        .map(|value| value.trim().parse::<usize>().unwrap())
                                })
                                .unwrap_or(0);
                            if request.len() >= end + 4 + length {
                                break;
                            }
                        }
                    }
                    if seen < completed {
                        seen += 1;
                        let body = br#"{"code":0,"data":{"fields":["trade_date","close"],"items":[["20261003","3.25"]]}}"#;
                        socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).as_bytes()).unwrap();
                        socket.write_all(body).unwrap();
                        continue;
                    }
                    if partial_body {
                        socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4096\r\nConnection: close\r\n\r\n{\"code\":0,").unwrap();
                    }
                    socket
                        .set_read_timeout(Some(std::time::Duration::from_millis(50)))
                        .unwrap();
                    arrived_tx.send(()).unwrap();
                    while !stopped.load(Ordering::Acquire) {
                        match socket.read(&mut buffer) {
                            Ok(0) => {
                                let _ = closed_tx.send(());
                                break;
                            }
                            Err(error)
                                if matches!(
                                    error.kind(),
                                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                                ) => {}
                            Err(_) => {
                                let _ = closed_tx.send(());
                                break;
                            }
                            Ok(_) => {}
                        }
                    }
                    break;
                }
            });
            Self {
                url,
                arrived,
                closed,
                stop,
                worker: Some(worker),
            }
        }

        fn wait_arrived(&self) {
            self.arrived
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
        }

        fn wait_closed(&self) {
            self.closed
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
        }
    }

    impl Drop for StalledServer {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Release);
            self.worker.take().unwrap().join().unwrap();
        }
    }

    fn spawn_refresh(
        state: Arc<AppState>,
        endpoint: String,
    ) -> (
        thread::JoinHandle<()>,
        std::sync::mpsc::Receiver<CommandResult<QuoteRefreshReport>>,
    ) {
        let (tx, rx) = std::sync::mpsc::channel();
        let worker = thread::spawn(move || {
            let result = tauri::async_runtime::block_on(async {
                let (job, instruments) = begin_refresh(&state)?;
                let client = quote_client_builder().https_only(false).build().unwrap();
                run_refresh(
                    &state,
                    &job,
                    &client,
                    "fictional-test-token",
                    instruments,
                    &endpoint,
                )
                .await
            });
            let _ = tx.send(result);
        });
        (worker, rx)
    }

    #[test]
    fn quote_disable_cancels_stalled_headers_and_body_without_waiting_for_http_timeout() {
        for partial_body in [false, true] {
            let (_directory, state) = fixture();
            let server = StalledServer::new(1, partial_body);
            let (worker, result) = spawn_refresh(state.clone(), server.url.clone());
            server.wait_arrived();
            set_enabled(&state, false);
            assert_eq!(
                result
                    .recv_timeout(std::time::Duration::from_secs(5))
                    .unwrap()
                    .unwrap_err()
                    .code,
                "QUOTES_CANCELLED"
            );
            worker.join().unwrap();
            server.wait_closed();
            assert_no_quote_writes(&state); // The first successful price remains uncommitted.
        }
    }

    #[test]
    fn quote_session_events_and_idle_deadline_wake_stalled_io_without_database_lock() {
        for reason in ["manual", "system", "idle", "shutdown"] {
            let (_directory, state) = fixture();
            let server = StalledServer::new(0, true);
            let (worker, result) = spawn_refresh(state.clone(), server.url.clone());
            server.wait_arrived();
            // Revocation must wake the HTTP future even while SQL is busy.
            let database_guard = state.runtime.lock().unwrap();
            match reason {
                "system" => {
                    state
                        .session
                        .system_event(crate::session::SystemEvent::ScreenLocked);
                }
                "idle" => {
                    state.session.configure(5).unwrap();
                    state.session.age_for_test(301);
                    assert!(!state.session.snapshot().unlocked);
                }
                _ => {
                    state.session.revoke(reason);
                }
            }
            assert_eq!(
                result
                    .recv_timeout(std::time::Duration::from_secs(5))
                    .unwrap()
                    .unwrap_err()
                    .code,
                "APP_LOCKED"
            );
            worker.join().unwrap();
            server.wait_closed();
            assert_eq!(
                database_guard
                    .connection
                    .as_ref()
                    .unwrap()
                    .query_row("SELECT count(*) FROM price_points", [], |r| r
                        .get::<_, i64>(0))
                    .unwrap(),
                0
            );
        }
    }

    #[test]
    fn quote_off_on_and_reunlock_never_revive_old_jobs() {
        let (_directory, state) = fixture();
        let (old, instruments) = begin_refresh(&state).unwrap();
        set_enabled(&state, false);
        set_enabled(&state, true);
        let polled = AtomicBool::new(false);
        let result = tauri::async_runtime::block_on(old.run(&state.session, async {
            polled.store(true, Ordering::SeqCst);
            Ok(())
        }));
        assert_eq!(result.unwrap_err().code, "QUOTES_CANCELLED");
        assert!(!polled.load(Ordering::SeqCst));
        assert_eq!(
            commit_prices(
                &state,
                &old,
                vec![PricePointInput {
                    instrument_id: instruments[0].id.clone(),
                    price_date: "2026-10-03".into(),
                    price: "3".into(),
                    source: "tushare".into()
                }],
                vec![]
            )
            .unwrap_err()
            .code,
            "QUOTES_CANCELLED"
        );
        drop(old);
        let (old, _) = begin_refresh(&state).unwrap();
        state.session.revoke("manual");
        state
            .session
            .activate(state.session.challenge().unwrap(), 0)
            .unwrap();
        assert_eq!(old.check(&state.session).unwrap_err().code, "APP_LOCKED");
        drop(old);
        assert!(begin_refresh(&state).is_ok());
        assert_no_quote_writes(&state);
    }

    #[test]
    fn quote_restore_scope_cancels_io_blocks_new_jobs_and_releases_on_drop() {
        let (_directory, state) = fixture();
        let server = StalledServer::new(0, false);
        let (worker, result) = spawn_refresh(state.clone(), server.url.clone());
        server.wait_arrived();
        let outer = state.quotes.restoring();
        let inner = state.quotes.restoring();
        assert_eq!(
            result
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap()
                .unwrap_err()
                .code,
            "QUOTES_CANCELLED"
        );
        worker.join().unwrap();
        server.wait_closed();
        assert_eq!(
            begin_refresh(&state).err().unwrap().code,
            "QUOTES_RESTORE_IN_PROGRESS"
        );
        drop(inner);
        assert_eq!(
            begin_refresh(&state).err().unwrap().code,
            "QUOTES_RESTORE_IN_PROGRESS"
        );
        drop(outer);
        assert!(begin_refresh(&state).is_ok());
        assert_no_quote_writes(&state);
    }

    #[test]
    fn quote_failed_backup_and_snapshot_restore_still_invalidate_old_results() {
        let (_directory, state) = fixture();
        for snapshot in [false, true] {
            let (job, _) = begin_refresh(&state).unwrap();
            if snapshot {
                assert!(
                    crate::backup::restore_snapshot(&state, "daily-2099-01-01.sqlite3").is_err()
                );
            } else {
                assert!(
                    crate::backup::restore_backup(
                        &state,
                        &state.data_dir.join("missing.workbench-backup"),
                        "fictional-password"
                    )
                    .is_err()
                );
            }
            assert_eq!(
                job.check(&state.session).unwrap_err().code,
                "QUOTES_CANCELLED"
            );
            assert_eq!(
                commit_prices(&state, &job, vec![], vec![])
                    .unwrap_err()
                    .code,
                "QUOTES_CANCELLED"
            );
            drop(job);
            assert!(begin_refresh(&state).is_ok());
        }
        assert_no_quote_writes(&state);
    }

    #[test]
    fn quote_successful_restore_rejects_old_batch_and_allows_fresh_refresh() {
        let (_directory, state) = fixture();
        let name = "daily-2099-01-01.sqlite3";
        state
            .with_connection(|db| {
                crate::database::create_encrypted_snapshot(
                    db,
                    &[73; 32],
                    &state.backup_dir.join(name),
                )
            })
            .unwrap();
        let (old, instruments) = begin_refresh(&state).unwrap();
        crate::backup::restore_snapshot(&state, name).unwrap();
        let price = PricePointInput {
            instrument_id: instruments[0].id.clone(),
            price_date: "2026-10-03".into(),
            price: "3".into(),
            source: "tushare".into(),
        };
        assert_eq!(
            commit_prices(&state, &old, vec![price], vec![])
                .unwrap_err()
                .code,
            "QUOTES_CANCELLED"
        );
        drop(old);
        assert!(begin_refresh(&state).is_ok());
        assert_no_quote_writes(&state);
    }

    #[test]
    fn quote_duplicate_refresh_is_rejected_and_permit_is_released_after_failure() {
        let (_directory, state) = fixture();
        let (job, _) = begin_refresh(&state).unwrap();
        assert_eq!(
            begin_refresh(&state).err().unwrap().code,
            "QUOTE_REFRESH_BUSY"
        );
        drop(job);
        assert!(begin_refresh(&state).is_ok());
        set_enabled(&state, false);
        assert_eq!(begin_refresh(&state).err().unwrap().code, "QUOTES_DISABLED");
    }

    #[test]
    fn quote_price_batch_rolls_back_all_rows_and_timestamp_when_a_later_price_is_invalid() {
        let (_directory, state) = fixture();
        let (job, instruments) = begin_refresh(&state).unwrap();
        let prices = instruments
            .iter()
            .enumerate()
            .map(|(index, item)| PricePointInput {
                instrument_id: item.id.clone(),
                price_date: "2026-10-03".into(),
                price: if index == 0 { "3" } else { "invalid" }.into(),
                source: "tushare".into(),
            })
            .collect();
        assert!(commit_prices(&state, &job, prices, vec![]).is_err());
        assert_no_quote_writes(&state);
    }

    #[test]
    fn quote_successful_batch_keeps_provider_failures_and_preserves_current_settings() {
        let (_directory, state) = fixture();
        let (job, instruments) = begin_refresh(&state).unwrap();
        let mut settings = state.with_connection(repository::settings).unwrap();
        settings.theme = "dark".into();
        update_settings(&state, settings).unwrap();
        let price = PricePointInput {
            instrument_id: instruments[0].id.clone(),
            price_date: "2026-10-03".into(),
            price: "3.25".into(),
            source: "tushare".into(),
        };
        let report = commit_prices(
            &state,
            &job,
            vec![price],
            vec!["fictional-provider-failure".into()],
        )
        .unwrap();
        assert_eq!((report.updated, report.failed), (1, 1));
        state
            .with_connection(|db| {
                let settings = repository::settings(db)?;
                assert_eq!(settings.theme, "dark");
                assert!(settings.quote_enabled);
                assert_eq!(
                    settings.last_quote_refresh.as_deref(),
                    Some(report.refreshed_at.as_str())
                );
                assert_eq!(
                    db.query_row("SELECT count(*) FROM price_points", [], |r| r
                        .get::<_, i64>(0))?,
                    1
                );
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn quote_invalid_settings_do_not_cancel_an_authorized_job() {
        let (_directory, state) = fixture();
        let (job, _) = begin_refresh(&state).unwrap();
        let mut invalid = state.with_connection(repository::settings).unwrap();
        invalid.quote_enabled = false;
        invalid.lock_minutes = 7;
        assert!(update_settings(&state, invalid).is_err());
        job.check(&state.session).unwrap();
        assert!(
            state
                .with_connection(repository::settings)
                .unwrap()
                .quote_enabled
        );
    }

    #[test]
    fn quote_dropped_future_releases_task_wakers_and_refresh_permit() {
        struct WakeCounter(AtomicBool);
        impl std::task::Wake for WakeCounter {
            fn wake(self: Arc<Self>) {
                self.0.store(true, Ordering::SeqCst);
            }
        }
        let (_directory, state) = fixture();
        let (job, _) = begin_refresh(&state).unwrap();
        let owner = Arc::new(WakeCounter(AtomicBool::new(false)));
        let waker = std::task::Waker::from(owner.clone());
        let baseline = Arc::strong_count(&owner);
        let mut cx = std::task::Context::from_waker(&waker);
        let mut future =
            Box::pin(job.run(&state.session, std::future::pending::<CommandResult<()>>()));
        assert!(future.as_mut().poll(&mut cx).is_pending());
        assert!(Arc::strong_count(&owner) > baseline);
        drop(future);
        drop(job);
        assert_eq!(Arc::strong_count(&owner), baseline);
        assert!(!owner.0.load(Ordering::SeqCst));
        assert!(begin_refresh(&state).is_ok());
    }

    #[test]
    fn quote_production_endpoint_is_fixed_https_and_http_is_rejected_before_connecting() {
        let url = reqwest::Url::parse(TUSHARE_ENDPOINT).unwrap();
        assert_eq!(url.scheme(), "https");
        assert_eq!(url.host_str(), Some("api.tushare.pro"));
        assert_eq!(url.port_or_known_default(), Some(443));
        assert_eq!(url.username(), "");
        assert!(url.password().is_none() && url.query().is_none() && url.fragment().is_none());
        let server = LocalServer::json(br#"{"code":0,"data":null}"#, true);
        let client = build_quote_client().unwrap();
        assert!(tauri::async_runtime::block_on(client.get(&server.url).send()).is_err());
        assert_eq!(server.requests.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn quote_redirects_never_forward_credentials_to_another_endpoint() {
        let target = LocalServer::json(br#"{"code":0,"data":null}"#, true);
        for status in [301, 302, 303, 307, 308] {
            let response = format!(
                "HTTP/1.1 {status} Redirect\r\nLocation: {}/fictional-test-token\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                target.url
            );
            let source = LocalServer::new(response.into_bytes());
            let error = local_response(&source).unwrap_err();
            assert_eq!(error.code, "QUOTE_REDIRECT_BLOCKED");
            assert!(
                !serde_json::to_string(&error)
                    .unwrap()
                    .contains("fictional-test-token")
            );
            assert_eq!(source.requests.load(Ordering::SeqCst), 1);
            assert_eq!(target.requests.load(Ordering::SeqCst), 0);
        }
    }

    #[test]
    fn quote_response_bounds_cover_headers_unknown_length_and_exact_limit() {
        let server = LocalServer::new(
            format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{{}}",
                MAX_QUOTE_RESPONSE_BYTES + 1
            )
            .into_bytes(),
        );
        assert_eq!(
            local_response(&server).unwrap_err().code,
            "QUOTE_RESPONSE_TOO_LARGE"
        );
        let mut bytes = br#"{"code":0,"data":null}"#.to_vec();
        bytes.resize(MAX_QUOTE_RESPONSE_BYTES, b' ');
        let exact = LocalServer::json(&bytes, false);
        assert_eq!(local_response(&exact).unwrap().code, 0);
        bytes.push(b' ');
        for content_length in [false, true] {
            let oversized = LocalServer::json(&bytes, content_length);
            assert_eq!(
                local_response(&oversized).unwrap_err().code,
                "QUOTE_RESPONSE_TOO_LARGE"
            );
        }
    }

    #[test]
    fn quote_chunked_response_cannot_bypass_actual_byte_limit() {
        let bytes = vec![b' '; MAX_QUOTE_RESPONSE_BYTES + 1];
        let mut response =
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n".to_vec();
        for chunk in bytes.chunks(8192) {
            response.extend_from_slice(format!("{:x}\r\n", chunk.len()).as_bytes());
            response.extend_from_slice(chunk);
            response.extend_from_slice(b"\r\n");
        }
        response.extend_from_slice(b"0\r\n\r\n");
        assert_eq!(
            local_response(&LocalServer::new(response))
                .unwrap_err()
                .code,
            "QUOTE_RESPONSE_TOO_LARGE"
        );
    }

    #[test]
    fn quote_errors_do_not_echo_provider_messages_or_malformed_json() {
        for (bytes, code) in [
            (br#"{"code":987654321,"msg":"fictional-test-token and private holdings 987654321","data":null}"#.as_slice(), "QUOTE_PROVIDER_ERROR"),
            (br#"{"code":"fictional-test-token","data":null}"#.as_slice(), "QUOTE_RESPONSE_ERROR"),
            (br#"fictional-test-token not JSON"#.as_slice(), "QUOTE_RESPONSE_ERROR"),
        ] {
            let error = local_response(&LocalServer::json(bytes, true)).unwrap_err();
            assert_eq!(error.code, code);
            let serialized = serde_json::to_string(&error).unwrap();
            assert!(!serialized.contains("fictional-test-token") && !serialized.contains("987654321"));
        }
        let server = LocalServer::new(
            b"HTTP/1.1 403 fictional-test-token\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                .to_vec(),
        );
        let error = local_response(&server).unwrap_err();
        assert_eq!(error.code, "QUOTE_HTTP_ERROR");
        assert!(
            !serde_json::to_string(&error)
                .unwrap()
                .contains("fictional-test-token")
        );
    }

    #[test]
    fn quote_normal_payload_still_extracts_latest_fund_and_market_prices() {
        let exact: TushareResponse = serde_json::from_str(r#"{"code":0,"data":{"fields":["trade_date","close"],"items":[["20260829",9007199254740993.123456789]]}}"#).unwrap();
        assert_eq!(
            extract_price(exact, "close", "trade_date").unwrap().1,
            "9007199254740993.123456789"
        );
        for (fields, rows, price_field, date_field) in [
            (
                vec!["trade_date", "close"],
                serde_json::json!([["20261001", 4.25], ["20261002", "4.5678"]]),
                "close",
                "trade_date",
            ),
            (
                vec!["unit_nav", "nav_date"],
                serde_json::json!([["1.23456789", "20261002"], ["1.0", "20261001"]]),
                "unit_nav",
                "nav_date",
            ),
        ] {
            let body = serde_json::to_vec(&json!({"code":0,"data":{"fields":fields,"items":rows}}))
                .unwrap();
            let payload = local_response(&LocalServer::json(&body, true)).unwrap();
            let (date, price) = extract_price(payload, price_field, date_field).unwrap();
            assert_eq!(date, "2026-10-02");
            assert_eq!(
                price,
                if price_field == "close" {
                    "4.5678"
                } else {
                    "1.23456789"
                }
            );
        }
    }

    #[test]
    fn quote_proxy_child() {
        let Ok(url) = std::env::var("WORKBENCH_QUOTE_TEST_URL") else {
            return;
        };
        tauri::async_runtime::block_on(async {
            let client = quote_client_builder().https_only(false).build().unwrap();
            let response = client.get(url).send().await.unwrap();
            assert_eq!(read_quote_response(response).await.unwrap().code, 0);
        });
    }

    #[test]
    fn quote_proxy_environment_is_ignored_without_mutating_parent_environment() {
        let server = LocalServer::json(br#"{"code":0,"data":null}"#, true);
        let mut child = std::process::Command::new(std::env::current_exe().unwrap());
        child
            .args(["--exact", "quotes::tests::quote_proxy_child", "--nocapture"])
            .env("WORKBENCH_QUOTE_TEST_URL", &server.url);
        for variable in [
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "ALL_PROXY",
            "http_proxy",
            "https_proxy",
            "all_proxy",
        ] {
            child.env(variable, "http://127.0.0.1:1");
        }
        child.env("NO_PROXY", "").env("no_proxy", "");
        let output = child.output().unwrap();
        assert!(
            output.status.success(),
            "isolated proxy test: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(server.requests.load(Ordering::SeqCst), 1);
    }
}
