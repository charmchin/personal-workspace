use std::{
    fs::{self, File},
    io::{BufReader, Cursor, Read},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use age::{Decryptor, Encryptor, Identity, secrecy::SecretString};
use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{Connection, MAIN_DB};
use tar::{Archive, Builder, Header};
use zeroize::Zeroizing;

use crate::{
    database::{
        AppState, LATEST_SCHEMA_VERSION, RuntimeState, database_record_count,
        database_table_counts, export_encrypted_copy, now_utc, open_cipher_connection_read_only,
        open_database, prune_snapshots, restrict_file_permissions, validate_connection,
    },
    error::{CommandError, CommandResult},
    models::{BackupInfo, BackupManifest},
    restore::{self, PREVIOUS_DATABASE, STAGED_DATABASE},
};

const BACKUP_FORMAT_VERSION: i64 = 2;
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
const MAX_BACKUP_FILE_BYTES: u64 = 2 * 1024 * 1024 * 1024 + 16 * 1024 * 1024;

fn validate_password(password: &str) -> CommandResult<()> {
    if password.chars().count() < 10 {
        return Err(
            CommandError::new("WEAK_BACKUP_PASSWORD", "备份密码至少需要 10 个字符")
                .with_recovery("建议使用至少四个随机词或更长的独立密码。"),
        );
    }
    Ok(())
}

fn append_bytes<W: std::io::Write>(
    archive: &mut Builder<W>,
    name: &str,
    bytes: &[u8],
) -> CommandResult<()> {
    let mut header = Header::new_gnu();
    header.set_size(bytes.len() as u64);
    header.set_mode(0o600);
    header.set_mtime(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    header.set_cksum();
    archive.append_data(&mut header, name, Cursor::new(bytes))?;
    Ok(())
}

fn plaintext_snapshot(connection: &Connection) -> CommandResult<Vec<u8>> {
    connection.execute_batch(
        "ATTACH DATABASE ':memory:' AS workbench_plaintext KEY '';\
         SELECT sqlcipher_export('workbench_plaintext');",
    )?;
    let bytes = {
        let data = connection.serialize("workbench_plaintext")?;
        data.to_vec()
    };
    connection.execute_batch("DETACH DATABASE workbench_plaintext;")?;
    Ok(bytes)
}

pub fn export_backup(
    state: &AppState,
    destination: &Path,
    password: &str,
) -> CommandResult<BackupManifest> {
    let epoch = state.session.require_active()?;
    validate_password(password)?;
    if destination.extension().and_then(|value| value.to_str()) != Some("workbench-backup") {
        return Err(CommandError::new(
            "INVALID_BACKUP_PATH",
            "备份文件必须使用 .workbench-backup 扩展名",
        ));
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }

    let (database_bytes, manifest) = state.with_connection(|connection| {
        validate_backup_database(connection)?;
        let bytes = Zeroizing::new(plaintext_snapshot(connection)?);
        let mut snapshot = Connection::open_in_memory()?;
        snapshot.deserialize_read_exact(MAIN_DB, Cursor::new(&*bytes), bytes.len(), true)?;
        validate_backup_database(&snapshot)?;
        let manifest = BackupManifest {
            format_version: BACKUP_FORMAT_VERSION,
            created_at: now_utc(),
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            record_count: database_record_count(&snapshot)?,
            schema_version: Some(LATEST_SCHEMA_VERSION),
            table_record_counts: Some(database_table_counts(&snapshot)?),
        };
        Ok((bytes, manifest))
    })?;

    let temporary = destination.with_extension("workbench-backup.tmp");
    if temporary.exists() {
        fs::remove_file(&temporary)?;
    }
    let output = File::create(&temporary)?;
    restrict_file_permissions(&temporary)?;
    let encryptor = Encryptor::with_user_passphrase(SecretString::from(password.to_owned()));
    let encrypted_writer = encryptor.wrap_output(output).map_err(|error| {
        CommandError::new(
            "BACKUP_ENCRYPTION_FAILED",
            format!("无法初始化备份加密：{error}"),
        )
    })?;
    let mut archive = Builder::new(encrypted_writer);
    let manifest_json = serde_json::to_vec_pretty(&manifest)?;
    append_bytes(&mut archive, "manifest.json", &manifest_json)?;
    append_bytes(&mut archive, "database.sqlite3", &database_bytes)?;
    archive.finish()?;
    let encrypted_writer = archive.into_inner()?;
    encrypted_writer.finish().map_err(|error| {
        CommandError::new(
            "BACKUP_ENCRYPTION_FAILED",
            format!("无法完成备份加密：{error}"),
        )
    })?;
    if let Err(error) = state.session.require_epoch(epoch) {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    fs::rename(&temporary, destination)?;
    restrict_file_permissions(destination)?;
    Ok(manifest)
}

fn read_backup(path: &Path, password: &str) -> CommandResult<(BackupManifest, Vec<u8>)> {
    validate_password(password)?;
    if path.extension().and_then(|value| value.to_str()) != Some("workbench-backup") {
        return Err(CommandError::new(
            "INVALID_BACKUP_PATH",
            "只能恢复 .workbench-backup 备份文件",
        ));
    }
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(CommandError::new(
            "INVALID_BACKUP_FILE",
            "备份路径不是可读取的普通文件",
        ));
    }
    if metadata.len() > MAX_BACKUP_FILE_BYTES {
        return Err(CommandError::new(
            "BACKUP_TOO_LARGE",
            "备份文件超过 2 GB 安全限制",
        ));
    }
    let input = BufReader::new(File::open(path)?);
    let decryptor = Decryptor::new(input)
        .map_err(|_| CommandError::new("INVALID_BACKUP", "文件不是有效的个人工作台备份"))?;
    let identity = age::scrypt::Identity::new(SecretString::from(password.to_owned()));
    let reader = decryptor
        .decrypt(std::iter::once(&identity as &dyn Identity))
        .map_err(|_| {
            CommandError::new("BACKUP_PASSWORD_INCORRECT", "备份密码错误或文件已损坏")
                .with_recovery("当前数据库未被修改，请检查密码后重试。")
        })?;
    let mut archive = Archive::new(reader);
    let mut manifest: Option<BackupManifest> = None;
    let mut database: Option<Vec<u8>> = None;
    for entry in archive.entries()? {
        let mut entry = entry?;
        if !entry.header().entry_type().is_file() {
            return Err(CommandError::new(
                "INVALID_BACKUP",
                "备份包含非普通文件，已拒绝读取",
            ));
        }
        let path = entry.path()?.to_string_lossy().to_string();
        match path.as_str() {
            "manifest.json" => {
                if manifest.is_some() || entry.size() > MAX_MANIFEST_BYTES {
                    return Err(CommandError::new(
                        "INVALID_BACKUP",
                        "备份清单重复或超过大小限制",
                    ));
                }
                let mut bytes = vec![];
                entry.read_to_end(&mut bytes)?;
                manifest = Some(serde_json::from_slice(&bytes)?);
            }
            "database.sqlite3" => {
                if database.is_some() {
                    return Err(CommandError::new(
                        "INVALID_BACKUP",
                        "备份包含重复数据库文件",
                    ));
                }
                let size = entry.size();
                if size > 2 * 1024 * 1024 * 1024 {
                    return Err(CommandError::new(
                        "BACKUP_TOO_LARGE",
                        "备份数据库超过 2 GB 安全限制",
                    ));
                }
                let mut bytes = Vec::with_capacity(size as usize);
                entry.read_to_end(&mut bytes)?;
                database = Some(bytes);
            }
            _ => {
                return Err(CommandError::new(
                    "INVALID_BACKUP",
                    "备份中包含未识别的文件，已拒绝读取",
                ));
            }
        }
    }
    // Tar stops at its end marker before the age reader necessarily consumes
    // the final authenticated chunk. Drain and validate all remaining bytes.
    let mut reader = archive.into_inner();
    let mut tail = [0_u8; 8192];
    loop {
        let count = reader.read(&mut tail)?;
        if count == 0 {
            break;
        }
        if tail[..count].iter().any(|byte| *byte != 0) {
            return Err(CommandError::new(
                "INVALID_BACKUP",
                "备份归档结束后存在未识别的数据",
            ));
        }
    }
    let manifest = manifest.ok_or_else(|| CommandError::new("INVALID_BACKUP", "备份缺少清单"))?;
    if !matches!(manifest.format_version, 1 | BACKUP_FORMAT_VERSION) {
        return Err(CommandError::new(
            "BACKUP_VERSION_UNSUPPORTED",
            format!("暂不支持备份格式版本 {}", manifest.format_version),
        ));
    }
    if manifest.record_count < 0
        || (manifest.format_version == BACKUP_FORMAT_VERSION
            && (!manifest
                .schema_version
                .is_some_and(|version| (1..=LATEST_SCHEMA_VERSION).contains(&version))
                || manifest.table_record_counts.is_none()))
    {
        return Err(CommandError::new(
            "INVALID_BACKUP",
            "备份缺少有效的结构版本或逐表校验清单",
        ));
    }
    let database = database.ok_or_else(|| CommandError::new("INVALID_BACKUP", "备份缺少数据库"))?;
    Ok((manifest, database))
}

fn create_restore_point(state: &AppState, current: &Connection, key: &[u8]) -> CommandResult<()> {
    let timestamp = Utc::now().format("%Y%m%dT%H%M%S%.3fZ");
    let recovery_path = state
        .backup_dir
        .join(format!("pre-restore-{timestamp}.sqlite3"));
    crate::database::create_encrypted_snapshot(current, key, &recovery_path)?;
    prune_snapshots(&state.backup_dir, "pre-restore-", 5)
}

fn validate_temporary_database(path: &Path, key: &[u8]) -> CommandResult<i64> {
    // Validate without migrations first: open_database would create missing
    // tables/columns and could silently hide a damaged snapshot.
    let candidate = open_cipher_connection_read_only(path, key)?;
    validate_backup_database(&candidate)?;
    let record_count = database_record_count(&candidate)?;
    drop(candidate);
    let target = open_database(path, key)?;
    target.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
    drop(target);
    for suffix in ["-wal", "-shm"] {
        let sidecar = PathBuf::from(format!("{}{}", path.display(), suffix));
        if sidecar.exists() {
            fs::remove_file(sidecar)?;
        }
    }
    restrict_file_permissions(path)?;
    Ok(record_count)
}

fn validate_backup_database(connection: &Connection) -> CommandResult<()> {
    validate_connection(connection)?;
    crate::repository::validate_portfolio_for_backup(connection).map_err(|_| {
        CommandError::new(
            "BACKUP_PORTFOLIO_INVALID",
            "备份中的投资交易序列或估值无法通过校验，已拒绝恢复",
        )
        .with_recovery("当前数据库未被替换，请检查原始交易和价格，或选择其他完整备份。")
    })?;
    Ok(())
}

fn validate_manifest(connection: &Connection, manifest: &BackupManifest) -> CommandResult<()> {
    if database_record_count(connection)? != manifest.record_count {
        return Err(CommandError::new(
            "BACKUP_COUNT_MISMATCH",
            "备份总记录数校验失败，已拒绝恢复",
        ));
    }
    if let Some(version) = manifest.schema_version
        && version != crate::database::schema_version(connection)?
    {
        return Err(CommandError::new(
            "BACKUP_VERSION_UNSUPPORTED",
            "备份清单中的数据库结构版本不受支持",
        ));
    }
    if let Some(expected) = &manifest.table_record_counts
        && database_table_counts(connection)? != *expected
    {
        return Err(CommandError::new(
            "BACKUP_COUNT_MISMATCH",
            "备份逐表记录数校验失败，已拒绝恢复",
        )
        .with_recovery("当前数据库未被替换，请选择未经修改且记录完整的备份。"));
    }
    Ok(())
}

fn remove_temporary_database(path: &Path) {
    for suffix in ["", "-wal", "-shm"] {
        let candidate = PathBuf::from(format!("{}{}", path.display(), suffix));
        if fs::symlink_metadata(&candidate).is_ok_and(|metadata| {
            metadata.file_type().is_file() || metadata.file_type().is_symlink()
        }) {
            let _ = fs::remove_file(candidate);
        }
    }
}

fn reopen_current_database(
    state: &AppState,
    runtime: &mut RuntimeState,
    key: &[u8],
) -> CommandResult<()> {
    runtime.connection = Some(open_database(&state.database_path, key)?);
    runtime.snapshot_stamp = None;
    Ok(())
}

fn ensure_no_pending_restore(state: &AppState) -> CommandResult<()> {
    if restore::has_restore_artifacts(&state.data_dir) {
        return Err(CommandError::new(
            "RESTORE_RECOVERY_PENDING",
            "检测到尚未处理的恢复文件，已停止新的恢复",
        )
        .with_recovery("请先锁定并重新解锁，系统将验证并处理遗留恢复文件。"));
    }
    Ok(())
}

fn replace_database_from_temporary(
    state: &AppState,
    runtime: &mut RuntimeState,
    key: &[u8],
    temporary: &Path,
) -> CommandResult<()> {
    replace_database_with_checkpoints(state, runtime, key, temporary, |_| {})
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RestoreCheckpoint {
    PreviousPersisted,
    Replaced,
    Reopened,
    Finalized,
}

fn replace_database_with_checkpoints(
    state: &AppState,
    runtime: &mut RuntimeState,
    key: &[u8],
    temporary: &Path,
    mut checkpoint: impl FnMut(RestoreCheckpoint),
) -> CommandResult<()> {
    let replaced = state.data_dir.join(PREVIOUS_DATABASE);
    if fs::symlink_metadata(&replaced).is_ok() {
        return Err(CommandError::new(
            "RESTORE_RECOVERY_PENDING",
            "检测到尚未处理的恢复前数据库，已停止新的恢复",
        )
        .with_recovery("请先锁定并重新解锁，系统将验证并处理遗留恢复文件。"));
    }
    // Persist the validated candidate before the original database is touched.
    restore::sync_database(temporary)?;
    restore::sync_directory(&state.data_dir)?;
    let current = runtime
        .connection
        .as_ref()
        .ok_or_else(CommandError::locked)?;
    let busy: i64 = current.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| row.get(0))?;
    if busy != 0 {
        return Err(CommandError::new(
            "RESTORE_DATABASE_BUSY",
            "数据库仍被其他操作占用，已停止恢复",
        )
        .with_recovery("请关闭其他工作台窗口或开发副本后重试。"));
    }
    runtime.connection = None;

    for suffix in ["-wal", "-shm"] {
        let sidecar = PathBuf::from(format!("{}{}", state.database_path.display(), suffix));
        let cleanup = restrict_file_permissions(&sidecar).and_then(|_| {
            if sidecar.exists() {
                fs::remove_file(&sidecar)?;
            }
            Ok(())
        });
        if let Err(error) = cleanup {
            reopen_current_database(state, runtime, key)?;
            return Err(CommandError::new(
                "RESTORE_SIDECAR_CLEANUP_FAILED",
                format!("无法安全清理数据库临时文件：{error}"),
            )
            .with_recovery("当前数据库未被替换，请关闭占用文件的程序后重试。"));
        }
    }

    // A separately persisted copy permits rollback without ever removing the main path.
    let preparation = restore::sync_database(&state.database_path)
        .and_then(|_| restore::copy_database(&state.database_path, &replaced))
        .and_then(|_| restore::sync_directory(&state.data_dir));
    if let Err(error) = preparation {
        reopen_current_database(state, runtime, key)?;
        return Err(CommandError::new(
            "RESTORE_PREPARE_FAILED",
            format!("无法准备数据库替换：{error}"),
        )
        .with_recovery("当前数据库未被替换；请锁定并重新解锁后重试。"));
    }
    checkpoint(RestoreCheckpoint::PreviousPersisted);
    if let Err(error) = fs::rename(temporary, &state.database_path) {
        reopen_current_database(state, runtime, key)?;
        return Err(CommandError::new(
            "RESTORE_REPLACE_FAILED",
            format!("替换数据库失败：{error}"),
        )
        .with_recovery("原数据库仍保留在主路径，请锁定并重新解锁后重试。"));
    }
    checkpoint(RestoreCheckpoint::Replaced);
    let reopened = restore::sync_directory(&state.data_dir)
        .and_then(|_| open_database(&state.database_path, key));
    match reopened {
        Ok(connection) => {
            runtime.connection = Some(connection);
            runtime.snapshot_stamp = None;
        }
        Err(open_error) => {
            restore::rollback_to_previous(
                &state.data_dir,
                &state.database_path,
                &state.backup_dir,
                key,
            )
            .map_err(|rollback| {
                CommandError::new(
                    "RESTORE_ROLLBACK_FAILED",
                    format!("新数据库无法打开或持久化，回滚失败：{open_error}; {rollback}"),
                )
                .with_recovery("全部候选文件均保留，请锁定并重新打开工作台以验证恢复状态。")
            })?;
            reopen_current_database(state, runtime, key)?;
            return Err(CommandError::new(
                "RESTORE_OPEN_FAILED",
                format!("恢复未能完成，已回滚原数据库：{open_error}"),
            ));
        }
    }
    checkpoint(RestoreCheckpoint::Reopened);
    if fs::symlink_metadata(&replaced).is_ok() {
        // 恢复已成功时，旧临时库的清理失败不应被误报为恢复失败。
        let _ = fs::remove_file(replaced);
        let _ = restore::sync_directory(&state.data_dir);
    }
    checkpoint(RestoreCheckpoint::Finalized);
    Ok(())
}

fn snapshot_path(state: &AppState, name: &str) -> CommandResult<PathBuf> {
    let candidate = Path::new(name);
    let valid_name = !name.is_empty()
        && name.ends_with(".sqlite3")
        && candidate.components().count() == 1
        && candidate.file_name().and_then(|value| value.to_str()) == Some(name);
    if !valid_name {
        return Err(CommandError::new(
            "INVALID_SNAPSHOT_NAME",
            "快照名称无效，已拒绝操作",
        ));
    }
    let path = state.backup_dir.join(name);
    let metadata = fs::symlink_metadata(&path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            CommandError::new("SNAPSHOT_NOT_FOUND", "快照不存在或已被清理")
        } else {
            error.into()
        }
    })?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(CommandError::new(
            "INVALID_SNAPSHOT_FILE",
            "目标不是可操作的本地快照文件",
        ));
    }
    Ok(path)
}

