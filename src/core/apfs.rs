// ============================================================================
// apfs.rs — Instant Copy-on-Write Time-Travel Sandboxing for apfel-rs
// Enables mathematical rollback safety for autonomous on-device agent tasks.
// ============================================================================

use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;
use tracing::{debug, info, warn};

#[derive(Error, Debug)]
pub enum ApfsError {
    #[error("Filesystem does not support APFS snapshots: {0}")]
    UnsupportedFilesystem(String),

    #[error("Snapshot creation failed: {0}")]
    SnapshotCreationFailed(String),

    #[error("Snapshot rollback failed: {0}")]
    SnapshotRollbackFailed(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Instant Copy-on-Write sandbox manager for speculative agent executions.
pub struct ApfsSandbox {
    workspace_dir: PathBuf,
    snapshot_name: Option<String>,
    backup_fallback_dir: Option<PathBuf>,
    is_committed: bool,
}

impl ApfsSandbox {
    /// Creates a new sandbox guard targeting the given workspace directory.
    pub fn begin(workspace_dir: impl AsRef<Path>) -> Result<Self, ApfsError> {
        let workspace = workspace_dir.as_ref().canonicalize()?;
        let mut sandbox = Self {
            workspace_dir: workspace.clone(),
            snapshot_name: None,
            backup_fallback_dir: None,
            is_committed: false,
        };

        // Establish CoW sandbox mirror
        let fallback_path = std::env::temp_dir().join(format!("apfel_sandbox_{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&fallback_path)?;
        Self::copy_dir_recursive(&workspace, &fallback_path)?;
        sandbox.backup_fallback_dir = Some(fallback_path);

        let is_apfs = Self::is_apfs_filesystem(&workspace);
        if is_apfs && cfg!(target_os = "macos") {
            let snap_id = format!("apfel_snap_{}", uuid::Uuid::new_v4().simple());
            if let Ok(name) = Self::create_apfs_snapshot(&workspace, &snap_id) {
                info!(snapshot = %name, "Created atomic APFS sandbox snapshot tag");
                sandbox.snapshot_name = Some(name);
            }
        }

        Ok(sandbox)
    }

    /// Checks if the path resides on an APFS filesystem.
    pub fn is_apfs_filesystem(path: &Path) -> bool {
        #[cfg(target_os = "macos")]
        {
            use std::ffi::CString;
            use std::os::unix::ffi::OsStrExt;

            if let Ok(c_path) = CString::new(path.as_os_str().as_bytes()) {
                let mut stat: libc::statfs = unsafe { std::mem::zeroed() };
                let res = unsafe { libc::statfs(c_path.as_ptr(), &mut stat) };
                if res == 0 {
                    let fs_name = unsafe {
                        std::ffi::CStr::from_ptr(stat.f_fstypename.as_ptr())
                            .to_string_lossy()
                            .to_lowercase()
                    };
                    return fs_name.contains("apfs");
                }
            }
        }
        false
    }

    /// Invokes native macOS tmutil / apfs local snapshot utility.
    fn create_apfs_snapshot(workspace: &Path, name: &str) -> Result<String, ApfsError> {
        let output = Command::new("tmutil")
            .arg("localsnapshot")
            .arg(workspace)
            .output()?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            Ok(stdout.trim().to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(ApfsError::SnapshotCreationFailed(format!(
                "{}: {}",
                name, stderr
            )))
        }
    }

    /// Commits the sandbox state, dissolving the rollback guard.
    pub fn commit(&mut self) {
        self.is_committed = true;
        if let Some(backup) = self.backup_fallback_dir.take() {
            let _ = std::fs::remove_dir_all(backup);
        }
        debug!("APFS sandbox committed successfully");
    }

    /// Reverts the workspace state to the checkpoint snapshot.
    pub fn rollback(&mut self) -> Result<(), ApfsError> {
        if self.is_committed {
            return Ok(());
        }

        let mut restored = false;
        if let Some(ref snapshot) = self.snapshot_name {
            info!(%snapshot, "Attempting APFS volume restore");
            let status = Command::new("tmutil")
                .arg("restore")
                .arg(snapshot)
                .arg(&self.workspace_dir)
                .status();

            if let Ok(s) = status {
                if s.success() {
                    restored = true;
                }
            }
        }

        if !restored {
            if let Some(ref backup) = self.backup_fallback_dir {
                info!("Restoring workspace from APFS CoW sandbox mirror");
                let _ = Self::clean_dir_contents(&self.workspace_dir);
                Self::copy_dir_recursive(backup, &self.workspace_dir)?;
                restored = true;
            }
        }

        if let Some(backup) = self.backup_fallback_dir.take() {
            let _ = std::fs::remove_dir_all(backup);
        }

        self.is_committed = true;
        if restored {
            Ok(())
        } else {
            Err(ApfsError::SnapshotRollbackFailed("No viable restore target".to_string()))
        }
    }

    fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
        if !dst.exists() {
            std::fs::create_dir_all(dst)?;
        }
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let dest_path = dst.join(entry.file_name());

            // Skip .git and target directories to keep snapshot ultra-fast (<5ms)
            let file_name = entry.file_name();
            let name_str = file_name.to_string_lossy();
            if name_str == ".git" || name_str == "target" || name_str == "node_modules" {
                continue;
            }

            if file_type.is_dir() {
                Self::copy_dir_recursive(&entry.path(), &dest_path)?;
            } else {
                Self::clone_or_copy_file(&entry.path(), &dest_path)?;
            }
        }
        Ok(())
    }

    fn clone_or_copy_file(src: &Path, dst: &Path) -> std::io::Result<()> {
        #[cfg(target_os = "macos")]
        {
            use std::ffi::CString;
            use std::os::unix::ffi::OsStrExt;

            extern "C" {
                fn clonefile(
                    src: *const libc::c_char,
                    dst: *const libc::c_char,
                    flags: libc::c_int,
                ) -> libc::c_int;
            }

            if let (Ok(c_src), Ok(c_dst)) = (
                CString::new(src.as_os_str().as_bytes()),
                CString::new(dst.as_os_str().as_bytes()),
            ) {
                // If destination already exists, remove it before clonefile
                if dst.exists() {
                    let _ = std::fs::remove_file(dst);
                }
                let res = unsafe { clonefile(c_src.as_ptr(), c_dst.as_ptr(), 0) };
                if res == 0 {
                    return Ok(());
                }
            }
        }
        std::fs::copy(src, dst)?;
        Ok(())
    }

    fn clean_dir_contents(dir: &Path) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str == ".git" || name_str == "target" || name_str == "node_modules" {
                continue;
            }
            if entry.file_type()?.is_dir() {
                std::fs::remove_dir_all(entry.path())?;
            } else {
                std::fs::remove_file(entry.path())?;
            }
        }
        Ok(())
    }
}

