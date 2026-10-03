use std::{
    collections::BTreeMap,
    fs,
    io::{Cursor, Read, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::Mutex,
    sync::atomic::AtomicBool,
};

use age::{Decryptor, Encryptor, Identity, secrecy::SecretString};
use chrono::{Datelike, SecondsFormat, Utc};
use rand::RngCore;
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use security_framework::{
    access_control::{ProtectionMode, SecAccessControl},
    passwords::{
        AccessControlOptions, PasswordOptions, generic_password, set_generic_password_options,
    },
};
use zeroize::Zeroizing;

use crate::{
    error::{CommandError, CommandResult},
    models::{AppSettings, SecurityStatus},
};

const KEYCHAIN_SERVICE: &str = "com.local.personalworkbench";
const DB_KEY_ACCOUNT: &str = "database-key";
const TUSHARE_TOKEN_ACCOUNT: &str = "tushare-token";
const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;
const ERR_SEC_MISSING_ENTITLEMENT: i32 = -34018;
const KEYCHAIN_MODE_FILE: &str = "keychain-mode-login";
const PASSWORD_KEY_FILE: &str = "database-key.age";
const LOCAL_PASSWORD_MIN_CHARS: usize = 6;
pub(crate) const LATEST_SCHEMA_VERSION: i64 = 2;
const BUSINESS_TABLES: &[&str] = &[
    "tasks",
    "calendar_items",
    "projects",
    "work_logs",
    "goals",
    "habits",
    "habit_checks",
    "learning_items",
    "content_items",
    "content_metrics",
    "investment_accounts",
    "instruments",
    "portfolio_transactions",
    "price_points",
    "review_snapshots",
];

pub struct RuntimeState {
    pub connection: Option<Connection>,
    pub key: Option<Zeroizing<Vec<u8>>>,
    pub snapshot_warning: Option<String>,
    pub recovery_notice: Option<String>,
    pub(crate) snapshot_stamp: Option<(std::time::Instant, u64, chrono::NaiveDate)>,
}

pub struct AppState {
    pub runtime: Mutex<RuntimeState>,
    authentication: Mutex<()>,
    pub data_dir: PathBuf,
    pub database_path: PathBuf,
    pub backup_dir: PathBuf,
    pub(crate) session: crate::session::SessionPolicy,
    pub(crate) quotes: crate::quotes::QuoteControl,
    pub(crate) stopping: AtomicBool,
    _instance: crate::instance::InstanceLease,
}

impl AppState {
    pub fn new(data_dir: PathBuf) -> CommandResult<Self> {
        fs::create_dir_all(&data_dir)?;
        let instance = crate::instance::InstanceLease::acquire(&data_dir)?;
        fs::set_permissions(&data_dir, fs::Permissions::from_mode(0o700))?;
        let backup_dir = data_dir.join("backups");
        fs::create_dir_all(&backup_dir)?;
        fs::set_permissions(&backup_dir, fs::Permissions::from_mode(0o700))?;
        let database_path = data_dir.join("workbench.sqlite3");
        restrict_database_permissions(&database_path)?;
        for entry in fs::read_dir(&backup_dir)? {
            let path = entry?.path();
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.file_type().is_file() && !metadata.file_type().is_symlink() {
                restrict_file_permissions(&path)?;
            }
        }
        prune_snapshots(&backup_dir, "pre-password-change-", 5)?;
        prune_snapshots(&backup_dir, "pre-restore-", 5)?;
        prune_snapshots(&backup_dir, "pre-migration-", 5)?;
        Ok(Self {
            _instance: instance,
            database_path,
            data_dir,
            backup_dir,
            session: crate::session::SessionPolicy::new(),
            quotes: crate::quotes::QuoteControl::default(),
            stopping: AtomicBool::new(false),
            authentication: Mutex::new(()),
            runtime: Mutex::new(RuntimeState {
                connection: None,
                key: None,
                snapshot_warning: None,
                recovery_notice: None,
                snapshot_stamp: None,
            }),
        })
    }

    pub fn status(&self) -> SecurityStatus {
        let mut runtime = self
            .runtime
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if runtime.connection.is_none() && self.session.snapshot().unlocked {
            self.session.revoke("databaseUnavailable");
        }
        if !self.session.snapshot().unlocked {
            Self::close_runtime(&mut runtime);
        }
        self.status_for_runtime(&runtime)
    }

    fn status_for_runtime(&self, runtime: &RuntimeState) -> SecurityStatus {
        let session = self.session.snapshot();
        SecurityStatus {
            initialized: self.has_existing_database()
                || self.data_dir.join(PASSWORD_KEY_FILE).exists(),
            unlocked: runtime.connection.is_some() && session.unlocked,
            database_path: self.database_path.to_string_lossy().to_string(),
            keychain_mode: self.keychain_mode(),
            snapshot_warning: runtime.snapshot_warning.clone(),
            recovery_notice: runtime.recovery_notice.clone(),
            session_epoch: session.session_epoch,
            lock_reason: session.lock_reason,
        }
    }

    fn has_existing_database(&self) -> bool {
        fs::symlink_metadata(&self.database_path).is_ok()
            || crate::restore::has_restore_artifacts(&self.data_dir)
    }

    fn keychain_mode(&self) -> String {
        if self.data_dir.join(PASSWORD_KEY_FILE).exists() {
            "passphrase".into()
        } else if self.data_dir.join(KEYCHAIN_MODE_FILE).exists() {
            "loginKeychain".into()
        } else {
            "userPresence".into()
        }
    }

    fn authentication_guard(&self) -> CommandResult<std::sync::MutexGuard<'_, ()>> {
        self.authentication.try_lock().map_err(|_| {
            CommandError::new("AUTHENTICATION_IN_PROGRESS", "另一项认证操作尚未结束")
                .with_recovery("请等待当前解锁或口令修改完成后再试。")
        })
    }

    pub fn unlock(&self) -> CommandResult<SecurityStatus> {
        let _authentication = self.authentication_guard()?;
        let ticket = self.session.challenge()?;
        if self.data_dir.join(PASSWORD_KEY_FILE).exists() {
            return Err(CommandError::new(
                "PASSWORD_REQUIRED",
                "此工作台使用本地口令保护数据库密钥",
            ));
        }
        let mut runtime = self
            .runtime
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if runtime.connection.is_some() {
            if self.session.require_active().is_ok() {
                return Ok(self.status_for_runtime(&runtime));
            }
            Self::close_runtime(&mut runtime);
        }

        let key = Zeroizing::new(match read_database_key() {
            Ok(key) => key,
            Err(error)
                if error.code() == ERR_SEC_ITEM_NOT_FOUND && !self.has_existing_database() =>
            {
                create_database_key(&self.data_dir)?
            }
            Err(error) => {
                return Err(CommandError::new(
                    "KEYCHAIN_UNLOCK_FAILED",
                    format!("无法从 macOS 钥匙串读取数据库密钥：{error}"),
                )
                .with_recovery("请完成 Touch ID 或系统密码验证后重试；不要删除现有数据库文件。"));
            }
        });

        runtime.recovery_notice = crate::restore::recover_interrupted_restore(
            &self.data_dir,
            &self.database_path,
            &self.backup_dir,
            &key,
        )?;
        let connection =
            open_database_with_migration_snapshot(&self.database_path, &key, &self.backup_dir)?;
        let epoch = self
            .session
            .activate(ticket, load_settings(&connection)?.lock_minutes)?;
        runtime.key = Some(key);
        runtime.connection = Some(connection);

        runtime.snapshot_warning =
            if let (Some(connection), Some(key)) = (&runtime.connection, &runtime.key) {
                create_automatic_snapshots(connection, key, &self.backup_dir)
                    .err()
                    .map(|error| format!("自动快照未能完成：{}", error.message))
            } else {
                None
            };

        if let Err(error) = self.session.require_epoch(epoch) {
            Self::close_runtime(&mut runtime);
            return Err(error);
        }
        Ok(self.status_for_runtime(&runtime))
    }

    pub fn initialize_with_password(&self, password: &str) -> CommandResult<SecurityStatus> {
        let _authentication = self.authentication_guard()?;
        let ticket = self.session.challenge()?;
        if self.has_existing_database() {
            return Err(CommandError::new(
                "ALREADY_INITIALIZED",
                "工作台已经初始化，不能重新生成数据库密钥",
            ));
        }
        if fs::symlink_metadata(self.data_dir.join(PASSWORD_KEY_FILE)).is_ok() {
            return Err(CommandError::new(
                "LOCAL_KEY_ALREADY_EXISTS",
                "已存在口令保护的数据库密钥，不能重新生成",
            )
            .with_recovery(
                "请取消初始化并用原工作台口令解锁，继续完成未结束的初始化；不要删除密钥文件。",
            ));
        }
        let mut key = Zeroizing::new(vec![0_u8; 32]);
        rand::rng().fill_bytes(key.as_mut_slice());
        store_password_protected_key(&self.data_dir, &key, password)?;
        self.open_with_key(key.to_vec(), ticket)
    }

    pub fn unlock_with_password(&self, password: &str) -> CommandResult<SecurityStatus> {
        let _authentication = self.authentication_guard()?;
        let ticket = self.session.challenge()?;
        let key = read_password_protected_key(&self.data_dir, password)?;
        self.open_with_key(key, ticket)
    }

    pub fn change_local_password(
        &self,
        current_password: &str,
        new_password: &str,
    ) -> CommandResult<SecurityStatus> {
        let _authentication = self.authentication_guard()?;
        let epoch = self.session.require_active()?;
        validate_local_password(current_password)?;
        validate_local_password(new_password)?;
        if current_password == new_password {
            return Err(CommandError::new(
                "PASSWORD_UNCHANGED",
                "新工作台口令不能与当前口令相同",
            ));
        }
        if !self.data_dir.join(PASSWORD_KEY_FILE).exists() {
            return Err(CommandError::new(
                "PASSWORD_CHANGE_UNAVAILABLE",
                "当前数据库密钥由 macOS 钥匙串保护，没有可修改的本地工作台口令",
            )
            .with_recovery("请继续使用 Touch ID、系统密码或当前用户的登录钥匙串解锁。"));
        }

        let decrypted_key = Zeroizing::new(read_password_protected_key(
            &self.data_dir,
            current_password,
        )?);
        let mut runtime = self
            .runtime
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        self.session.require_epoch(epoch)?;
        let active_key = runtime.key.as_deref().ok_or_else(CommandError::locked)?;
        let connection = runtime
            .connection
            .as_ref()
            .ok_or_else(CommandError::locked)?;
        if decrypted_key.as_slice() != active_key {
            return Err(CommandError::new(
                "LOCAL_KEY_MISMATCH",
                "当前口令对应的密钥与已打开数据库不一致",
            )
            .with_recovery("数据库和密钥文件均未修改，请重新启动工作台后再试。"));
        }

        connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        let timestamp = Utc::now().format("%Y%m%dT%H%M%S%.3fZ");
        let recovery_path = self
            .backup_dir
            .join(format!("pre-password-change-{timestamp}.sqlite3"));
        create_encrypted_snapshot(connection, &decrypted_key, &recovery_path)?;
        prune_snapshots(&self.backup_dir, "pre-password-change-", 5)?;
        store_password_protected_key(&self.data_dir, &decrypted_key, new_password)?;

        self.session.revoke("passwordChanged");
        runtime.connection = None;
        runtime.key = None;
        runtime.snapshot_warning = None;
        Ok(self.status_for_runtime(&runtime))
    }

    fn open_with_key(&self, key: Vec<u8>, ticket: u64) -> CommandResult<SecurityStatus> {
        let key = Zeroizing::new(key);
        let mut runtime = self
            .runtime
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if runtime.connection.is_some() {
            if self.session.require_active().is_ok() {
                return Ok(self.status_for_runtime(&runtime));
            }
            Self::close_runtime(&mut runtime);
        }
        runtime.recovery_notice = crate::restore::recover_interrupted_restore(
            &self.data_dir,
            &self.database_path,
            &self.backup_dir,
            &key,
        )?;
        let connection =
            open_database_with_migration_snapshot(&self.database_path, &key, &self.backup_dir)?;
        let epoch = self
            .session
            .activate(ticket, load_settings(&connection)?.lock_minutes)?;
        runtime.key = Some(key);
        runtime.connection = Some(connection);
        runtime.snapshot_warning =
            if let (Some(connection), Some(key)) = (&runtime.connection, &runtime.key) {
                create_automatic_snapshots(connection, key, &self.backup_dir)
                    .err()
                    .map(|error| format!("自动快照未能完成：{}", error.message))
            } else {
                None
            };
        if let Err(error) = self.session.require_epoch(epoch) {
            Self::close_runtime(&mut runtime);
            return Err(error);
        }
        Ok(self.status_for_runtime(&runtime))
    }

    pub fn lock(&self) -> SecurityStatus {
        self.session.revoke("manual");
        let mut runtime = self
            .runtime
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        Self::close_runtime(&mut runtime);
        self.status_for_runtime(&runtime)
    }

    fn close_runtime(runtime: &mut RuntimeState) {
        runtime.snapshot_stamp = None;
        runtime.key = None;
        runtime.connection = None;
    }

    pub(crate) fn close_revoked_session(&self) {
        if let Ok(mut runtime) = self.runtime.try_lock()
            && !self.session.snapshot().unlocked
        {
            Self::close_runtime(&mut runtime);
        }
    }

    /// Nonblocking acquisition: snapshot work must never delay revoking a session.
    /// Errors are reported in SecurityStatus and do not undo successful business writes.
    pub(crate) fn refresh_snapshot_if_due(&self, force: bool) -> CommandResult<bool> {
        let Ok(mut runtime) = self.runtime.try_lock() else {
            return Ok(false);
        };
        if !force && !self.session.snapshot().unlocked {
            return Ok(false);
        }
        let (Some(connection), Some(key)) = (&runtime.connection, &runtime.key) else {
            return Ok(false);
        };
        let now = Utc::now()
            .with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).expect("fixed offset"));
        let changes = connection.total_changes();
        if let Some((at, previous, day)) = runtime.snapshot_stamp
            && day == now.date_naive()
            && (previous == changes
                || (!force && at.elapsed() < std::time::Duration::from_secs(300)))
        {
            return Ok(false);
        }
        let result = (|| {
            create_automatic_snapshots(connection, key, &self.backup_dir)?;
            let name = format!(
                "latest-{}-{}.sqlite3",
                Utc::now().format("%Y%m%dT%H%M%S%.3fZ"),
                uuid::Uuid::now_v7()
            );
            create_encrypted_snapshot(connection, key, &self.backup_dir.join(name))?;
            prune_snapshots(&self.backup_dir, "latest-", 3)?;
            Ok(())
        })();
        // Throttle failures as well; a full disk must not trigger a busy retry loop.
        let saved_changes = if result.is_ok() {
            runtime
                .connection
                .as_ref()
                .map_or(changes, Connection::total_changes)
        } else {
            u64::MAX
        };
        runtime.snapshot_stamp = Some((std::time::Instant::now(), saved_changes, now.date_naive()));
        runtime.snapshot_warning = result
            .as_ref()
            .err()
            .map(|error: &CommandError| format!("最近恢复点未能完成：{}", error.message));
        result.map(|()| true)
    }

    pub fn with_connection<T>(
        &self,
        operation: impl FnOnce(&Connection) -> CommandResult<T>,
    ) -> CommandResult<T> {
        let epoch = self.session.require_active()?;
        let runtime = self
            .runtime
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        self.session.require_epoch(epoch)?;
        let connection = runtime
            .connection
            .as_ref()
            .ok_or_else(CommandError::locked)?;
        let result = operation(connection);
        self.session.require_epoch(epoch)?;
        result
    }

    pub fn with_connection_mut<T>(
        &self,
        operation: impl FnOnce(&mut Connection) -> CommandResult<T>,
    ) -> CommandResult<T> {
        let epoch = self.session.require_active()?;
        let mut runtime = self
            .runtime
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        self.session.require_epoch(epoch)?;
        let connection = runtime
            .connection
            .as_mut()
            .ok_or_else(CommandError::locked)?;
        let result = operation(connection);
        self.session.require_epoch(epoch)?;
        result
    }
}

