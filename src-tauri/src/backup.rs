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
        AppState, RuntimeState, database_record_count, export_encrypted_copy, now_utc,
        open_database, prune_snapshots, restrict_file_permissions, validate_connection,
    },
    error::{CommandError, CommandResult},
    models::{BackupInfo, BackupManifest},
};

const BACKUP_FORMAT_VERSION: i64 = 1;
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
        validate_connection(connection)?;
        let manifest = BackupManifest {
            format_version: BACKUP_FORMAT_VERSION,
            created_at: now_utc(),
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            record_count: database_record_count(connection)?,
        };
        Ok((plaintext_snapshot(connection)?, manifest))
    })?;

    let database_bytes = Zeroizing::new(database_bytes);
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
        let path = entry.path()?.to_string_lossy().to_string();
        match path.as_str() {
            "manifest.json" => {
                let mut bytes = vec![];
                entry.read_to_end(&mut bytes)?;
                manifest = Some(serde_json::from_slice(&bytes)?);
            }
            "database.sqlite3" => {
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
    let manifest = manifest.ok_or_else(|| CommandError::new("INVALID_BACKUP", "备份缺少清单"))?;
    if manifest.format_version != BACKUP_FORMAT_VERSION {
        return Err(CommandError::new(
            "BACKUP_VERSION_UNSUPPORTED",
            format!("暂不支持备份格式版本 {}", manifest.format_version),
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
    let target = open_database(path, key)?;
    validate_connection(&target)?;
    let record_count = database_record_count(&target)?;
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
    Ok(())
}

fn replace_database_from_temporary(
    state: &AppState,
    runtime: &mut RuntimeState,
    key: &[u8],
    temporary: &Path,
) -> CommandResult<()> {
    let replaced = state.data_dir.join("database-before-restore.sqlite3.tmp");
    if replaced.exists() {
        fs::remove_file(&replaced)?;
    }
    let current = runtime
        .connection
        .as_ref()
        .ok_or_else(CommandError::locked)?;
    current.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
    runtime.connection = None;

    for suffix in ["-wal", "-shm"] {
        let sidecar = PathBuf::from(format!("{}{}", state.database_path.display(), suffix));
        if sidecar.exists()
            && let Err(error) = fs::remove_file(&sidecar)
        {
            reopen_current_database(state, runtime, key)?;
            return Err(CommandError::new(
                "RESTORE_SIDECAR_CLEANUP_FAILED",
                format!("无法安全清理数据库临时文件：{error}"),
            )
            .with_recovery("当前数据库未被替换，请关闭占用文件的程序后重试。"));
        }
    }

    if let Err(error) = fs::rename(&state.database_path, &replaced) {
        reopen_current_database(state, runtime, key)?;
        return Err(CommandError::new(
            "RESTORE_PREPARE_FAILED",
            format!("无法准备数据库替换：{error}"),
        ));
    }
    if let Err(error) = fs::rename(temporary, &state.database_path) {
        fs::rename(&replaced, &state.database_path).map_err(|rollback| {
            CommandError::new(
                "RESTORE_ROLLBACK_FAILED",
                format!("数据库替换失败且无法自动回滚：{error}；回滚错误：{rollback}"),
            )
            .with_recovery("请勿继续操作，保留数据目录并使用恢复前快照。")
        })?;
        reopen_current_database(state, runtime, key)?;
        return Err(CommandError::new(
            "RESTORE_REPLACE_FAILED",
            format!("替换数据库失败：{error}"),
        ));
    }

    match open_database(&state.database_path, key) {
        Ok(connection) => runtime.connection = Some(connection),
        Err(open_error) => {
            fs::remove_file(&state.database_path).map_err(|remove_error| {
                CommandError::new(
                    "RESTORE_ROLLBACK_FAILED",
                    format!("新数据库无法打开，且无法移除以执行回滚：{open_error}; {remove_error}"),
                )
                .with_recovery("请勿继续操作，保留数据目录并使用恢复前快照。")
            })?;
            fs::rename(&replaced, &state.database_path).map_err(|rollback| {
                CommandError::new(
                    "RESTORE_ROLLBACK_FAILED",
                    format!("新数据库无法打开，回滚旧数据库失败：{open_error}; {rollback}"),
                )
                .with_recovery("请勿继续操作，保留数据目录并使用恢复前快照。")
            })?;
            reopen_current_database(state, runtime, key)?;
            return Err(CommandError::new(
                "RESTORE_OPEN_FAILED",
                format!("恢复后的数据库无法打开：{open_error}"),
            ));
        }
    }
    if replaced.exists() {
        // 恢复已成功时，旧临时库的清理失败不应被误报为恢复失败。
        let _ = fs::remove_file(replaced);
    }
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
    let (manifest, bytes) = read_backup(source, password)?;
    let bytes = Zeroizing::new(bytes);
    let mut memory = Connection::open_in_memory()?;
    memory.deserialize_read_exact(MAIN_DB, Cursor::new(&bytes), bytes.len(), false)?;
    validate_connection(&memory)?;
    let restored_count = database_record_count(&memory)?;
    if restored_count != manifest.record_count {
        return Err(CommandError::new(
            "BACKUP_COUNT_MISMATCH",
            "备份记录数校验失败，已拒绝恢复",
        ));
    }

    let mut runtime = state
        .runtime
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let key = Zeroizing::new(
        runtime
            .key
            .as_ref()
            .ok_or_else(CommandError::locked)?
            .to_vec(),
    );
    let temporary = state.data_dir.join("restore.sqlite3.tmp");
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
    if let Err(error) = replace_database_from_temporary(state, &mut runtime, &key, &temporary) {
        remove_temporary_database(&temporary);
        return Err(error);
    }
    Ok(manifest)
}

pub fn restore_snapshot(state: &AppState, name: &str) -> CommandResult<i64> {
    let source = snapshot_path(state, name)?;
    let mut runtime = state
        .runtime
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let key = Zeroizing::new(
        runtime
            .key
            .as_ref()
            .ok_or_else(CommandError::locked)?
            .to_vec(),
    );
    let temporary = state.data_dir.join("restore.sqlite3.tmp");
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
    if let Err(error) = replace_database_from_temporary(state, &mut runtime, &key, &temporary) {
        remove_temporary_database(&temporary);
        return Err(error);
    }
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