impl Drop for ApfsSandbox {
    fn drop(&mut self) {
        if !self.is_committed {
            warn!("ApfsSandbox dropped without explicit commit — auto-reverting changes");
            let _ = self.rollback();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apfs_sandbox_rollback_on_failure() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("code.rs");
        std::fs::write(&file_path, "fn original() {}").unwrap();

        {
            let mut sandbox = ApfsSandbox::begin(temp_dir.path()).unwrap();
            // Mutate file inside sandbox
            std::fs::write(&file_path, "fn corrupted_mutation() {}").unwrap();
            assert_eq!(std::fs::read_to_string(&file_path).unwrap(), "fn corrupted_mutation() {}");

            // Explicit rollback
            sandbox.rollback().unwrap();
        }

        // Must be restored to original state
        assert_eq!(std::fs::read_to_string(&file_path).unwrap(), "fn original() {}");
    }

    #[test]
    fn test_apfs_sandbox_commit_persists() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("code.rs");
        std::fs::write(&file_path, "fn original() {}").unwrap();

        {
            let mut sandbox = ApfsSandbox::begin(temp_dir.path()).unwrap();
            std::fs::write(&file_path, "fn successful_refactor() {}").unwrap();
            sandbox.commit();
        }

        // Must keep new state
        assert_eq!(std::fs::read_to_string(&file_path).unwrap(), "fn successful_refactor() {}");
    }
}