pub fn restore_backup(
    state: &AppState,
    source: &Path,
    password: &str,
) -> CommandResult<BackupManifest> {
    let epoch = state.session.require_active()?;
    let _quotes_restore = state.quotes.restoring();
    let (manifest, bytes) = read_backup(source, password)?;
    let bytes = Zeroizing::new(bytes);
    let mut memory = Connection::open_in_memory()?;
    memory.deserialize_read_exact(MAIN_DB, Cursor::new(&bytes), bytes.len(), false)?;
    validate_backup_database(&memory)?;
    validate_manifest(&memory, &manifest)?;

    let mut runtime = state
        .runtime
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    state.session.require_epoch(epoch)?;
    let key = Zeroizing::new(
        runtime
            .key
            .as_ref()
            .ok_or_else(CommandError::locked)?
            .to_vec(),
    );
    ensure_no_pending_restore(state)?;
    let temporary = state.data_dir.join(STAGED_DATABASE);
    remove_temporary_database(&temporary);
    let preparation = export_encrypted_copy(&memory, &key, &temporary)
        .and_then(|_| validate_temporary_database(&temporary, &key).map(|_| ()));
    if let Err(error) = preparation {
        remove_temporary_database(&temporary);
        return Err(error);
    }
    let current = runtime
        .connection
        .as_ref()
        .ok_or_else(CommandError::locked)?;
    create_restore_point(state, current, &key)?;
    state.session.require_epoch(epoch)?;
    if let Err(error) = replace_database_from_temporary(state, &mut runtime, &key, &temporary) {
        if !state.data_dir.join(PREVIOUS_DATABASE).exists() {
            remove_temporary_database(&temporary);
        }
        return Err(error);
    }
    state.session.require_epoch(epoch)?;
    state.session.configure(
        crate::database::load_settings(
            runtime
                .connection
                .as_ref()
                .ok_or_else(CommandError::locked)?,
        )?
        .lock_minutes,
    )?;
    Ok(manifest)
}