fn read_database_key() -> Result<Vec<u8>, security_framework::base::Error> {
    generic_password(PasswordOptions::new_generic_password(
        KEYCHAIN_SERVICE,
        DB_KEY_ACCOUNT,
    ))
}

fn create_database_key(data_dir: &Path) -> CommandResult<Vec<u8>> {
    let mut key = vec![0_u8; 32];
    rand::rng().fill_bytes(&mut key);

    let access_control = SecAccessControl::create_with_protection(
        Some(ProtectionMode::AccessibleWhenUnlockedThisDeviceOnly),
        AccessControlOptions::USER_PRESENCE.bits(),
    )
    .map_err(|error| {
        CommandError::new(
            "KEYCHAIN_CONFIGURATION_FAILED",
            format!("无法创建钥匙串访问控制：{error}"),
        )
    })?;

    let mut options = PasswordOptions::new_generic_password(KEYCHAIN_SERVICE, DB_KEY_ACCOUNT);
    options.set_access_synchronized(Some(false));
    options.set_label("个人工作台数据库密钥");
    options.set_comment("仅在本机使用，不同步到 iCloud 钥匙串");
    options.set_access_control(access_control);
    if let Err(error) = set_generic_password_options(&key, options) {
        if error.code() != ERR_SEC_MISSING_ENTITLEMENT {
            return Err(CommandError::new(
                "KEYCHAIN_SAVE_FAILED",
                format!("无法将数据库密钥保存到 macOS 钥匙串：{error}"),
            )
            .with_recovery("请确认已启用系统登录密码或 Touch ID，然后重试。"));
        }
        let mut fallback = PasswordOptions::new_generic_password(KEYCHAIN_SERVICE, DB_KEY_ACCOUNT);
        fallback.set_label("个人工作台数据库密钥");
        fallback.set_comment("本地未签名构建：由 macOS 登录钥匙串保护，不同步到 iCloud");
        set_generic_password_options(&key, fallback).map_err(|_| {
            CommandError::new(
                "KEYCHAIN_FALLBACK_REQUIRED",
                "当前本地构建无法写入 macOS 钥匙串",
            )
            .with_recovery("请设置一个独立的工作台口令；随机数据库密钥将被加密后保存在本机。")
        })?;
        fs::write(data_dir.join(KEYCHAIN_MODE_FILE), b"login-keychain")?;
    }
    Ok(key)
}

