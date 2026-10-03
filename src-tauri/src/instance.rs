//! Lifetime exclusive ownership of a data directory, including while the database is locked.
use crate::error::{CommandError, CommandResult};
use std::{
    fs::{self, File, OpenOptions, TryLockError},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::Path,
};

pub(crate) struct InstanceLease {
    _file: File,
}

impl InstanceLease {
    pub fn acquire(directory: &Path) -> CommandResult<Self> {
        let path = directory.join("instance.lock");
        if let Ok(metadata) = fs::symlink_metadata(&path)
            && (!metadata.is_file() || metadata.file_type().is_symlink() || metadata.nlink() != 1)
        {
            return Err(CommandError::new(
                "INSTANCE_LOCK_INVALID",
                "工作台实例锁不是独立的普通文件",
            ));
        }
        // Never truncate or unlink the lock file: all cooperating processes lock the same inode.
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(&path)?;
        let descriptor = file.metadata()?;
        let named = fs::symlink_metadata(&path)?;
        if !named.is_file()
            || named.file_type().is_symlink()
            || named.nlink() != 1
            || descriptor.dev() != named.dev()
            || descriptor.ino() != named.ino()
        {
            return Err(CommandError::new(
                "INSTANCE_LOCK_INVALID",
                "工作台实例锁路径发生变化",
            ));
        }
        match file.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                return Err(CommandError::new(
                    "APP_ALREADY_RUNNING",
                    "该本地工作台已在另一个进程运行",
                )
                .with_recovery("请返回已有窗口；不要同时启动开发版和安装版。"));
            }
            Err(TryLockError::Error(error)) => return Err(error.into()),
        }
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
        Ok(Self { _file: file })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn instance_lock_is_exclusive_and_released_without_deleting_file() {
        let directory = tempfile::tempdir().unwrap();
        let first = InstanceLease::acquire(directory.path()).unwrap();
        assert_eq!(
            InstanceLease::acquire(directory.path()).err().unwrap().code,
            "APP_ALREADY_RUNNING"
        );
        drop(first);
        assert!(directory.path().join("instance.lock").exists());
        assert!(InstanceLease::acquire(directory.path()).is_ok());
    }
    #[test]
    fn instance_lock_rejects_links_without_changing_target() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("target");
        fs::write(&target, b"untouched").unwrap();
        std::os::unix::fs::symlink(&target, directory.path().join("instance.lock")).unwrap();
        assert!(InstanceLease::acquire(directory.path()).is_err());
        assert_eq!(fs::read(target).unwrap(), b"untouched");
    }
    #[test]
    fn instance_child() {
        let Ok(directory) = std::env::var("WORKBENCH_INSTANCE_TEST_DIRECTORY") else {
            return;
        };
        let blocked = InstanceLease::acquire(Path::new(&directory)).err().unwrap();
        assert_eq!(blocked.code, "APP_ALREADY_RUNNING");
    }
    #[test]
    fn instance_another_process_cannot_acquire_same_directory() {
        let directory = tempfile::tempdir().unwrap();
        let _lease = InstanceLease::acquire(directory.path()).unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "instance::tests::instance_child"])
            .env("WORKBENCH_INSTANCE_TEST_DIRECTORY", directory.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
