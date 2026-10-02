//! Durable database replacement and recovery of interrupted (including legacy) restores.
use std::{
    fs::{self, File, OpenOptions},
    io,
    os::unix::fs::OpenOptionsExt,
    path::Path,
};

use uuid::Uuid;

use crate::{
    database::{open_cipher_connection_read_only, restrict_file_permissions, validate_connection},
    error::{CommandError, CommandResult},
};

pub(crate) const PREVIOUS_DATABASE: &str = "database-before-restore.sqlite3.tmp";
pub(crate) const STAGED_DATABASE: &str = "restore.sqlite3.tmp";

pub(crate) fn has_restore_artifacts(data_dir: &Path) -> bool {
    [PREVIOUS_DATABASE, STAGED_DATABASE].iter().any(|name| {
        ["", "-wal", "-shm"]
            .iter()
            .any(|suffix| fs::symlink_metadata(data_dir.join(format!("{name}{suffix}"))).is_ok())
    })
}

fn regular_file_exists(path: &Path) -> CommandResult<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => Ok(true),
        Ok(_) => Err(CommandError::new(
            "UNSAFE_RESTORE_FILE",
            "恢复文件路径不是普通文件，已停止自动处理",
        )
        .with_recovery("请保留数据目录并检查异常文件；不要删除数据库或密钥。")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

pub(crate) fn sync_directory(directory: &Path) -> CommandResult<()> {
    File::open(directory)?.sync_all()?;
    Ok(())
}

pub(crate) fn sync_database(path: &Path) -> CommandResult<()> {
    if !regular_file_exists(path)? {
        return Err(CommandError::new(
            "RESTORE_FILE_MISSING",
            "恢复数据库文件不存在",
        ));
    }
    restrict_file_permissions(path)?;
    File::open(path)?.sync_all()?;
    Ok(())
}

/// Never overwrite an existing recovery file, including a dangling symlink.
pub(crate) fn copy_database(source: &Path, destination: &Path) -> CommandResult<()> {
    if !regular_file_exists(source)? {
        return Err(CommandError::new(
            "RESTORE_FILE_MISSING",
            "待保留的数据库文件不存在",
        ));
    }
    let mut input = File::open(source)?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(destination)?;
    io::copy(&mut input, &mut output)?;
    output.sync_all()?;
    Ok(())
}

fn validate_candidate(path: &Path, key: &[u8]) -> CommandResult<()> {
    if !regular_file_exists(path)? {
        return Err(CommandError::new(
            "RESTORE_FILE_MISSING",
            "待验证的数据库文件不存在",
        ));
    }
    let connection = open_cipher_connection_read_only(path, key)?;
    validate_connection(&connection)
}

fn validate_artifact_paths(data_dir: &Path, database_path: &Path) -> CommandResult<()> {
    for name in [PREVIOUS_DATABASE, STAGED_DATABASE] {
        for suffix in ["", "-wal", "-shm"] {
            regular_file_exists(&data_dir.join(format!("{name}{suffix}")))?;
        }
    }
    for suffix in ["", "-wal", "-shm"] {
        regular_file_exists(&database_path.with_file_name(format!(
            "{}{}",
            database_path.file_name().unwrap_or_default().to_string_lossy(),
            suffix
        )))?;
    }
    Ok(())
}

/// Preserve interrupted candidates as user-visible recovery points, including WAL sidecars.
fn archive_candidate(source: &Path, backup_dir: &Path, copy: bool) -> CommandResult<()> {
    let mut has_sidecars = false;
    for suffix in ["-wal", "-shm"] {
        has_sidecars |= regular_file_exists(&source.with_file_name(format!(
            "{}{}",
            source.file_name().unwrap_or_default().to_string_lossy(),
            suffix
        )))?;
    }
    // A multi-file candidate must not appear as a self-contained snapshot in the restore picker.
    // Preserve the whole group for diagnosis; ordinary snapshot restoration only copies one file.
    let extension = if has_sidecars {
        "sqlite3.pending"
    } else {
        "sqlite3"
    };
    let name = format!("interrupted-restore-{}.{extension}", Uuid::now_v7());
    let destination = backup_dir.join(&name);
    for suffix in ["", "-wal", "-shm"] {
        let source = source.with_file_name(format!(
            "{}{}",
            source.file_name().unwrap_or_default().to_string_lossy(),
            suffix
        ));
        if regular_file_exists(&source)? {
            let destination = destination.with_file_name(format!("{name}{suffix}"));
            if copy {
                copy_database(&source, &destination)?;
            } else {
                restrict_file_permissions(&source)?;
                fs::rename(&source, &destination)?;
                sync_database(&destination)?;
            }
        }
    }
    sync_directory(backup_dir)?;
    if let Some(parent) = source.parent() {
        sync_directory(parent)?;
    }
    Ok(())
}

fn remove_sidecars(path: &Path) -> CommandResult<()> {
    for suffix in ["-wal", "-shm"] {
        let sidecar = path.with_file_name(format!(
            "{}{}",
            path.file_name().unwrap_or_default().to_string_lossy(),
            suffix
        ));
        if regular_file_exists(&sidecar)? {
            fs::remove_file(sidecar)?;
        }
    }
    Ok(())
}

pub(crate) fn rollback_to_previous(
    data_dir: &Path,
    database_path: &Path,
    backup_dir: &Path,
    key: &[u8],
) -> CommandResult<()> {
    validate_artifact_paths(data_dir, database_path)?;
    let previous = data_dir.join(PREVIOUS_DATABASE);
    validate_candidate(&previous, key)?;
    // Previous databases were checkpointed before replacement. Do not silently drop WAL data.
    for suffix in ["-wal", "-shm"] {
        if regular_file_exists(&data_dir.join(format!("{PREVIOUS_DATABASE}{suffix}")))? {
            return Err(CommandError::new(
                "RESTORE_RECOVERY_REQUIRED",
                "恢复前数据库还包含日志文件，无法安全自动回滚",
            )
            .with_recovery("所有文件均已保留，请保留数据目录并排查恢复状态。"));
        }
    }
    archive_candidate(database_path, backup_dir, true)?;
    remove_sidecars(database_path)?;
    sync_database(&previous)?;
    // Same-directory rename replaces the current path atomically; the old file never goes missing.
    fs::rename(&previous, database_path)?;
    sync_directory(data_dir)?;
    validate_candidate(database_path, key)?;
    Ok(())
}

pub(crate) fn recover_interrupted_restore(
    data_dir: &Path,
    database_path: &Path,
    backup_dir: &Path,
    key: &[u8],
) -> CommandResult<Option<String>> {
    if !has_restore_artifacts(data_dir) {
        return Ok(None);
    }
    validate_artifact_paths(data_dir, database_path)?;
    let notice = if validate_candidate(database_path, key).is_ok() {
        "检测到上次恢复中断，已验证并保留当前数据库；遗留候选文件已保存在本地备份目录，请核对后再使用。"
    } else {
        let previous = data_dir.join(PREVIOUS_DATABASE);
        if validate_candidate(&previous, key).is_err() {
            return Err(CommandError::new(
                "RESTORE_RECOVERY_REQUIRED",
                "上次恢复未完成，无法验证当前数据库或恢复前数据库",
            )
            .with_recovery(
                "未创建空库或修改密钥。请保留全部数据文件，并核对密钥或使用已验证备份排查。",
            ));
        }
        rollback_to_previous(data_dir, database_path, backup_dir, key)?;
        "检测到上次恢复中断，已安全回滚到恢复前数据库；未完成的候选文件已保存在本地备份目录，请核对后再使用。"
    };
    archive_candidate(&data_dir.join(PREVIOUS_DATABASE), backup_dir, false)?;
    archive_candidate(&data_dir.join(STAGED_DATABASE), backup_dir, false)?;
    Ok(Some(notice.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::{AppState, open_database};

    const KEY: [u8; 32] = [61; 32];

    fn candidate(path: &Path, name: &str) {
        let connection = open_database(path, &KEY).unwrap();
        connection.execute("INSERT INTO projects(id,name,area,status,color,notes,created_at,updated_at) VALUES('p',?1,'work','active','#000','','now','now')", [name]).unwrap();
        drop(connection);
    }

    fn recover(state: &AppState) -> CommandResult<Option<String>> {
        recover_interrupted_restore(
            &state.data_dir,
            &state.database_path,
            &state.backup_dir,
            &KEY,
        )
    }

    fn project(path: &Path) -> String {
        open_cipher_connection_read_only(path, &KEY)
            .unwrap()
            .query_row("SELECT name FROM projects WHERE id='p'", [], |row| {
                row.get(0)
            })
            .unwrap()
    }

    #[test]
    fn legacy_missing_main_recovers_previous_and_preserves_staged_candidate() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::new(directory.path().to_owned()).unwrap();
        candidate(&state.data_dir.join(PREVIOUS_DATABASE), "before");
        candidate(&state.data_dir.join(STAGED_DATABASE), "after");
        assert!(state.status().initialized);
        assert!(recover(&state).unwrap().unwrap().contains("回滚"));
        assert_eq!(project(&state.database_path), "before");
        let backups = crate::backup::list_backups(&state).unwrap();
        assert_eq!(backups.len(), 1);
        assert_eq!(project(&state.backup_dir.join(&backups[0].name)), "after");
        assert!(!has_restore_artifacts(&state.data_dir));
        assert!(recover(&state).unwrap().is_none());
        assert_eq!(project(&state.database_path), "before");
    }

    #[test]
    fn valid_primary_wins_and_both_unused_candidates_are_preserved() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::new(directory.path().to_owned()).unwrap();
        candidate(&state.database_path, "committed");
        candidate(&state.data_dir.join(PREVIOUS_DATABASE), "before");
        candidate(&state.data_dir.join(STAGED_DATABASE), "uncommitted");
        recover(&state).unwrap();
        assert_eq!(project(&state.database_path), "committed");
        let mut names: Vec<_> = crate::backup::list_backups(&state)
            .unwrap()
            .iter()
            .map(|backup| project(&state.backup_dir.join(&backup.name)))
            .collect();
        names.sort();
        assert_eq!(names, ["before", "uncommitted"]);
        assert!(!has_restore_artifacts(&state.data_dir));
    }

    #[test]
    fn corrupt_primary_is_preserved_before_rollback() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::new(directory.path().to_owned()).unwrap();
        candidate(&state.data_dir.join(PREVIOUS_DATABASE), "before");
        fs::write(&state.database_path, b"damaged candidate").unwrap();
        recover(&state).unwrap();
        assert_eq!(project(&state.database_path), "before");
        let backups = crate::backup::list_backups(&state).unwrap();
        assert_eq!(backups.len(), 1);
        assert_eq!(
            fs::read(state.backup_dir.join(&backups[0].name)).unwrap(),
            b"damaged candidate"
        );
    }

    #[test]
    fn unverified_or_staged_only_files_never_create_an_empty_database() {
        for previous_exists in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let state = AppState::new(directory.path().to_owned()).unwrap();
            let staged = state.data_dir.join(STAGED_DATABASE);
            candidate(&staged, "uncommitted");
            let staged_bytes = fs::read(&staged).unwrap();
            if previous_exists {
                fs::write(
                    state.data_dir.join(PREVIOUS_DATABASE),
                    b"invalid old database",
                )
                .unwrap();
            }
            assert!(state.status().initialized);
            assert_eq!(
                recover(&state).unwrap_err().code,
                "RESTORE_RECOVERY_REQUIRED"
            );
            assert!(!state.database_path.exists());
            assert_eq!(fs::read(staged).unwrap(), staged_bytes);
            if previous_exists {
                assert_eq!(
                    fs::read(state.data_dir.join(PREVIOUS_DATABASE)).unwrap(),
                    b"invalid old database"
                );
            }
        }
    }

    #[test]
    fn wrong_key_does_not_modify_recovery_candidates() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::new(directory.path().to_owned()).unwrap();
        let previous = state.data_dir.join(PREVIOUS_DATABASE);
        candidate(&previous, "before");
        let before = fs::read(&previous).unwrap();
        assert_eq!(
            recover_interrupted_restore(
                &state.data_dir,
                &state.database_path,
                &state.backup_dir,
                &[62; 32]
            )
            .unwrap_err()
            .code,
            "RESTORE_RECOVERY_REQUIRED"
        );
        assert_eq!(fs::read(previous).unwrap(), before);
        assert!(!state.database_path.exists());
        assert!(
            !state
                .data_dir
                .join(format!("{PREVIOUS_DATABASE}-wal"))
                .exists()
        );
        assert!(
            !state
                .data_dir
                .join(format!("{PREVIOUS_DATABASE}-shm"))
                .exists()
        );
    }

    #[test]
    fn read_only_validation_handles_chinese_and_uri_special_characters() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::new(directory.path().join("恢复 #%? 空格")).unwrap();
        candidate(&state.data_dir.join(PREVIOUS_DATABASE), "before");
        recover(&state).unwrap();
        assert_eq!(project(&state.database_path), "before");
    }

    #[test]
    fn linked_candidate_is_rejected_without_touching_its_target() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::new(directory.path().join("data")).unwrap();
        let outside = directory.path().join("outside");
        fs::write(&outside, b"untouched").unwrap();
        std::os::unix::fs::symlink(&outside, state.data_dir.join(PREVIOUS_DATABASE)).unwrap();
        assert_eq!(recover(&state).unwrap_err().code, "UNSAFE_RESTORE_FILE");
        assert_eq!(fs::read(outside).unwrap(), b"untouched");
        assert!(!state.database_path.exists());
    }

    #[test]
    fn previous_with_uncheckpointed_wal_is_not_silently_renamed() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::new(directory.path().to_owned()).unwrap();
        let previous = state.data_dir.join(PREVIOUS_DATABASE);
        candidate(&previous, "before");
        let connection = open_database(&previous, &KEY).unwrap();
        connection
            .execute("UPDATE projects SET name='in WAL'", [])
            .unwrap();
        assert!(
            state
                .data_dir
                .join(format!("{PREVIOUS_DATABASE}-wal"))
                .exists()
        );
        assert_eq!(
            recover(&state).unwrap_err().code,
            "RESTORE_RECOVERY_REQUIRED"
        );
        assert!(!state.database_path.exists());
        assert_eq!(project(&previous), "in WAL");
        drop(connection);
        recover(&state).unwrap();
        assert_eq!(project(&state.database_path), "in WAL");
    }

    #[test]
    fn archived_wal_group_is_not_offered_as_a_single_file_snapshot() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::new(directory.path().to_owned()).unwrap();
        candidate(&state.database_path, "committed");
        let staged = state.data_dir.join(STAGED_DATABASE);
        candidate(&staged, "staged");
        let connection = open_database(&staged, &KEY).unwrap();
        connection
            .execute("UPDATE projects SET name='staged WAL'", [])
            .unwrap();
        // Use copies to avoid moving files underneath this fixture's live connection.
        archive_candidate(&staged, &state.backup_dir, true).unwrap();
        assert!(crate::backup::list_backups(&state).unwrap().is_empty());
        let names: Vec<_> = fs::read_dir(&state.backup_dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert!(names.iter().any(|name| name.ends_with(".sqlite3.pending")));
        assert!(
            names
                .iter()
                .any(|name| name.ends_with(".sqlite3.pending-wal"))
        );
        let archived = names
            .iter()
            .find(|name| name.ends_with(".sqlite3.pending"))
            .unwrap();
        assert_eq!(project(&state.backup_dir.join(archived)), "staged WAL");
        drop(connection);
    }
}
