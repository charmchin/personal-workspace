use std::{
    fs,
    io::{Cursor, Read, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::Mutex,
};

use age::{Decryptor, Encryptor, Identity, secrecy::SecretString};
use chrono::{Datelike, Local, SecondsFormat, Utc};
use rand::RngCore;
use rusqlite::{Connection, OptionalExtension, params};
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
const LATEST_SCHEMA_VERSION: i64 = 1;

pub struct RuntimeState {
    pub connection: Option<Connection>,
    pub key: Option<Zeroizing<Vec<u8>>>,
    pub snapshot_warning: Option<String>,
}

pub struct AppState {
    pub runtime: Mutex<RuntimeState>,
    pub data_dir: PathBuf,
    pub database_path: PathBuf,
    pub backup_dir: PathBuf,
}

impl AppState {
    pub fn new(data_dir: PathBuf) -> CommandResult<Self> {
        fs::create_dir_all(&data_dir)?;
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
            database_path,
            data_dir,
            backup_dir,
            runtime: Mutex::new(RuntimeState {
                connection: None,
                key: None,
                snapshot_warning: None,
            }),
        })
    }

    pub fn status(&self) -> SecurityStatus {
        let runtime = self
            .runtime
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        SecurityStatus {
            initialized: self.database_path.exists(),
            unlocked: runtime.connection.is_some(),
            database_path: self.database_path.to_string_lossy().to_string(),
            keychain_mode: self.keychain_mode(),
            snapshot_warning: runtime.snapshot_warning.clone(),
        }
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

    pub fn unlock(&self) -> CommandResult<SecurityStatus> {
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
            return Ok(SecurityStatus {
                initialized: true,
                unlocked: true,
                database_path: self.database_path.to_string_lossy().to_string(),
                keychain_mode: self.keychain_mode(),
                snapshot_warning: runtime.snapshot_warning.clone(),
            });
        }

        let key = match read_database_key() {
            Ok(key) => key,
            Err(error)
                if error.code() == ERR_SEC_ITEM_NOT_FOUND && !self.database_path.exists() =>
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
        };

        let connection =
            open_database_with_migration_snapshot(&self.database_path, &key, &self.backup_dir)?;
        runtime.key = Some(Zeroizing::new(key));
        runtime.connection = Some(connection);

        runtime.snapshot_warning =
            if let (Some(connection), Some(key)) = (&runtime.connection, &runtime.key) {
                create_automatic_snapshots(connection, key, &self.backup_dir)
                    .err()
                    .map(|error| format!("自动快照未能完成：{}", error.message))
            } else {
                None
            };

        Ok(SecurityStatus {
            initialized: true,
            unlocked: true,
            database_path: self.database_path.to_string_lossy().to_string(),
            keychain_mode: self.keychain_mode(),
            snapshot_warning: runtime.snapshot_warning.clone(),
        })
    }

    pub fn initialize_with_password(&self, password: &str) -> CommandResult<SecurityStatus> {
        if self.database_path.exists() {
            return Err(CommandError::new(
                "ALREADY_INITIALIZED",
                "工作台已经初始化，不能重新生成数据库密钥",
            ));
        }
        let mut key = vec![0_u8; 32];
        rand::rng().fill_bytes(&mut key);
        store_password_protected_key(&self.data_dir, &key, password)?;
        self.open_with_key(key)
    }

    pub fn unlock_with_password(&self, password: &str) -> CommandResult<SecurityStatus> {
        let key = read_password_protected_key(&self.data_dir, password)?;
        self.open_with_key(key)
    }

    pub fn change_local_password(
        &self,
        current_password: &str,
        new_password: &str,
    ) -> CommandResult<SecurityStatus> {
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

        runtime.connection = None;
        runtime.key = None;
        runtime.snapshot_warning = None;
        Ok(self.status_without_lock(false))
    }

    fn open_with_key(&self, key: Vec<u8>) -> CommandResult<SecurityStatus> {
        let mut runtime = self
            .runtime
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if runtime.connection.is_some() {
            return Ok(self.status_without_lock(true));
        }
        let connection =
            open_database_with_migration_snapshot(&self.database_path, &key, &self.backup_dir)?;
        runtime.key = Some(Zeroizing::new(key));
        runtime.connection = Some(connection);
        runtime.snapshot_warning =
            if let (Some(connection), Some(key)) = (&runtime.connection, &runtime.key) {
                create_automatic_snapshots(connection, key, &self.backup_dir)
                    .err()
                    .map(|error| format!("自动快照未能完成：{}", error.message))
            } else {
                None
            };
        Ok(SecurityStatus {
            initialized: true,
            unlocked: true,
            database_path: self.database_path.to_string_lossy().to_string(),
            keychain_mode: self.keychain_mode(),
            snapshot_warning: runtime.snapshot_warning.clone(),
        })
    }

    fn status_without_lock(&self, unlocked: bool) -> SecurityStatus {
        SecurityStatus {
            initialized: self.database_path.exists(),
            unlocked,
            database_path: self.database_path.to_string_lossy().to_string(),
            keychain_mode: self.keychain_mode(),
            snapshot_warning: None,
        }
    }

    pub fn lock(&self) -> SecurityStatus {
        let mut runtime = self
            .runtime
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if let Some(connection) = runtime.connection.take() {
            let _ = connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
        }
        runtime.key = None;
        SecurityStatus {
            initialized: self.database_path.exists(),
            unlocked: false,
            database_path: self.database_path.to_string_lossy().to_string(),
            keychain_mode: self.keychain_mode(),
            snapshot_warning: runtime.snapshot_warning.clone(),
        }
    }

    pub fn with_connection<T>(
        &self,
        operation: impl FnOnce(&Connection) -> CommandResult<T>,
    ) -> CommandResult<T> {
        let runtime = self
            .runtime
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let connection = runtime
            .connection
            .as_ref()
            .ok_or_else(CommandError::locked)?;
        operation(connection)
    }

    pub fn with_connection_mut<T>(
        &self,
        operation: impl FnOnce(&mut Connection) -> CommandResult<T>,
    ) -> CommandResult<T> {
        let mut runtime = self
            .runtime
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let connection = runtime
            .connection
            .as_mut()
            .ok_or_else(CommandError::locked)?;
        operation(connection)
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
        .map_err(|_| {
            CommandError::new("LOCAL_PASSWORD_INCORRECT", "工作台口令错误")
                .with_recovery("当前数据库未被修改，请检查口令后重试。")
        })?;
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
    let key_hex = hex::encode(key);
    connection.execute_batch(&format!(
        "PRAGMA key = \"x'{key_hex}'\";\
         PRAGMA cipher_memory_security = ON;\
         PRAGMA foreign_keys = ON;\
         PRAGMA busy_timeout = 5000;"
    ))?;

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

fn schema_version(connection: &Connection) -> CommandResult<i64> {
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

fn run_migrations(connection: &Connection) -> CommandResult<()> {
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
    let now = Local::now();
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
    if destination.exists() {
        restrict_file_permissions(destination)?;
        return Ok(());
    }
    let temporary = destination.with_extension("tmp");
    if temporary.exists() {
        fs::remove_file(&temporary)?;
    }
    export_encrypted_copy(source, key, &temporary)?;
    let target = open_cipher_connection(&temporary, key)?;
    let integrity: String = target.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
    if integrity != "ok" {
        drop(target);
        let _ = fs::remove_file(&temporary);
        return Err(CommandError::new(
            "SNAPSHOT_INTEGRITY_FAILED",
            "自动快照完整性检查失败",
        ));
    }
    drop(target);
    fs::rename(temporary, destination)?;
    restrict_file_permissions(destination)?;
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
    let key_hex = hex::encode(key);
    source.execute_batch(&format!(
        "ATTACH DATABASE '{escaped_path}' AS workbench_export KEY \"x'{key_hex}'\";\
         SELECT sqlcipher_export('workbench_export');\
         DETACH DATABASE workbench_export;"
    ))?;
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
    let tables = [
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
    let mut total = 0_i64;
    for table in tables {
        total += connection.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
            row.get::<_, i64>(0)
        })?;
    }
    Ok(total)
}

pub fn validate_connection(connection: &Connection) -> CommandResult<()> {
    let integrity: String = connection.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
    if integrity != "ok" {
        return Err(CommandError::new(
            "DATABASE_INTEGRITY_FAILED",
            format!("数据库完整性检查未通过：{integrity}"),
        ));
    }
    let version: Option<i64> = connection
        .query_row("SELECT max(version) FROM schema_migrations", [], |row| {
            row.get::<_, Option<i64>>(0)
        })
        .optional()?
        .flatten();
    if version.unwrap_or(0) < 1 {
        return Err(CommandError::new(
            "BACKUP_VERSION_UNSUPPORTED",
            "备份数据库缺少受支持的结构版本",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