pub fn restore_snapshot(state: &AppState, name: &str) -> CommandResult<i64> {
    let epoch = state.session.require_active()?;
    let _quotes_restore = state.quotes.restoring();
    let source = snapshot_path(state, name)?;
    let mut runtime = state
        .runtime
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    state.session.require_epoch(epoch)?;
    let key = Zeroizing::new(
        runtime
            .key
            .as_ref()
            .ok_or_else(CommandError::locked)?
            .to_vec(),
    );
    ensure_no_pending_restore(state)?;
    let temporary = state.data_dir.join(STAGED_DATABASE);
    remove_temporary_database(&temporary);
    fs::copy(source, &temporary)?;
    restrict_file_permissions(&temporary)?;
    let record_count = match validate_temporary_database(&temporary, &key) {
        Ok(count) => count,
        Err(error) => {
            remove_temporary_database(&temporary);
            return Err(error);
        }
    };
    let current = runtime
        .connection
        .as_ref()
        .ok_or_else(CommandError::locked)?;
    create_restore_point(state, current, &key)?;
    state.session.require_epoch(epoch)?;
    if let Err(error) = replace_database_from_temporary(state, &mut runtime, &key, &temporary) {
        if !state.data_dir.join(PREVIOUS_DATABASE).exists() {
            remove_temporary_database(&temporary);
        }
        return Err(error);
    }
    state.session.require_epoch(epoch)?;
    state.session.configure(
        crate::database::load_settings(
            runtime
                .connection
                .as_ref()
                .ok_or_else(CommandError::locked)?,
        )?
        .lock_minutes,
    )?;
    Ok(record_count)
}