fn validate_local_password(password: &str) -> CommandResult<()> {
    if password.chars().count() < LOCAL_PASSWORD_MIN_CHARS {
        return Err(
            CommandError::new("WEAK_LOCAL_PASSWORD", "工作台口令至少需要 6 个字符")
                .with_recovery("建议混合字母、数字或符号，并使用与 Mac 登录密码不同的口令。"),
        );
    }
    Ok(())
}

fn store_password_protected_key(data_dir: &Path, key: &[u8], password: &str) -> CommandResult<()> {
    validate_local_password(password)?;
    let encrypted = encrypt_database_key(key, password)?;
    let destination = data_dir.join(PASSWORD_KEY_FILE);
    let temporary = data_dir.join("database-key.age.tmp");
    if temporary.exists() {
        fs::remove_file(&temporary)?;
    }
    let result = (|| -> CommandResult<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)?;
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600))?;
        file.write_all(&encrypted)?;
        file.sync_all()?;
        drop(file);

        let verified = Zeroizing::new(read_password_protected_key_file(&temporary, password)?);
        if verified.as_slice() != key {
            return Err(CommandError::new(
                "LOCAL_KEY_VERIFICATION_FAILED",
                "新工作台口令的密钥文件验证失败",
            )
            .with_recovery("原口令和数据库均未修改，请重试。"));
        }
        fs::rename(&temporary, destination)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn encrypt_database_key(key: &[u8], password: &str) -> CommandResult<Vec<u8>> {
    let encryptor = Encryptor::with_user_passphrase(SecretString::from(password.to_owned()));
    let mut encrypted = Vec::new();
    {
        let mut writer = encryptor.wrap_output(&mut encrypted).map_err(|error| {
            CommandError::new(
                "KEY_ENCRYPTION_FAILED",
                format!("无法初始化本地密钥加密：{error}"),
            )
        })?;
        writer.write_all(key)?;
        writer.finish().map_err(|error| {
            CommandError::new(
                "KEY_ENCRYPTION_FAILED",
                format!("无法完成本地密钥加密：{error}"),
            )
        })?;
    }
    Ok(encrypted)
}

fn read_password_protected_key(data_dir: &Path, password: &str) -> CommandResult<Vec<u8>> {
    validate_local_password(password)?;
    read_password_protected_key_file(&data_dir.join(PASSWORD_KEY_FILE), password)
}

fn read_password_protected_key_file(path: &Path, password: &str) -> CommandResult<Vec<u8>> {
    let bytes = fs::read(path)?;
    let decryptor = Decryptor::new(Cursor::new(bytes))
        .map_err(|_| CommandError::new("LOCAL_KEY_INVALID", "本地数据库密钥文件格式无效"))?;
    let identity = age::scrypt::Identity::new(SecretString::from(password.to_owned()));
    let mut reader = decryptor
        .decrypt(std::iter::once(&identity as &dyn Identity))
        .map_err(local_key_decryption_error)?;
    let mut key = Vec::with_capacity(32);
    reader.read_to_end(&mut key)?;
    if key.len() != 32 {
        return Err(CommandError::new(
            "LOCAL_KEY_INVALID",
            "解密后的数据库密钥长度无效",
        ));
    }
    Ok(key)
}

fn local_key_decryption_error(error: age::DecryptError) -> CommandError {
    if matches!(error, age::DecryptError::ExcessiveWork { .. }) {
        return CommandError::new("LOCAL_KEY_RESOURCE_LIMIT", "本地密钥解密成本超过当前设备的安全限制")
            .with_recovery("这不表示口令错误。密钥和数据库未被修改，请关闭高负载程序后重试；若持续出现，请保留原密钥文件并联系维护者，不要重新初始化。");
    }
    CommandError::new("LOCAL_PASSWORD_INCORRECT", "工作台口令错误")
        .with_recovery("当前数据库未被修改，请检查口令后重试。")
}

pub fn set_tushare_token(token: &str) -> CommandResult<()> {
    let trimmed = token.trim();
    if trimmed.len() < 16 || trimmed.len() > 128 {
        return Err(
            CommandError::new("INVALID_TOKEN", "Tushare Token 长度不符合预期")
                .with_recovery("请从 Tushare 个人中心复制完整 Token。"),
        );
    }
    let mut options =
        PasswordOptions::new_generic_password(KEYCHAIN_SERVICE, TUSHARE_TOKEN_ACCOUNT);
    options.set_label("个人工作台 Tushare Token");
    options.set_comment("行情请求凭据，不写入工作台数据库或日志");
    set_generic_password_options(trimmed.as_bytes(), options).map_err(|error| {
        CommandError::new(
            "TOKEN_SAVE_FAILED",
            format!("无法将 Token 保存到 macOS 钥匙串：{error}"),
        )
    })
}

pub fn get_tushare_token() -> CommandResult<String> {
    let options = PasswordOptions::new_generic_password(KEYCHAIN_SERVICE, TUSHARE_TOKEN_ACCOUNT);
    let bytes = generic_password(options).map_err(|error| {
        CommandError::new(
            "TOKEN_NOT_CONFIGURED",
            format!("尚未配置可用的 Tushare Token：{error}"),
        )
        .with_recovery("请在设置中保存 Token，或继续使用手工价格。")
    })?;
    String::from_utf8(bytes)
        .map_err(|_| CommandError::new("INVALID_TOKEN", "钥匙串中的 Token 格式无效"))
}

pub(crate) fn open_cipher_connection(path: &Path, key: &[u8]) -> CommandResult<Connection> {
    let connection = Connection::open(path)?;
    configure_cipher_connection(connection, key)
}

pub(crate) fn open_cipher_connection_read_only(
    path: &Path,
    key: &[u8],
) -> CommandResult<Connection> {
    let mut flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    // SQLite can create WAL/SHM even for a read-only connection to a WAL-mode database.
    // A closed, checkpointed candidate has no sidecars and must be inspected without creating them.
    let no_sidecars = ["-wal", "-shm"].iter().all(|suffix| {
        fs::symlink_metadata(PathBuf::from(format!("{}{}", path.display(), suffix)))
            .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
    });
    let open_path = if no_sidecars {
        let absolute = std::path::absolute(path)?;
        let mut uri = reqwest::Url::from_file_path(&absolute)
            .map_err(|_| CommandError::new("INVALID_DATABASE_PATH", "无法解析数据库文件路径"))?;
        uri.set_query(Some("immutable=1"));
        flags |= OpenFlags::SQLITE_OPEN_URI;
        PathBuf::from(uri.as_str())
    } else {
        path.to_owned()
    };
    let connection = Connection::open_with_flags(open_path, flags)?;
    configure_cipher_connection(connection, key)
}

fn configure_cipher_connection(connection: Connection, key: &[u8]) -> CommandResult<Connection> {
    let key_hex = Zeroizing::new(hex::encode(key));
    let key_sql = Zeroizing::new(format!(
        "PRAGMA key = \"x'{key_hex}'\";\
         PRAGMA cipher_memory_security = ON;\
         PRAGMA foreign_keys = ON;\
         PRAGMA busy_timeout = 5000;",
        key_hex = key_hex.as_str()
    ));
    connection.execute_batch(&key_sql)?;
    drop(key_sql);
    drop(key_hex);

    let cipher_version: Option<String> = connection
        .query_row("PRAGMA cipher_version", [], |row| row.get(0))
        .optional()?;
    if cipher_version.as_deref().unwrap_or_default().is_empty() {
        return Err(CommandError::new(
            "ENCRYPTION_UNAVAILABLE",
            "当前构建未启用 SQLCipher，已拒绝打开数据库",
        ));
    }
    connection
        .query_row("SELECT count(*) FROM sqlite_master", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(|_| {
            CommandError::new("DATABASE_UNLOCK_FAILED", "数据库密钥不正确或数据库已损坏")
                .with_recovery("请勿覆盖文件；可尝试从已验证备份恢复。")
        })?;
    Ok(connection)
}

pub(crate) fn open_database(path: &Path, key: &[u8]) -> CommandResult<Connection> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let connection = open_cipher_connection(path, key)?;
    finish_open_database(connection, path)
}

fn open_database_with_migration_snapshot(
    path: &Path,
    key: &[u8],
    backup_dir: &Path,
) -> CommandResult<Connection> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let existed = path.metadata().is_ok_and(|metadata| metadata.len() > 0);
    let connection = open_cipher_connection(path, key)?;
    if existed && schema_version(&connection)? < LATEST_SCHEMA_VERSION {
        let timestamp = Utc::now().format("%Y%m%dT%H%M%S%.3fZ");
        let snapshot = backup_dir.join(format!("pre-migration-{timestamp}.sqlite3"));
        create_encrypted_snapshot(&connection, key, &snapshot)?;
        prune_snapshots(backup_dir, "pre-migration-", 5)?;
    }
    finish_open_database(connection, path)
}