pub fn list_backups(state: &AppState) -> CommandResult<Vec<BackupInfo>> {
    let mut backups = vec![];
    for entry in fs::read_dir(&state.backup_dir)? {
        let entry = entry?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if !name.ends_with(".sqlite3") {
            continue;
        }
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
            continue;
        }
        let created: DateTime<Utc> = metadata
            .modified()
            .map(DateTime::<Utc>::from)
            .unwrap_or_else(|_| Utc::now());
        let kind = if name.starts_with("daily-") {
            "daily"
        } else if name.starts_with("weekly-") {
            "weekly"
        } else if name.starts_with("latest-") {
            "latest"
        } else {
            "recovery"
        };
        backups.push(BackupInfo {
            name: name.to_owned(),
            created_at: created.to_rfc3339_opts(SecondsFormat::Millis, true),
            size_bytes: metadata.len(),
            kind: kind.to_owned(),
        });
    }
    backups.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(backups)
}

pub fn delete_backup(state: &AppState, name: &str) -> CommandResult<()> {
    let path = snapshot_path(state, name)?;
    fs::remove_file(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use zeroize::Zeroizing;

    fn validation_fixture() -> (tempfile::TempDir, AppState) {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::new(directory.path().join("data")).unwrap();
        crash_fixture(&state.database_path, "current-unchanged");
        {
            let mut runtime = state.runtime.lock().unwrap();
            runtime.connection = Some(open_database(&state.database_path, &[64; 32]).unwrap());
            runtime.key = Some(Zeroizing::new(vec![64; 32]));
        }
        state.session.activate(0, 0).unwrap();
        (directory, state)
    }

    fn test_archive(path: &Path, entries: &[(&str, &[u8])]) {
        let encryptor = Encryptor::with_user_passphrase(SecretString::from(
            "test-only-backup-password".to_owned(),
        ));
        let writer = encryptor.wrap_output(File::create(path).unwrap()).unwrap();
        let mut archive = Builder::new(writer);
        for (name, bytes) in entries {
            append_bytes(&mut archive, name, bytes).unwrap();
        }
        archive.finish().unwrap();
        archive.into_inner().unwrap().finish().unwrap();
    }

    fn test_manifest(connection: &Connection) -> BackupManifest {
        BackupManifest {
            format_version: 2,
            created_at: now_utc(),
            app_version: "test".into(),
            record_count: database_record_count(connection).unwrap(),
            schema_version: Some(LATEST_SCHEMA_VERSION),
            table_record_counts: Some(database_table_counts(connection).unwrap()),
        }
    }

    #[test]
    fn format_two_schema_one_backup_restores_then_migrates_without_count_mismatch() {
        let (directory, state) = validation_fixture();
        let old = Connection::open_in_memory().unwrap();
        crate::database::run_migrations(&old).unwrap();
        old.execute_batch("DROP INDEX idx_work_logs_task; ALTER TABLE work_logs DROP COLUMN task_id; DELETE FROM schema_migrations WHERE version=2; INSERT INTO work_logs(id,log_date,title,markdown,minutes,created_at,updated_at) VALUES('old-log','2026-10-03','旧版记录','原文',60,'now','now');").unwrap();
        let mut manifest = test_manifest(&old);
        manifest.schema_version = Some(1);
        let bytes = old.serialize(rusqlite::MAIN_DB).unwrap();
        let json = serde_json::to_vec(&manifest).unwrap();
        let path = directory.path().join("v1.workbench-backup");
        test_archive(
            &path,
            &[("manifest.json", &json), ("database.sqlite3", &bytes)],
        );
        restore_backup(&state, &path, "test-only-backup-password").unwrap();
        state
            .with_connection(|db| {
                assert_eq!(crate::database::schema_version(db)?, 2);
                let logs = crate::repository::list_work_logs(db)?;
                assert_eq!(logs.len(), 1);
                assert_eq!(logs[0].markdown, "原文");
                assert_eq!(logs[0].minutes, 60);
                assert!(logs[0].task_id.is_none());
                Ok(())
            })
            .unwrap();
    }

    fn current_is_unchanged(state: &AppState) {
        assert_eq!(
            state
                .with_connection(|db| db
                    .query_row("SELECT name FROM projects", [], |row| row
                        .get::<_, String>(0))
                    .map_err(Into::into))
                .unwrap(),
            "current-unchanged"
        );
        assert!(!state.data_dir.join(STAGED_DATABASE).exists());
        assert!(!state.data_dir.join(PREVIOUS_DATABASE).exists());
        assert!(list_backups(state).unwrap().is_empty());
    }

    fn table_contents(
        connection: &Connection,
    ) -> std::collections::BTreeMap<String, Vec<Vec<rusqlite::types::Value>>> {
        let mut contents = std::collections::BTreeMap::new();
        for table in database_table_counts(connection).unwrap().keys() {
            let mut statement = connection
                .prepare(&format!("SELECT * FROM {table} ORDER BY 1"))
                .unwrap();
            let columns = statement.column_count();
            let rows = statement
                .query_map([], |row| {
                    (0..columns)
                        .map(|column| row.get(column))
                        .collect::<rusqlite::Result<Vec<rusqlite::types::Value>>>()
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            contents.insert(table.clone(), rows);
        }
        contents
    }

    #[test]
    fn backup_export_delete_test_database_restore_preserves_every_table_and_relation() {
        let (directory, state) = validation_fixture();
        state.with_connection(|db| {
            db.execute_batch("INSERT INTO tasks(id,title,project_id,created_at,updated_at) VALUES('t','关联任务','p','now','now'); INSERT INTO work_logs(id,log_date,project_id,title,created_at,updated_at) VALUES('w','2026-10-03','p','日志','now','now'); INSERT INTO habits(id,name,created_at,updated_at) VALUES('h','阅读','now','now'); INSERT INTO habit_checks(id,habit_id,check_date) VALUES('hc','h','2026-10-03');")?;
            Ok(())
        }).unwrap();
        let expected = state.with_connection(|db| Ok(table_contents(db))).unwrap();
        let path = directory.path().join("complete.workbench-backup");
        export_backup(&state, &path, "test-only-backup-password").unwrap();
        let database_path = state.database_path.clone();
        state.runtime.lock().unwrap().connection = None;
        drop(state);
        fs::remove_file(&database_path).unwrap(); // Only this isolated fixture database.
        assert!(!database_path.exists());
        let restarted = AppState::new(directory.path().join("data")).unwrap();
        {
            let mut runtime = restarted.runtime.lock().unwrap();
            runtime.connection = Some(open_database(&restarted.database_path, &[64; 32]).unwrap());
            runtime.key = Some(Zeroizing::new(vec![64; 32]));
        }
        restarted.session.activate(0, 0).unwrap();
        restore_backup(&restarted, &path, "test-only-backup-password").unwrap();
        assert_eq!(
            restarted
                .with_connection(|db| Ok(table_contents(db)))
                .unwrap(),
            expected
        );
        restarted.with_connection(validate_backup_database).unwrap();
    }

    #[test]
    fn backup_new_format_records_all_tables_and_legacy_format_remains_readable() {
        let (directory, state) = validation_fixture();
        let path = directory.path().join("current.workbench-backup");
        let manifest = export_backup(&state, &path, "test-only-backup-password").unwrap();
        let counts = manifest.table_record_counts.as_ref().unwrap();
        assert_eq!(counts.len(), 17);
        assert_eq!(manifest.format_version, 2);
        assert_eq!(manifest.schema_version, Some(LATEST_SCHEMA_VERSION));
        assert_eq!(counts["projects"], 1);
        assert_eq!(counts["settings"], 1);
        let (read_manifest, bytes) = read_backup(&path, "test-only-backup-password").unwrap();
        assert_eq!(
            read_manifest.table_record_counts,
            manifest.table_record_counts
        );
        let mut legacy = manifest.clone();
        legacy.format_version = 1;
        legacy.schema_version = None;
        legacy.table_record_counts = None;
        let legacy_json = serde_json::to_vec(&legacy).unwrap();
        let legacy_path = directory.path().join("legacy.workbench-backup");
        test_archive(
            &legacy_path,
            &[
                ("manifest.json", &legacy_json),
                ("database.sqlite3", &bytes),
            ],
        );
        state
            .with_connection(|db| {
                db.execute("DELETE FROM projects", [])?;
                Ok(())
            })
            .unwrap();
        let restored = restore_backup(&state, &legacy_path, "test-only-backup-password").unwrap();
        assert_eq!(restored.format_version, 1);
        assert!(restored.table_record_counts.is_none());
        assert_eq!(
            state.with_connection(database_table_counts).unwrap(),
            *counts
        );
        state
            .with_connection(|db| {
                db.execute("DELETE FROM projects", [])?;
                Ok(())
            })
            .unwrap();
        restore_backup(&state, &path, "test-only-backup-password").unwrap();
        assert_eq!(
            state.with_connection(database_table_counts).unwrap(),
            *counts
        );
    }

    #[test]
    fn backup_per_table_counts_detect_same_total_swaps_and_incomplete_manifests() {
        let (directory, state) = validation_fixture();
        for change in ["swap", "missing", "negative", "schema", "future", "total"] {
            let (bytes, mut manifest) = state
                .with_connection(|db| Ok((plaintext_snapshot(db)?, test_manifest(db))))
                .unwrap();
            let expected = match change {
                "swap" => {
                    let counts = manifest.table_record_counts.as_mut().unwrap();
                    counts.insert("projects".into(), 0);
                    counts.insert("tasks".into(), 1);
                    "BACKUP_COUNT_MISMATCH"
                }
                "missing" => {
                    manifest.table_record_counts = None;
                    "INVALID_BACKUP"
                }
                "negative" => {
                    manifest
                        .table_record_counts
                        .as_mut()
                        .unwrap()
                        .insert("tasks".into(), -1);
                    "BACKUP_COUNT_MISMATCH"
                }
                "schema" => {
                    manifest.schema_version = Some(999);
                    "INVALID_BACKUP"
                }
                "future" => {
                    manifest.format_version = 999;
                    "BACKUP_VERSION_UNSUPPORTED"
                }
                _ => {
                    manifest.record_count += 1;
                    "BACKUP_COUNT_MISMATCH"
                }
            };
            let json = serde_json::to_vec(&manifest).unwrap();
            let path = directory.path().join(format!("{change}.workbench-backup"));
            test_archive(
                &path,
                &[("manifest.json", &json), ("database.sqlite3", &bytes)],
            );
            assert_eq!(
                restore_backup(&state, &path, "test-only-backup-password")
                    .unwrap_err()
                    .code,
                expected
            );
            current_is_unchanged(&state);
        }
    }

    #[test]
    fn backup_bad_database_and_snapshot_are_rejected_before_migration_or_replacement() {
        let (directory, state) = validation_fixture();
        for (change, expected) in [
            ("DROP TABLE review_snapshots", "BACKUP_SCHEMA_INVALID"),
            (
                "UPDATE schema_migrations SET version=999 WHERE version=2",
                "BACKUP_VERSION_UNSUPPORTED",
            ),
            (
                "PRAGMA foreign_keys=OFF; INSERT INTO habit_checks(id,habit_id,check_date) VALUES('bad','missing','2026-10-03')",
                "BACKUP_FOREIGN_KEY_INVALID",
            ),
            (
                "UPDATE settings SET lock_minutes=999",
                "BACKUP_SCHEMA_INVALID",
            ),
            (
                "INSERT INTO investment_accounts VALUES('a','a','securities','CNY','now','now'); INSERT INTO instruments VALUES('i','test','test','etf','CN','CNY','1','now','now'); INSERT INTO portfolio_transactions VALUES('t','a','i','sell','2026-10-03','1','1','0','0','0','','now','now')",
                "BACKUP_PORTFOLIO_INVALID",
            ),
        ] {
            let candidate_path = state.backup_dir.join("daily-2026-10-03.sqlite3");
            let candidate = open_database(&candidate_path, &[64; 32]).unwrap();
            let manifest = test_manifest(&candidate);
            candidate.execute_batch(change).unwrap();
            let bytes = plaintext_snapshot(&candidate).unwrap();
            candidate
                .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
                .unwrap();
            drop(candidate);
            let encrypted_before = fs::read(&candidate_path).unwrap();
            let json = serde_json::to_vec(&manifest).unwrap();
            let path = directory.path().join("invalid.workbench-backup");
            test_archive(
                &path,
                &[("manifest.json", &json), ("database.sqlite3", &bytes)],
            );
            assert_eq!(
                restore_backup(&state, &path, "test-only-backup-password")
                    .unwrap_err()
                    .code,
                expected
            );
            assert_eq!(
                restore_snapshot(&state, "daily-2026-10-03.sqlite3")
                    .unwrap_err()
                    .code,
                expected
            );
            assert_eq!(fs::read(&candidate_path).unwrap(), encrypted_before);
            fs::remove_file(&candidate_path).unwrap();
            current_is_unchanged(&state);
        }
    }

    #[test]
    fn backup_archive_rejects_duplicate_and_oversized_manifest_entries() {
        let (directory, state) = validation_fixture();
        let (bytes, manifest) = state
            .with_connection(|db| Ok((plaintext_snapshot(db)?, test_manifest(db))))
            .unwrap();
        let json = serde_json::to_vec(&manifest).unwrap();
        let oversized = vec![b' '; MAX_MANIFEST_BYTES as usize + 1];
        for entries in [
            vec![
                ("manifest.json", json.as_slice()),
                ("manifest.json", json.as_slice()),
                ("database.sqlite3", bytes.as_slice()),
            ],
            vec![
                ("manifest.json", json.as_slice()),
                ("database.sqlite3", bytes.as_slice()),
                ("database.sqlite3", bytes.as_slice()),
            ],
            vec![
                ("manifest.json", oversized.as_slice()),
                ("database.sqlite3", bytes.as_slice()),
            ],
        ] {
            let path = directory.path().join("bad-archive.workbench-backup");
            test_archive(&path, &entries);
            assert_eq!(
                restore_backup(&state, &path, "test-only-backup-password")
                    .unwrap_err()
                    .code,
                "INVALID_BACKUP"
            );
            current_is_unchanged(&state);
        }
        let path = directory.path().join("truncated.workbench-backup");
        test_archive(
            &path,
            &[("manifest.json", &json), ("database.sqlite3", &bytes)],
        );
        let mut encrypted = fs::read(&path).unwrap();
        encrypted.truncate(encrypted.len() - 10);
        fs::write(&path, encrypted).unwrap();
        assert!(restore_backup(&state, &path, "test-only-backup-password").is_err());
        current_is_unchanged(&state);

        // Put the final age chunk after the tar EOF; merely iterating entries
        // would never authenticate this chunk and could accept the corruption.
        let mut archive = Builder::new(Vec::new());
        append_bytes(&mut archive, "manifest.json", &json).unwrap();
        append_bytes(&mut archive, "database.sqlite3", &bytes).unwrap();
        archive.finish().unwrap();
        let mut plaintext = archive.into_inner().unwrap();
        plaintext.resize(plaintext.len() + 128 * 1024, 0);
        let encryptor = Encryptor::with_user_passphrase(SecretString::from(
            "test-only-backup-password".to_owned(),
        ));
        let mut writer = encryptor.wrap_output(File::create(&path).unwrap()).unwrap();
        std::io::Write::write_all(&mut writer, &plaintext).unwrap();
        writer.finish().unwrap();
        assert!(read_backup(&path, "test-only-backup-password").is_ok());
        let mut encrypted = fs::read(&path).unwrap();
        let last = encrypted.len() - 1;
        encrypted[last] ^= 1;
        fs::write(&path, encrypted).unwrap();
        assert!(restore_backup(&state, &path, "test-only-backup-password").is_err());
        current_is_unchanged(&state);
    }

    fn crash_fixture(path: &Path, name: &str) {
        let connection = open_database(path, &[64; 32]).unwrap();
        connection.execute("INSERT INTO projects(id,name,area,status,color,notes,created_at,updated_at) VALUES('p',?1,'work','active','#000','','now','now')", [name]).unwrap();
        drop(connection);
    }

    // Run only in the isolated child below. process::exit deliberately skips SQLite destructors.
    #[test]
    #[ignore = "subprocess helper for interrupted_restore_survives_process_exit_at_each_checkpoint"]
    fn interrupted_restore_child() {
        let Some(directory) = std::env::var_os("WORKBENCH_RESTORE_TEST_DIR") else {
            return;
        };
        let phase = std::env::var("WORKBENCH_RESTORE_TEST_CHECKPOINT").unwrap();
        let state = AppState::new(PathBuf::from(directory)).unwrap();
        let key = [64; 32];
        let mut runtime = state.runtime.lock().unwrap();
        runtime.connection = Some(open_database(&state.database_path, &key).unwrap());
        runtime.key = Some(Zeroizing::new(key.to_vec()));
        state.session.activate(0, 0).unwrap();
        replace_database_with_checkpoints(
            &state,
            &mut runtime,
            &key,
            &state.data_dir.join(STAGED_DATABASE),
            |checkpoint| {
                assert!(
                    state.database_path.is_file(),
                    "primary must never disappear"
                );
                if format!("{checkpoint:?}") == phase {
                    std::process::exit(86);
                }
            },
        )
        .unwrap();
        panic!("requested crash checkpoint was not reached");
    }

    #[test]
    fn interrupted_restore_survives_process_exit_at_each_checkpoint() {
        for phase in [
            RestoreCheckpoint::PreviousPersisted,
            RestoreCheckpoint::Replaced,
            RestoreCheckpoint::Reopened,
            RestoreCheckpoint::Finalized,
        ] {
            let directory = tempfile::tempdir().unwrap();
            let state = AppState::new(directory.path().to_owned()).unwrap();
            crash_fixture(&state.database_path, "before");
            crash_fixture(&state.data_dir.join(STAGED_DATABASE), "after");
            drop(state); // The parent releases directory ownership before the crash-test child.
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "backup::tests::interrupted_restore_child",
                    "--ignored",
                    "--nocapture",
                ])
                .env("WORKBENCH_RESTORE_TEST_DIR", directory.path())
                .env("WORKBENCH_RESTORE_TEST_CHECKPOINT", format!("{phase:?}"))
                .output()
                .unwrap();
            assert_eq!(
                output.status.code(),
                Some(86),
                "{phase:?}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(directory.path().join("workbench.sqlite3").is_file());
            let restarted = AppState::new(directory.path().to_owned()).unwrap();
            assert!(restarted.status().initialized);
            restore::recover_interrupted_restore(
                &restarted.data_dir,
                &restarted.database_path,
                &restarted.backup_dir,
                &[64; 32],
            )
            .unwrap();
            let connection = open_database(&restarted.database_path, &[64; 32]).unwrap();
            let name: String = connection
                .query_row("SELECT name FROM projects", [], |row| row.get(0))
                .unwrap();
            assert_eq!(
                name,
                if phase == RestoreCheckpoint::PreviousPersisted {
                    "before"
                } else {
                    "after"
                },
                "{phase:?}"
            );
            assert!(!restore::has_restore_artifacts(&restarted.data_dir));
            drop(connection);
            assert!(
                restore::recover_interrupted_restore(
                    &restarted.data_dir,
                    &restarted.database_path,
                    &restarted.backup_dir,
                    &[64; 32]
                )
                .unwrap()
                .is_none()
            );
        }
    }

    #[test]
    fn new_restore_does_not_erase_pending_candidates() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::new(directory.path().to_owned()).unwrap();
        let key = [64; 32];
        crash_fixture(&state.database_path, "current");
        let snapshot = "daily-2026-10-02.sqlite3";
        crash_fixture(&state.backup_dir.join(snapshot), "snapshot");
        let connection = open_database(&state.database_path, &key).unwrap();
        {
            let mut runtime = state.runtime.lock().unwrap();
            runtime.connection = Some(connection);
            runtime.key = Some(Zeroizing::new(key.to_vec()));
            state.session.activate(0, 0).unwrap();
        }
        let staged = state.data_dir.join(STAGED_DATABASE);
        fs::write(&staged, b"do not erase").unwrap();
        assert_eq!(
            restore_snapshot(&state, snapshot).unwrap_err().code,
            "RESTORE_RECOVERY_PENDING"
        );
        assert_eq!(fs::read(staged).unwrap(), b"do not erase");
        assert!(state.status().unlocked);
    }

    #[test]
    fn failed_reopen_rolls_back_without_discarding_damaged_candidate() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::new(directory.path().to_owned()).unwrap();
        let key = [64; 32];
        crash_fixture(&state.database_path, "before");
        let staged = state.data_dir.join(STAGED_DATABASE);
        crash_fixture(&staged, "after");
        let mut runtime = state.runtime.lock().unwrap();
        runtime.connection = Some(open_database(&state.database_path, &key).unwrap());
        runtime.key = Some(Zeroizing::new(key.to_vec()));
        state.session.activate(0, 0).unwrap();
        let error =
            replace_database_with_checkpoints(&state, &mut runtime, &key, &staged, |phase| {
                if phase == RestoreCheckpoint::PreviousPersisted {
                    fs::write(&staged, b"damaged after validation").unwrap();
                }
                assert!(state.database_path.exists());
            })
            .unwrap_err();
        assert_eq!(error.code, "RESTORE_OPEN_FAILED");
        let name: String = runtime
            .connection
            .as_ref()
            .unwrap()
            .query_row("SELECT name FROM projects", [], |row| row.get(0))
            .unwrap();
        assert_eq!(name, "before");
        drop(runtime);
        let backups = list_backups(&state).unwrap();
        assert_eq!(backups.len(), 1);
        assert_eq!(
            fs::read(state.backup_dir.join(&backups[0].name)).unwrap(),
            b"damaged after validation"
        );
    }

    #[test]
    fn short_backup_password_is_rejected() {
        assert!(validate_password("too-short").is_err());
        assert!(validate_password("a sufficiently long password").is_ok());
    }

    #[test]
    fn encrypted_backup_round_trip_preserves_current_data_on_wrong_password() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::new(directory.path().join("data")).unwrap();
        let key = vec![7_u8; 32];
        let connection = open_database(&state.database_path, &key).unwrap();
        connection.execute("INSERT INTO projects(id,name,area,status,color,notes,created_at,updated_at) VALUES('p1','恢复验证','work','active','#397064','','now','now')", []).unwrap();
        {
            let mut runtime = state.runtime.lock().unwrap();
            runtime.connection = Some(connection);
            runtime.key = Some(Zeroizing::new(key));
            state.session.activate(0, 0).unwrap();
        }

        let backup_path = directory.path().join("roundtrip.workbench-backup");
        let manifest = export_backup(&state, &backup_path, "correct horse battery staple").unwrap();
        assert!(backup_path.exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&backup_path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        assert!(manifest.record_count > 0);
        state
            .with_connection(|db| {
                db.execute("DELETE FROM projects", [])?;
                Ok(())
            })
            .unwrap();

        let wrong = restore_backup(&state, &backup_path, "this password is incorrect").unwrap_err();
        assert_eq!(wrong.code, "BACKUP_PASSWORD_INCORRECT");
        let count_after_wrong: i64 = state
            .with_connection(|db| {
                Ok(db.query_row("SELECT count(*) FROM projects", [], |row| row.get(0))?)
            })
            .unwrap();
        assert_eq!(count_after_wrong, 0);

        restore_backup(&state, &backup_path, "correct horse battery staple").unwrap();
        let restored_name: String = state
            .with_connection(|db| {
                Ok(
                    db.query_row("SELECT name FROM projects WHERE id='p1'", [], |row| {
                        row.get(0)
                    })?,
                )
            })
            .unwrap();
        assert_eq!(restored_name, "恢复验证");
        assert!(
            list_backups(&state)
                .unwrap()
                .iter()
                .any(|item| item.kind == "recovery")
        );
    }

    #[test]
    fn snapshot_deletion_rejects_path_escape_and_removes_only_named_file() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::new(directory.path().join("data")).unwrap();
        let snapshot = state.backup_dir.join("daily-2026-08-30.sqlite3");
        fs::write(&snapshot, b"encrypted snapshot placeholder").unwrap();
        let outside = state.data_dir.join("workbench.sqlite3");
        fs::write(&outside, b"must remain").unwrap();

        let escape = delete_backup(&state, "../workbench.sqlite3").unwrap_err();
        assert_eq!(escape.code, "INVALID_SNAPSHOT_NAME");
        assert!(outside.exists());

        delete_backup(&state, "daily-2026-08-30.sqlite3").unwrap();
        assert!(!snapshot.exists());
        assert!(outside.exists());
        assert_eq!(
            delete_backup(&state, "daily-2026-08-30.sqlite3")
                .unwrap_err()
                .code,
            "SNAPSHOT_NOT_FOUND"
        );
    }

    #[test]
    fn encrypted_snapshot_can_restore_database_and_creates_recovery_point() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::new(directory.path().join("data")).unwrap();
        let key = vec![17_u8; 32];
        let connection = open_database(&state.database_path, &key).unwrap();
        connection.execute("INSERT INTO projects(id,name,area,status,color,notes,created_at,updated_at) VALUES('p1','快照中的项目','work','active','#397064','','now','now')", []).unwrap();
        let snapshot_name = "daily-2026-08-31.sqlite3";
        crate::database::create_encrypted_snapshot(
            &connection,
            &key,
            &state.backup_dir.join(snapshot_name),
        )
        .unwrap();
        connection
            .execute("UPDATE projects SET name='已修改的项目' WHERE id='p1'", [])
            .unwrap();
        {
            let mut runtime = state.runtime.lock().unwrap();
            runtime.connection = Some(connection);
            runtime.key = Some(Zeroizing::new(key));
            state.session.activate(0, 0).unwrap();
        }

        let count = restore_snapshot(&state, snapshot_name).unwrap();
        assert!(count > 0);
        let restored_name: String = state
            .with_connection(|database| {
                Ok(
                    database.query_row("SELECT name FROM projects WHERE id='p1'", [], |row| {
                        row.get(0)
                    })?,
                )
            })
            .unwrap();
        assert_eq!(restored_name, "快照中的项目");
        assert!(
            list_backups(&state)
                .unwrap()
                .iter()
                .any(|item| item.name.starts_with("pre-restore-"))
        );
    }

    #[test]
    fn invalid_snapshot_does_not_replace_current_database_or_leave_temporary_files() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::new(directory.path().join("data")).unwrap();
        let key = vec![18_u8; 32];
        let connection = open_database(&state.database_path, &key).unwrap();
        connection.execute("INSERT INTO projects(id,name,area,status,color,notes,created_at,updated_at) VALUES('safe','当前数据','work','active','#397064','','now','now')", []).unwrap();
        {
            let mut runtime = state.runtime.lock().unwrap();
            runtime.connection = Some(connection);
            runtime.key = Some(Zeroizing::new(key));
            state.session.activate(0, 0).unwrap();
        }
        let snapshot_name = "daily-2026-08-31.sqlite3";
        fs::write(state.backup_dir.join(snapshot_name), b"corrupted").unwrap();

        assert!(restore_snapshot(&state, snapshot_name).is_err());
        let current_name: String = state
            .with_connection(|database| {
                Ok(
                    database.query_row("SELECT name FROM projects WHERE id='safe'", [], |row| {
                        row.get(0)
                    })?,
                )
            })
            .unwrap();
        assert_eq!(current_name, "当前数据");
        assert!(!state.data_dir.join("restore.sqlite3.tmp").exists());
        assert!(!state.data_dir.join("restore.sqlite3.tmp-wal").exists());
        assert!(!state.data_dir.join("restore.sqlite3.tmp-shm").exists());
    }

    #[cfg(unix)]
    #[test]
    fn snapshot_operations_ignore_and_reject_symbolic_links() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let state = AppState::new(directory.path().join("data")).unwrap();
        let outside = directory.path().join("outside.sqlite3");
        fs::write(&outside, b"outside").unwrap();
        let linked_name = "daily-2026-08-31.sqlite3";
        symlink(&outside, state.backup_dir.join(linked_name)).unwrap();

        assert!(
            list_backups(&state)
                .unwrap()
                .iter()
                .all(|item| item.name != linked_name)
        );
        assert_eq!(
            delete_backup(&state, linked_name).unwrap_err().code,
            "INVALID_SNAPSHOT_FILE"
        );
        assert_eq!(fs::read(&outside).unwrap(), b"outside");
    }

    #[test]
    fn startup_keeps_only_five_snapshots_for_each_recovery_kind() {
        let directory = tempfile::tempdir().unwrap();
        let data_dir = directory.path().join("data");
        let state = AppState::new(data_dir.clone()).unwrap();
        for index in 0..7 {
            fs::write(
                state.backup_dir.join(format!(
                    "pre-password-change-20260830T120{index}00Z.sqlite3"
                )),
                b"snapshot",
            )
            .unwrap();
            fs::write(
                state
                    .backup_dir
                    .join(format!("pre-restore-20260830-120{index}00.sqlite3")),
                b"snapshot",
            )
            .unwrap();
        }
        fs::write(state.backup_dir.join("daily-2026-08-30.sqlite3"), b"daily").unwrap();
        drop(state);

        let reopened = AppState::new(data_dir).unwrap();
        let names = fs::read_dir(&reopened.backup_dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            names
                .iter()
                .filter(|name| name.starts_with("pre-password-change-"))
                .count(),
            5
        );
        assert_eq!(
            names
                .iter()
                .filter(|name| name.starts_with("pre-restore-"))
                .count(),
            5
        );
        assert!(names.iter().any(|name| name == "daily-2026-08-30.sqlite3"));
        assert!(!names.iter().any(|name| name.contains("120000")));
        assert!(!names.iter().any(|name| name.contains("120100")));
    }
}