fn finish_open_database(connection: Connection, path: &Path) -> CommandResult<Connection> {
    connection.execute_batch(
        "PRAGMA journal_mode = WAL;\
         PRAGMA synchronous = FULL;\
         PRAGMA foreign_keys = ON;\
         PRAGMA busy_timeout = 5000;",
    )?;
    run_migrations(&connection)?;
    restrict_database_permissions(path)?;
    Ok(connection)
}

pub(crate) fn schema_version(connection: &Connection) -> CommandResult<i64> {
    let exists: i64 = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='schema_migrations')",
        [],
        |row| row.get(0),
    )?;
    if exists == 0 {
        return Ok(0);
    }
    connection
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )
        .map_err(Into::into)
}

pub(crate) fn restrict_file_permissions(path: &Path) -> CommandResult<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() && !metadata.file_type().is_symlink() => {
            fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
        }
        Ok(_) => {
            return Err(CommandError::new(
                "UNSAFE_LOCAL_FILE",
                "安全文件路径不是普通文件，已拒绝访问",
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

fn restrict_database_permissions(path: &Path) -> CommandResult<()> {
    restrict_file_permissions(path)?;
    for suffix in ["-wal", "-shm"] {
        restrict_file_permissions(&PathBuf::from(format!("{}{}", path.display(), suffix)))?;
    }
    Ok(())
}

pub(crate) fn run_migrations(connection: &Connection) -> CommandResult<()> {
    if schema_version(connection)? > LATEST_SCHEMA_VERSION {
        return Err(schema_error());
    }
    run_base_migration(connection)?;
    if schema_version(connection)? < 2 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch("ALTER TABLE work_logs ADD COLUMN task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL; CREATE INDEX idx_work_logs_task ON work_logs(task_id); INSERT INTO schema_migrations(version,applied_at) VALUES(2,strftime('%Y-%m-%dT%H:%M:%fZ','now'));")?;
        transaction.commit()?;
    }
    Ok(())
}

fn run_base_migration(connection: &Connection) -> CommandResult<()> {
    connection.execute_batch(
        r#"
        BEGIN IMMEDIATE;
        CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS settings (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            theme TEXT NOT NULL DEFAULT 'system',
            amounts_hidden INTEGER NOT NULL DEFAULT 0,
            lock_minutes INTEGER NOT NULL DEFAULT 15,
            quote_enabled INTEGER NOT NULL DEFAULT 0,
            quote_auto_refresh INTEGER NOT NULL DEFAULT 1,
            last_quote_refresh TEXT,
            timezone TEXT NOT NULL DEFAULT 'Asia/Shanghai',
            currency TEXT NOT NULL DEFAULT 'CNY'
        );
        INSERT OR IGNORE INTO settings (id) VALUES (1);

        CREATE TABLE IF NOT EXISTS projects (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            area TEXT NOT NULL DEFAULT 'work',
            status TEXT NOT NULL DEFAULT 'active',
            color TEXT NOT NULL DEFAULT '#397064',
            notes TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS goals (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            horizon TEXT NOT NULL DEFAULT 'quarter',
            status TEXT NOT NULL DEFAULT 'active',
            progress INTEGER NOT NULL DEFAULT 0 CHECK(progress BETWEEN 0 AND 100),
            notes TEXT NOT NULL DEFAULT '',
            start_date TEXT,
            target_date TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS content_items (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            platform TEXT NOT NULL DEFAULT '未指定',
            format TEXT NOT NULL DEFAULT '图文',
            status TEXT NOT NULL DEFAULT 'idea',
            goal TEXT NOT NULL DEFAULT '',
            tags_json TEXT NOT NULL DEFAULT '[]',
            publish_at TEXT,
            notes TEXT NOT NULL DEFAULT '',
            asset_path TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS tasks (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            notes TEXT NOT NULL DEFAULT '',
            status TEXT NOT NULL DEFAULT 'todo',
            priority INTEGER NOT NULL DEFAULT 2 CHECK(priority BETWEEN 1 AND 3),
            due_date TEXT,
            scheduled_start TEXT,
            scheduled_end TEXT,
            recurrence TEXT NOT NULL DEFAULT 'none',
            project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
            goal_id TEXT REFERENCES goals(id) ON DELETE SET NULL,
            content_id TEXT REFERENCES content_items(id) ON DELETE SET NULL,
            completed_at TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS calendar_items (
            id TEXT PRIMARY KEY,
            kind TEXT NOT NULL DEFAULT 'event',
            title TEXT NOT NULL,
            notes TEXT NOT NULL DEFAULT '',
            start_at TEXT NOT NULL,
            end_at TEXT NOT NULL,
            all_day INTEGER NOT NULL DEFAULT 0,
            recurrence TEXT NOT NULL DEFAULT 'none',
            source TEXT NOT NULL DEFAULT 'internal',
            external_uid TEXT,
            project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(source, external_uid)
        );

        CREATE TABLE IF NOT EXISTS work_logs (
            id TEXT PRIMARY KEY,
            log_date TEXT NOT NULL,
            project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
            title TEXT NOT NULL,
            completed TEXT NOT NULL DEFAULT '',
            blockers TEXT NOT NULL DEFAULT '',
            next_steps TEXT NOT NULL DEFAULT '',
            minutes INTEGER NOT NULL DEFAULT 0 CHECK(minutes >= 0),
            energy INTEGER NOT NULL DEFAULT 3 CHECK(energy BETWEEN 1 AND 5),
            markdown TEXT NOT NULL DEFAULT '',
            attachment_path TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS habits (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            frequency TEXT NOT NULL DEFAULT 'daily',
            target_per_week INTEGER NOT NULL DEFAULT 7 CHECK(target_per_week BETWEEN 1 AND 7),
            color TEXT NOT NULL DEFAULT '#397064',
            active INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS habit_checks (
            id TEXT PRIMARY KEY,
            habit_id TEXT NOT NULL REFERENCES habits(id) ON DELETE CASCADE,
            check_date TEXT NOT NULL,
            value INTEGER NOT NULL DEFAULT 1,
            note TEXT NOT NULL DEFAULT '',
            UNIQUE(habit_id, check_date)
        );

        CREATE TABLE IF NOT EXISTS learning_items (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            kind TEXT NOT NULL DEFAULT 'course',
            status TEXT NOT NULL DEFAULT 'planned',
            progress INTEGER NOT NULL DEFAULT 0 CHECK(progress BETWEEN 0 AND 100),
            notes TEXT NOT NULL DEFAULT '',
            target_date TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS content_metrics (
            id TEXT PRIMARY KEY,
            content_id TEXT NOT NULL REFERENCES content_items(id) ON DELETE CASCADE,
            recorded_at TEXT NOT NULL,
            views INTEGER NOT NULL DEFAULT 0,
            likes INTEGER NOT NULL DEFAULT 0,
            comments INTEGER NOT NULL DEFAULT 0,
            saves INTEGER NOT NULL DEFAULT 0,
            followers_delta INTEGER NOT NULL DEFAULT 0,
            UNIQUE(content_id, recorded_at)
        );

        CREATE TABLE IF NOT EXISTS investment_accounts (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            kind TEXT NOT NULL DEFAULT 'securities',
            currency TEXT NOT NULL DEFAULT 'CNY',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS instruments (
            id TEXT PRIMARY KEY,
            code TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            kind TEXT NOT NULL DEFAULT 'stock',
            market TEXT NOT NULL DEFAULT 'CN',
            currency TEXT NOT NULL DEFAULT 'CNY',
            manual_price TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS portfolio_transactions (
            id TEXT PRIMARY KEY,
            account_id TEXT NOT NULL REFERENCES investment_accounts(id) ON DELETE CASCADE,
            instrument_id TEXT NOT NULL REFERENCES instruments(id) ON DELETE CASCADE,
            kind TEXT NOT NULL,
            trade_date TEXT NOT NULL,
            quantity TEXT NOT NULL DEFAULT '0',
            unit_price TEXT NOT NULL DEFAULT '0',
            amount TEXT NOT NULL DEFAULT '0',
            fee TEXT NOT NULL DEFAULT '0',
            tax TEXT NOT NULL DEFAULT '0',
            notes TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS price_points (
            id TEXT PRIMARY KEY,
            instrument_id TEXT NOT NULL REFERENCES instruments(id) ON DELETE CASCADE,
            price_date TEXT NOT NULL,
            price TEXT NOT NULL,
            source TEXT NOT NULL DEFAULT 'manual',
            created_at TEXT NOT NULL,
            UNIQUE(instrument_id, price_date, source)
        );

        CREATE TABLE IF NOT EXISTS review_snapshots (
            id TEXT PRIMARY KEY,
            period_type TEXT NOT NULL DEFAULT 'week',
            start_date TEXT NOT NULL,
            end_date TEXT NOT NULL,
            summary TEXT NOT NULL DEFAULT '',
            reflection TEXT NOT NULL DEFAULT '',
            status TEXT NOT NULL DEFAULT 'draft',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_tasks_status_due ON tasks(status, due_date);
        CREATE INDEX IF NOT EXISTS idx_tasks_schedule ON tasks(scheduled_start);
        CREATE INDEX IF NOT EXISTS idx_calendar_start ON calendar_items(start_at);
        CREATE INDEX IF NOT EXISTS idx_work_logs_date ON work_logs(log_date DESC);
        CREATE INDEX IF NOT EXISTS idx_content_status_publish ON content_items(status, publish_at);
        CREATE INDEX IF NOT EXISTS idx_habit_checks_date ON habit_checks(check_date, habit_id);
        CREATE INDEX IF NOT EXISTS idx_transactions_instrument_date ON portfolio_transactions(instrument_id, trade_date);
        CREATE INDEX IF NOT EXISTS idx_prices_instrument_date ON price_points(instrument_id, price_date DESC);

        INSERT OR IGNORE INTO schema_migrations(version, applied_at)
        VALUES (1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
        COMMIT;
        PRAGMA optimize;
        "#,
    )?;
    Ok(())
}

pub fn load_settings(connection: &Connection) -> CommandResult<AppSettings> {
    connection
        .query_row(
            "SELECT theme, amounts_hidden, lock_minutes, quote_enabled, quote_auto_refresh, last_quote_refresh, timezone, currency FROM settings WHERE id = 1",
            [],
            |row| {
                Ok(AppSettings {
                    theme: row.get(0)?,
                    amounts_hidden: row.get::<_, i64>(1)? != 0,
                    lock_minutes: row.get(2)?,
                    quote_enabled: row.get::<_, i64>(3)? != 0,
                    quote_auto_refresh: row.get::<_, i64>(4)? != 0,
                    last_quote_refresh: row.get(5)?,
                    timezone: row.get(6)?,
                    currency: row.get(7)?,
                })
            },
        )
        .map_err(Into::into)
}

pub fn save_settings(connection: &Connection, settings: &AppSettings) -> CommandResult<()> {
    if !matches!(settings.theme.as_str(), "light" | "dark" | "system") {
        return Err(CommandError::new("INVALID_THEME", "主题设置无效"));
    }
    if !matches!(settings.lock_minutes, 0 | 5 | 15 | 30) {
        return Err(CommandError::new("INVALID_LOCK_TIME", "自动锁定时间无效"));
    }
    connection.execute(
        "UPDATE settings SET theme = ?1, amounts_hidden = ?2, lock_minutes = ?3, quote_enabled = ?4, quote_auto_refresh = ?5, last_quote_refresh = ?6, timezone = ?7, currency = ?8 WHERE id = 1",
        params![
            settings.theme,
            settings.amounts_hidden as i64,
            settings.lock_minutes,
            settings.quote_enabled as i64,
            settings.quote_auto_refresh as i64,
            settings.last_quote_refresh,
            settings.timezone,
            settings.currency,
        ],
    )?;
    Ok(())
}

pub fn now_utc() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn create_automatic_snapshots(
    connection: &Connection,
    key: &[u8],
    backup_dir: &Path,
) -> CommandResult<()> {
    let now =
        Utc::now().with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).expect("fixed offset"));
    let day_name = format!("daily-{}.sqlite3", now.format("%Y-%m-%d"));
    create_encrypted_snapshot(connection, key, &backup_dir.join(day_name))?;

    let iso_week = now.iso_week();
    let week_name = format!("weekly-{}-W{:02}.sqlite3", iso_week.year(), iso_week.week());
    create_encrypted_snapshot(connection, key, &backup_dir.join(week_name))?;

    prune_snapshots(backup_dir, "daily-", 7)?;
    prune_snapshots(backup_dir, "weekly-", 4)?;
    Ok(())
}

pub fn create_encrypted_snapshot(
    source: &Connection,
    key: &[u8],
    destination: &Path,
) -> CommandResult<()> {
    if let Ok(metadata) = fs::symlink_metadata(destination) {
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(CommandError::new(
                "SNAPSHOT_PATH_INVALID",
                "快照目标不是普通文件",
            ));
        }
        restrict_file_permissions(destination)?;
        return Ok(());
    }
    let parent = destination
        .parent()
        .ok_or_else(|| CommandError::new("SNAPSHOT_PATH_INVALID", "快照目录无效"))?;
    let temporary_file = tempfile::Builder::new()
        .prefix(".snapshot-")
        .suffix(".tmp")
        .tempfile_in(parent)?;
    let temporary = temporary_file.path();
    export_encrypted_copy(source, key, temporary)?;
    let target = open_cipher_connection(temporary, key)?;
    let integrity: String = target.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
    if integrity != "ok" {
        drop(target);
        return Err(CommandError::new(
            "SNAPSHOT_INTEGRITY_FAILED",
            "自动快照完整性检查失败",
        ));
    }
    drop(target);
    fs::File::open(temporary)?.sync_all()?;
    fs::rename(temporary, destination)?;
    restrict_file_permissions(destination)?;
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}

pub(crate) fn export_encrypted_copy(
    source: &Connection,
    key: &[u8],
    destination: &Path,
) -> CommandResult<()> {
    if destination.exists() {
        fs::remove_file(destination)?;
    }
    let escaped_path = destination.to_string_lossy().replace('\'', "''");
    let key_hex = Zeroizing::new(hex::encode(key));
    let key_sql = Zeroizing::new(format!(
        "ATTACH DATABASE '{escaped_path}' AS workbench_export KEY \"x'{key_hex}'\";",
        key_hex = key_hex.as_str()
    ));
    source.execute_batch(&key_sql)?;
    drop(key_sql);
    drop(key_hex);
    let copied = source.execute_batch("SELECT sqlcipher_export('workbench_export');");
    let detached = source.execute_batch("DETACH DATABASE workbench_export;");
    copied?;
    detached?;
    restrict_file_permissions(destination)?;
    Ok(())
}

pub(crate) fn prune_snapshots(directory: &Path, prefix: &str, keep: usize) -> CommandResult<()> {
    let mut files: Vec<PathBuf> = fs::read_dir(directory)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            let valid_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(prefix) && name.ends_with(".sqlite3"));
            let regular_file = fs::symlink_metadata(path).is_ok_and(|metadata| {
                metadata.file_type().is_file() && !metadata.file_type().is_symlink()
            });
            valid_name && regular_file
        })
        .collect();
    files.sort();
    let remove_count = files.len().saturating_sub(keep);
    for path in files.into_iter().take(remove_count) {
        let _ = fs::remove_file(path);
    }
    Ok(())
}

pub fn database_record_count(connection: &Connection) -> CommandResult<i64> {
    let counts = database_table_counts(connection)?;
    let mut total = 0_i64;
    for table in BUSINESS_TABLES {
        total = total.checked_add(counts[*table]).ok_or_else(|| {
            CommandError::new("BACKUP_COUNT_INVALID", "数据库记录数超过可校验范围")
        })?;
    }
    Ok(total)
}

pub(crate) fn database_table_counts(
    connection: &Connection,
) -> CommandResult<BTreeMap<String, i64>> {
    let mut counts = BTreeMap::new();
    for table in BUSINESS_TABLES
        .iter()
        .copied()
        .chain(["settings", "schema_migrations"])
    {
        // Table names come only from the compiled schema allowlist.
        let count = connection.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
            row.get::<_, i64>(0)
        })?;
        counts.insert(table.to_owned(), count);
    }
    Ok(counts)
}

type ColumnDefinition = (String, String, i64, Option<String>, i64);
type ForeignKeyDefinition = (String, String, Option<String>, String, String, String);

#[derive(Debug, PartialEq, Eq)]
struct TableDefinition {
    columns: Vec<ColumnDefinition>,
    foreign_keys: Vec<ForeignKeyDefinition>,
    unique_keys: Vec<Vec<String>>,
}

fn table_definition(connection: &Connection, table: &str) -> CommandResult<TableDefinition> {
    let mut statement = connection.prepare(
        "SELECT name,type,\"notnull\",dflt_value,pk FROM pragma_table_info(?1) ORDER BY cid",
    )?;
    let columns = statement
        .query_map([table], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })?
        .collect::<Result<Vec<ColumnDefinition>, _>>()?;
    let mut statement = connection.prepare("SELECT \"table\",\"from\",\"to\",on_update,on_delete,\"match\" FROM pragma_foreign_key_list(?1) ORDER BY \"table\",\"from\",\"to\",seq")?;
    let foreign_keys = statement
        .query_map([table], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            ))
        })?
        .collect::<Result<Vec<ForeignKeyDefinition>, _>>()?;
    let mut statement =
        connection.prepare("SELECT name FROM pragma_index_list(?1) WHERE \"unique\"=1")?;
    let indexes = statement
        .query_map([table], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    let mut unique_keys = Vec::new();
    for name in indexes {
        // Index names are database-controlled; bind them instead of interpolating SQL.
        let mut statement =
            connection.prepare("SELECT name FROM pragma_index_info(?1) ORDER BY seqno")?;
        unique_keys.push(
            statement
                .query_map([name], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?,
        );
    }
    unique_keys.sort();
    Ok(TableDefinition {
        columns,
        foreign_keys,
        unique_keys,
    })
}

fn schema_error() -> CommandError {
    CommandError::new(
        "BACKUP_SCHEMA_INVALID",
        "数据库结构与当前支持的工作台版本不一致，已拒绝恢复",
    )
    .with_recovery("请使用结构完整的工作台备份；不要删除当前数据库或手工修改备份版本。")
}

pub fn validate_connection(connection: &Connection) -> CommandResult<()> {
    let integrity: String = connection.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
    if integrity != "ok" {
        return Err(CommandError::new(
            "DATABASE_INTEGRITY_FAILED",
            format!("数据库完整性检查未通过：{integrity}"),
        ));
    }
    // Build the expected structure on a separate in-memory connection, never
    // run migrations on an untrusted candidate to make a damaged backup pass.
    let version = schema_version(connection)?;
    if !(1..=LATEST_SCHEMA_VERSION).contains(&version) {
        return Err(CommandError::new(
            "BACKUP_VERSION_UNSUPPORTED",
            "数据库结构版本不受当前工作台支持",
        ));
    }
    let reference = Connection::open_in_memory()?;
    if version == 1 {
        run_base_migration(&reference)?;
    } else {
        run_migrations(&reference)?;
    }
    let mut statement = connection.prepare("SELECT name FROM sqlite_master WHERE type='table' AND substr(name,1,7)<>'sqlite_' ORDER BY name")?;
    let tables = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    let mut expected: Vec<String> = BUSINESS_TABLES
        .iter()
        .copied()
        .chain(["settings", "schema_migrations"])
        .map(str::to_owned)
        .collect();
    expected.sort();
    if tables != expected {
        return Err(schema_error());
    }
    for table in BUSINESS_TABLES
        .iter()
        .copied()
        .chain(["settings", "schema_migrations"])
    {
        let ordinary: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
            [table],
            |row| row.get(0),
        )?;
        if !ordinary
            || table_definition(connection, table).map_err(|_| schema_error())?
                != table_definition(&reference, table)?
        {
            return Err(schema_error());
        }
    }
    let unsafe_schema: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type IN ('trigger','view'))",
        [],
        |row| row.get(0),
    )?;
    if unsafe_schema {
        return Err(schema_error());
    }
    let mut statement =
        connection.prepare("SELECT version FROM schema_migrations ORDER BY version")?;
    let versions = statement
        .query_map([], |row| row.get::<_, i64>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    if versions != (1..=version).collect::<Vec<_>>() {
        return Err(CommandError::new(
            "BACKUP_VERSION_UNSUPPORTED",
            "数据库结构版本缺失或不受当前工作台支持，已拒绝恢复",
        )
        .with_recovery("请使用兼容版本的应用读取备份；不要手工降低数据库版本号。"));
    }
    let mut statement = connection.prepare("PRAGMA foreign_key_check")?;
    if statement.query([])?.next()?.is_some() {
        return Err(CommandError::new(
            "BACKUP_FOREIGN_KEY_INVALID",
            "数据库包含无效的跨模块关联，已拒绝恢复",
        )
        .with_recovery(
            "当前数据库未被替换，请选择关联完整的备份；不要手工删除关联记录来绕过校验。",
        ));
    }
    let valid_settings: i64 = connection.query_row("SELECT count(*) FROM settings WHERE id=1 AND theme IN ('light','dark','system') AND lock_minutes IN (0,5,15,30) AND amounts_hidden IN (0,1) AND quote_enabled IN (0,1) AND quote_auto_refresh IN (0,1) AND timezone='Asia/Shanghai' AND currency='CNY'", [], |row| row.get(0))?;
    let settings_count: i64 =
        connection.query_row("SELECT count(*) FROM settings", [], |row| row.get(0))?;
    if settings_count != 1 || valid_settings != 1 {
        return Err(schema_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latest_snapshots_capture_later_writes_are_throttled_and_keep_three() {
        let (_directory, state) = unlocked_fixture();
        assert!(state.refresh_snapshot_if_due(false).unwrap());
        let names = || {
            crate::backup::list_backups(&state)
                .unwrap()
                .into_iter()
                .filter(|backup| backup.kind == "latest")
                .collect::<Vec<_>>()
        };
        assert_eq!(names().len(), 1);
        // sqlcipher_export can increase total_changes: snapshot completion must
        // record the post-export counter, otherwise idle use makes endless copies.
        assert!(!state.refresh_snapshot_if_due(false).unwrap());
        for index in 0..5 {
            state.with_connection(|connection| {
                connection.execute("INSERT INTO projects(id,name,area,status,color,notes,created_at,updated_at) VALUES(?1,?1,'work','active','#397064','','2026-10-03T00:00:00Z','2026-10-03T00:00:00Z')", [format!("later-{index}")])?;
                Ok(())
            }).unwrap();
            assert!(!state.refresh_snapshot_if_due(false).unwrap());
            assert!(state.refresh_snapshot_if_due(true).unwrap());
        }
        let latest = names();
        assert_eq!(latest.len(), 3);
        let newest =
            open_cipher_connection(&state.backup_dir.join(&latest[0].name), &[71; 32]).unwrap();
        assert_eq!(
            newest
                .query_row("SELECT COUNT(*) FROM projects", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            5
        );
        assert!(!state.refresh_snapshot_if_due(true).unwrap());
        state.lock();
        assert!(!state.refresh_snapshot_if_due(false).unwrap());
        assert!(fs::read_dir(&state.backup_dir).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp")
        }));
    }

    #[test]
    fn snapshot_failure_is_reported_and_throttled_without_changing_business_data() {
        let (_directory, state) = unlocked_fixture();
        fs::remove_dir(&state.backup_dir).unwrap();
        fs::write(&state.backup_dir, b"not a directory").unwrap();
        assert!(state.refresh_snapshot_if_due(false).is_err());
        assert!(state.status().snapshot_warning.is_some());
        assert!(!state.refresh_snapshot_if_due(false).unwrap());
        state
            .with_connection(|connection| {
                assert!(crate::repository::list_projects(connection)?.is_empty());
                Ok(())
            })
            .unwrap();
        fs::remove_file(&state.backup_dir).unwrap();
        fs::create_dir(&state.backup_dir).unwrap();
        assert!(state.refresh_snapshot_if_due(true).unwrap());
        assert!(state.status().snapshot_warning.is_none());
    }

    #[test]
    fn backup_validation_rejects_orphaned_relations_even_with_foreign_keys_disabled() {
        let (_directory, state) = unlocked_fixture();
        let runtime = state.runtime.lock().unwrap();
        let connection = runtime.connection.as_ref().unwrap();
        connection.execute_batch("PRAGMA foreign_keys=OFF; INSERT INTO habit_checks(id,habit_id,check_date) VALUES('orphan','missing','2026-10-03');").unwrap();
        assert_eq!(
            connection
                .query_row("PRAGMA quick_check", [], |row| row.get::<_, String>(0))
                .unwrap(),
            "ok"
        );
        assert_eq!(
            validate_connection(connection).unwrap_err().code,
            "BACKUP_FOREIGN_KEY_INVALID"
        );
        assert_eq!(
            connection
                .query_row("SELECT count(*) FROM habit_checks", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn backup_validation_rejects_future_and_incomplete_schema_versions() {
        for change in [
            "DELETE FROM schema_migrations",
            "UPDATE schema_migrations SET version=999 WHERE version=2",
            "INSERT INTO schema_migrations VALUES(0,'now')",
        ] {
            let (_directory, state) = unlocked_fixture();
            let runtime = state.runtime.lock().unwrap();
            let connection = runtime.connection.as_ref().unwrap();
            connection.execute_batch(change).unwrap();
            assert_eq!(
                validate_connection(connection).unwrap_err().code,
                "BACKUP_VERSION_UNSUPPORTED"
            );
        }
    }

    #[test]
    fn backup_validation_rejects_missing_structure_and_unsafe_settings_without_repair() {
        for change in [
            "DROP TABLE review_snapshots",
            "ALTER TABLE projects DROP COLUMN notes",
            "DROP TABLE habit_checks; CREATE TABLE habit_checks(id TEXT PRIMARY KEY,habit_id TEXT NOT NULL,check_date TEXT NOT NULL,value INTEGER NOT NULL DEFAULT 1,note TEXT NOT NULL DEFAULT '',UNIQUE(habit_id,check_date))",
            "DROP TABLE projects; CREATE TABLE projects(id TEXT,name TEXT NOT NULL,area TEXT NOT NULL DEFAULT 'work',status TEXT NOT NULL DEFAULT 'active',color TEXT NOT NULL DEFAULT '#397064',notes TEXT NOT NULL DEFAULT '',created_at TEXT NOT NULL,updated_at TEXT NOT NULL)",
            "CREATE TRIGGER unexpected AFTER INSERT ON projects BEGIN DELETE FROM goals; END",
            "UPDATE settings SET lock_minutes=999",
            "DELETE FROM settings",
        ] {
            let (_directory, state) = unlocked_fixture();
            let runtime = state.runtime.lock().unwrap();
            let connection = runtime.connection.as_ref().unwrap();
            connection.execute_batch(change).unwrap();
            let before = connection.serialize(rusqlite::MAIN_DB).unwrap().to_vec();
            assert_eq!(
                validate_connection(connection).unwrap_err().code,
                "BACKUP_SCHEMA_INVALID",
                "{change}"
            );
            assert_eq!(
                connection.serialize(rusqlite::MAIN_DB).unwrap().to_vec(),
                before
            );
        }
    }

    #[test]
    fn concurrent_authentication_is_rejected_before_keys_or_files_are_accessed() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::new(directory.path().to_owned()).unwrap();
        let guard = state.authentication.lock().unwrap();
        for result in [
            state.unlock(),
            state.initialize_with_password("123456"),
            state.unlock_with_password("123456"),
            state.change_local_password("123456", "654321"),
        ] {
            assert_eq!(result.unwrap_err().code, "AUTHENTICATION_IN_PROGRESS");
        }
        assert!(!state.database_path.exists());
        assert!(!state.data_dir.join(PASSWORD_KEY_FILE).exists());
        drop(guard);
        assert!(state.authentication_guard().is_ok());
    }

    #[test]
    fn incomplete_initialization_preserves_existing_wrapped_key_and_can_resume() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::new(directory.path().to_owned()).unwrap();
        let key = vec![91; 32];
        store_password_protected_key(&state.data_dir, &key, "original-password").unwrap();
        let before = fs::read(state.data_dir.join(PASSWORD_KEY_FILE)).unwrap();
        assert_eq!(
            state
                .initialize_with_password("replacement-password")
                .unwrap_err()
                .code,
            "LOCAL_KEY_ALREADY_EXISTS"
        );
        assert_eq!(
            fs::read(state.data_dir.join(PASSWORD_KEY_FILE)).unwrap(),
            before
        );
        assert!(!state.database_path.exists());
        assert!(
            state
                .unlock_with_password("original-password")
                .unwrap()
                .unlocked
        );
        state.lock();
        assert!(
            state
                .unlock_with_password("original-password")
                .unwrap()
                .unlocked
        );
    }

    #[test]
    fn deleting_old_data_and_snapshots_keeps_original_password_for_a_fresh_database() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::new(directory.path().to_owned()).unwrap();
        state.initialize_with_password("original-password").unwrap();
        state.with_connection(|connection| {
            connection.execute("INSERT INTO projects(id,name,area,status,color,notes,created_at,updated_at) VALUES('old-test','旧测试数据','work','active','#000','','now','now')", [])?;
            Ok(())
        }).unwrap();
        assert!(state.refresh_snapshot_if_due(true).unwrap());
        let wrapped_key = fs::read(state.data_dir.join(PASSWORD_KEY_FILE)).unwrap();
        state.lock();
        drop(state);
        for entry in fs::read_dir(directory.path()).unwrap() {
            let entry = entry.unwrap();
            if entry.file_name() == PASSWORD_KEY_FILE {
                continue;
            }
            if entry.file_type().unwrap().is_dir() {
                fs::remove_dir_all(entry.path()).unwrap();
            } else {
                fs::remove_file(entry.path()).unwrap();
            }
        }
        let fresh = AppState::new(directory.path().to_owned()).unwrap();
        assert!(fresh.status().initialized);
        assert_eq!(fresh.status().keychain_mode, "passphrase");
        assert_eq!(
            fresh
                .unlock_with_password("incorrect-password")
                .unwrap_err()
                .code,
            "LOCAL_PASSWORD_INCORRECT"
        );
        assert!(!fresh.database_path.exists());
        assert!(
            fresh
                .unlock_with_password("original-password")
                .unwrap()
                .unlocked
        );
        fresh
            .with_connection(|connection| {
                for table in BUSINESS_TABLES {
                    let count: i64 = connection.query_row(
                        &format!("SELECT COUNT(*) FROM {table}"),
                        [],
                        |row| row.get(0),
                    )?;
                    assert_eq!(count, 0, "old records remained in {table}");
                }
                Ok(())
            })
            .unwrap();
        assert_eq!(
            fs::read(fresh.data_dir.join(PASSWORD_KEY_FILE)).unwrap(),
            wrapped_key
        );
        for backup in crate::backup::list_backups(&fresh).unwrap() {
            let key = read_password_protected_key(&fresh.data_dir, "original-password").unwrap();
            let connection =
                open_cipher_connection(&fresh.backup_dir.join(backup.name), &key).unwrap();
            assert_eq!(
                connection
                    .query_row("SELECT COUNT(*) FROM projects", [], |row| row
                        .get::<_, i64>(0))
                    .unwrap(),
                0
            );
        }
        fresh.lock();
        assert!(
            fresh
                .unlock_with_password("original-password")
                .unwrap()
                .unlocked
        );
    }

    fn unlocked_fixture() -> (tempfile::TempDir, AppState) {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::new(directory.path().to_owned()).unwrap();
        let key = vec![71; 32];
        let connection = open_database(&state.database_path, &key).unwrap();
        {
            let mut runtime = state.runtime.lock().unwrap();
            runtime.connection = Some(connection);
            runtime.key = Some(Zeroizing::new(key));
        }
        state.session.activate(0, 0).unwrap();
        (directory, state)
    }

    #[test]
    fn native_lock_revokes_access_without_waiting_for_database_and_closes_when_released() {
        let (_directory, state) = unlocked_fixture();
        let guard = state.runtime.lock().unwrap();
        // This must not acquire the database mutex: a native callback cannot wait for SQL.
        state
            .session
            .system_event(crate::session::SystemEvent::ScreenLocked);
        assert_eq!(
            state.with_connection(|_| Ok(())).unwrap_err().code,
            "APP_LOCKED"
        );
        state.close_revoked_session(); // try-lock; returns rather than blocking on our guard.
        assert!(guard.connection.is_some());
        drop(guard);
        state.close_revoked_session();
        let runtime = state.runtime.lock().unwrap();
        assert!(runtime.key.is_none() && runtime.connection.is_none());
        drop(runtime);
        assert!(!state.status().unlocked);
    }

    #[test]
    fn response_after_lock_is_discarded_but_started_transaction_finishes_safely() {
        let (_directory, state) = unlocked_fixture();
        let error = state.with_connection(|db| {
            db.execute("INSERT INTO projects(id,name,area,status,color,notes,created_at,updated_at) VALUES('p','committed','work','active','#000','','now','now')", [])?;
            state.session.revoke("system");
            Ok("sensitive response")
        }).unwrap_err();
        assert_eq!(error.code, "APP_LOCKED");
        assert!(state.with_connection(|_| Ok(())).is_err());
        state.close_revoked_session();
        let db = open_database(&state.database_path, &[71; 32]).unwrap();
        let name: String = db
            .query_row("SELECT name FROM projects", [], |row| row.get(0))
            .unwrap();
        assert_eq!(name, "committed");
    }

    #[test]
    fn unlock_started_before_system_lock_cannot_install_key_or_connection() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::new(directory.path().to_owned()).unwrap();
        let key = vec![72; 32];
        drop(open_database(&state.database_path, &key).unwrap());
        let ticket = state.session.challenge().unwrap();
        state
            .session
            .system_event(crate::session::SystemEvent::ScreenLocked);
        state
            .session
            .system_event(crate::session::SystemEvent::ScreenUnlocked);
        assert_eq!(
            state.open_with_key(key.clone(), ticket).unwrap_err().code,
            "APP_LOCKED"
        );
        let runtime = state.runtime.lock().unwrap();
        assert!(runtime.connection.is_none() && runtime.key.is_none());
        drop(runtime);
        assert!(
            state
                .open_with_key(key, state.session.challenge().unwrap())
                .unwrap()
                .unlocked
        );
    }

    #[test]
    fn expired_backend_session_rejects_reads_and_writes_without_javascript() {
        let (_directory, state) = unlocked_fixture();
        state.session.configure(5).unwrap();
        state.session.age_for_test(301);
        assert_eq!(
            state.with_connection(|_| Ok(())).unwrap_err().code,
            "APP_LOCKED"
        );
        assert_eq!(
            state.with_connection_mut(|_| Ok(())).unwrap_err().code,
            "APP_LOCKED"
        );
        assert!(state.session.activity().is_err());
        state.close_revoked_session();
        assert!(state.runtime.lock().unwrap().key.is_none());
    }

    #[test]
    fn missing_connection_cannot_leave_authorization_and_key_active() {
        let (_directory, state) = unlocked_fixture();
        state.runtime.lock().unwrap().connection = None;
        let status = state.status();
        assert!(!status.unlocked);
        assert_eq!(status.lock_reason.as_deref(), Some("databaseUnavailable"));
        assert!(state.runtime.lock().unwrap().key.is_none());
        assert!(state.session.require_active().is_err());
    }

    #[test]
    fn legacy_restore_artifacts_block_initialization_and_recover_on_password_unlock() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::new(directory.path().to_owned()).unwrap();
        let key = [63; 32];
        store_password_protected_key(&state.data_dir, &key, "original passphrase").unwrap();
        let previous = state.data_dir.join(crate::restore::PREVIOUS_DATABASE);
        let connection = open_database(&previous, &key).unwrap();
        connection.execute("INSERT INTO projects(id,name,area,status,color,notes,created_at,updated_at) VALUES('p','original data','work','active','#000','','now','now')", []).unwrap();
        drop(connection);
        let key_bytes = fs::read(state.data_dir.join(PASSWORD_KEY_FILE)).unwrap();
        let database_bytes = fs::read(&previous).unwrap();
        assert!(state.status().initialized);
        assert_eq!(
            state
                .initialize_with_password("replacement password")
                .unwrap_err()
                .code,
            "ALREADY_INITIALIZED"
        );
        assert_eq!(
            state
                .unlock_with_password("incorrect password")
                .unwrap_err()
                .code,
            "LOCAL_PASSWORD_INCORRECT"
        );
        assert_eq!(fs::read(&previous).unwrap(), database_bytes);
        assert_eq!(
            fs::read(state.data_dir.join(PASSWORD_KEY_FILE)).unwrap(),
            key_bytes
        );
        assert!(!state.database_path.exists());
        let unlocked = state.unlock_with_password("original passphrase").unwrap();
        assert!(unlocked.initialized && unlocked.unlocked);
        assert!(unlocked.recovery_notice.unwrap().contains("回滚"));
        let name: String = state
            .with_connection(|db| {
                Ok(db.query_row("SELECT name FROM projects", [], |row| row.get(0))?)
            })
            .unwrap();
        assert_eq!(name, "original data");
        assert_eq!(
            fs::read(state.data_dir.join(PASSWORD_KEY_FILE)).unwrap(),
            key_bytes
        );
        state.lock();
        assert!(
            state
                .unlock_with_password("original passphrase")
                .unwrap()
                .recovery_notice
                .is_none()
        );
    }

    #[test]
    fn local_password_minimum_is_six_characters() {
        assert_eq!(
            validate_local_password("12345").unwrap_err().code,
            "WEAK_LOCAL_PASSWORD"
        );
        assert!(validate_local_password("123456").is_ok());
        assert!(validate_local_password("安全口令12").is_ok());
    }

    #[test]
    fn database_is_encrypted_and_wrong_key_is_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("encrypted.sqlite3");
        let key = [3_u8; 32];
        let connection = open_database(&path, &key).unwrap();
        connection.execute("INSERT INTO projects(id,name,area,status,color,notes,created_at,updated_at) VALUES('p','秘密项目','work','active','#000','','now','now')", []).unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        drop(connection);

        let plaintext = Connection::open(&path).unwrap();
        assert!(
            plaintext
                .query_row("SELECT name FROM projects", [], |row| row
                    .get::<_, String>(0))
                .is_err()
        );
        drop(plaintext);

        let error = open_database(&path, &[4_u8; 32]).unwrap_err();
        assert_eq!(error.code, "DATABASE_UNLOCK_FAILED");
        let reopened = open_database(&path, &key).unwrap();
        let name: String = reopened
            .query_row("SELECT name FROM projects", [], |row| row.get(0))
            .unwrap();
        assert_eq!(name, "秘密项目");
    }

    #[test]
    fn database_upgrade_creates_encrypted_pre_migration_snapshot() {
        let directory = tempfile::tempdir().unwrap();
        let data_dir = directory.path().join("data");
        let backup_dir = data_dir.join("backups");
        fs::create_dir_all(&backup_dir).unwrap();
        let database_path = data_dir.join("workbench.sqlite3");
        let key = [23_u8; 32];
        let legacy = open_cipher_connection(&database_path, &key).unwrap();
        legacy
            .execute_batch("CREATE TABLE legacy_note(value TEXT); INSERT INTO legacy_note VALUES('upgrade-safe');")
            .unwrap();
        drop(legacy);

        let upgraded =
            open_database_with_migration_snapshot(&database_path, &key, &backup_dir).unwrap();
        assert_eq!(schema_version(&upgraded).unwrap(), LATEST_SCHEMA_VERSION);
        let snapshot = fs::read_dir(&backup_dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("pre-migration-"))
            })
            .unwrap();
        let plaintext = Connection::open(&snapshot).unwrap();
        assert!(
            plaintext
                .query_row("SELECT value FROM legacy_note", [], |row| row
                    .get::<_, String>(0))
                .is_err()
        );
        drop(plaintext);
        let encrypted = open_cipher_connection(&snapshot, &key).unwrap();
        let value: String = encrypted
            .query_row("SELECT value FROM legacy_note", [], |row| row.get(0))
            .unwrap();
        assert_eq!(value, "upgrade-safe");
    }

    #[test]
    fn schema_one_migration_preserves_logs_and_valid_encrypted_recovery_point() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("v1.sqlite3");
        let backups = directory.path().join("backups");
        fs::create_dir(&backups).unwrap();
        let key = [44; 32];
        let v1 = open_cipher_connection(&path, &key).unwrap();
        run_base_migration(&v1).unwrap();
        v1.execute_batch("INSERT INTO work_logs(id,log_date,title,markdown,minutes,created_at,updated_at) VALUES('old-log','2026-10-03','原记录','不丢失',60,'now','now');").unwrap();
        assert_eq!(schema_version(&v1).unwrap(), 1);
        drop(v1);
        let migrated = open_database_with_migration_snapshot(&path, &key, &backups).unwrap();
        assert_eq!(schema_version(&migrated).unwrap(), 2);
        let logs = crate::repository::list_work_logs(&migrated).unwrap();
        assert_eq!(logs[0].markdown, "不丢失");
        assert_eq!(logs[0].minutes, 60);
        assert!(logs[0].task_id.is_none());
        let snapshot = fs::read_dir(&backups)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let old = open_cipher_connection(&snapshot, &key).unwrap();
        assert_eq!(schema_version(&old).unwrap(), 1);
        validate_connection(&old).unwrap();
        assert!(
            Connection::open(&snapshot)
                .unwrap()
                .query_row("SELECT title FROM work_logs", [], |r| r.get::<_, String>(0))
                .is_err()
        );
        drop(migrated);
        let reopened = open_database_with_migration_snapshot(&path, &key, &backups).unwrap();
        assert_eq!(schema_version(&reopened).unwrap(), 2);
        assert_eq!(fs::read_dir(&backups).unwrap().count(), 1);
    }

    #[test]
    fn schema_two_migration_rolls_back_if_index_creation_fails() {
        let db = Connection::open_in_memory().unwrap();
        run_base_migration(&db).unwrap();
        db.execute_batch("CREATE INDEX idx_work_logs_task ON work_logs(title);")
            .unwrap();
        assert!(run_migrations(&db).is_err());
        assert_eq!(schema_version(&db).unwrap(), 1);
        assert!(db.prepare("SELECT task_id FROM work_logs").is_err());
    }

    #[test]
    fn password_protected_key_round_trip_rejects_wrong_password() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::new(directory.path().join("private-data")).unwrap();
        assert_eq!(
            fs::metadata(&state.data_dir).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(&state.backup_dir)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        let key = [11_u8; 32];
        store_password_protected_key(directory.path(), &key, "correct horse battery").unwrap();
        let metadata = fs::metadata(directory.path().join(PASSWORD_KEY_FILE)).unwrap();
        assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
        let restored =
            read_password_protected_key(directory.path(), "correct horse battery").unwrap();
        assert_eq!(restored, key);
        let wrong =
            read_password_protected_key(directory.path(), "incorrect horse battery").unwrap_err();
        assert_eq!(wrong.code, "LOCAL_PASSWORD_INCORRECT");
    }

    #[test]
    fn changing_local_password_preserves_database_and_locks_workspace() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::new(directory.path().to_path_buf()).unwrap();
        state
            .initialize_with_password("old correct horse battery")
            .unwrap();
        state
            .with_connection(|connection| {
                connection.execute("INSERT INTO projects(id,name,area,status,color,notes,created_at,updated_at) VALUES('kept','保留的数据','work','active','#000','','now','now')", [])?;
                Ok(())
            })
            .unwrap();

        let status = state
            .change_local_password(
                "old correct horse battery",
                "new correct river lantern battery",
            )
            .unwrap();
        assert!(!status.unlocked);
        assert_eq!(status.keychain_mode, "passphrase");
        assert_eq!(
            state
                .unlock_with_password("old correct horse battery")
                .unwrap_err()
                .code,
            "LOCAL_PASSWORD_INCORRECT"
        );
        state
            .unlock_with_password("new correct river lantern battery")
            .unwrap();
        let name: String = state
            .with_connection(|connection| {
                Ok(connection.query_row(
                    "SELECT name FROM projects WHERE id='kept'",
                    [],
                    |row| row.get(0),
                )?)
            })
            .unwrap();
        assert_eq!(name, "保留的数据");
        assert!(
            fs::read_dir(directory.path().join("backups"))
                .unwrap()
                .filter_map(Result::ok)
                .any(|entry| entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("pre-password-change-"))
        );
    }

    #[test]
    fn wrong_current_password_does_not_replace_key_file() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::new(directory.path().to_path_buf()).unwrap();
        state
            .initialize_with_password("original horse battery staple")
            .unwrap();
        let before = fs::read(directory.path().join(PASSWORD_KEY_FILE)).unwrap();

        let error = state
            .change_local_password(
                "incorrect horse battery staple",
                "replacement river lantern staple",
            )
            .unwrap_err();
        assert_eq!(error.code, "LOCAL_PASSWORD_INCORRECT");
        assert_eq!(
            fs::read(directory.path().join(PASSWORD_KEY_FILE)).unwrap(),
            before
        );
        state.lock();
        state
            .unlock_with_password("original horse battery staple")
            .unwrap();
    }
}
#[test]
fn resource_limited_key_decryption_is_not_misreported_as_wrong_password() {
    let limited = local_key_decryption_error(age::DecryptError::ExcessiveWork {
        required: 18,
        target: 10,
    });
    assert_eq!(limited.code, "LOCAL_KEY_RESOURCE_LIMIT");
    assert_eq!(
        local_key_decryption_error(age::DecryptError::DecryptionFailed).code,
        "LOCAL_PASSWORD_INCORRECT"
    );
}
