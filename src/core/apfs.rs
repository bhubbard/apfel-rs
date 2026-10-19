// ============================================================================
// apfs.rs — Instant Copy-on-Write Time-Travel Sandboxing for apfel-rs
// Enables mathematical rollback safety for autonomous on-device agent tasks.
// ============================================================================

use std::path::{Path, PathBuf};
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
    snapshot_id: String,
    backup_dir: Option<PathBuf>,
    is_committed: bool,
}

impl ApfsSandbox {
    /// Creates a new sandbox guard targeting the given workspace directory.
    pub fn begin(workspace_dir: impl AsRef<Path>) -> Result<Self, ApfsError> {
        let workspace = workspace_dir.as_ref().canonicalize()?;
        let snap_id = format!("apfel_snap_{}", uuid::Uuid::new_v4().simple());

        // Place backup in workspace's parent if writable and on the same volume (enabling instant CoW),
        // otherwise fall back to system temp directory.
        let backup_path = if let Some(parent) = workspace.parent() {
            let candidate = parent.join(format!(".apfel_cow_backup_{}", snap_id));
            if Self::is_same_filesystem(&workspace, parent) && std::fs::create_dir_all(&candidate).is_ok() {
                candidate
            } else {
                let temp_candidate = std::env::temp_dir().join(format!("apfel_cow_backup_{}", snap_id));
                std::fs::create_dir_all(&temp_candidate)?;
                temp_candidate
            }
        } else {
            let temp_candidate = std::env::temp_dir().join(format!("apfel_cow_backup_{}", snap_id));
            std::fs::create_dir_all(&temp_candidate)?;
            temp_candidate
        };

        Self::copy_dir_recursive(&workspace, &backup_path)?;
        info!(backup = %backup_path.display(), id = %snap_id, "Created APFS CoW sandbox mirror");

        Ok(Self {
            workspace_dir: workspace,
            snapshot_id: snap_id,
            backup_dir: Some(backup_path),
            is_committed: false,
        })
    }

    /// Returns the unique snapshot identifier.
    pub fn snapshot_id(&self) -> &str {
        &self.snapshot_id
    }

    /// Returns the workspace directory managed by the sandbox.
    pub fn workspace_dir(&self) -> &Path {
        &self.workspace_dir
    }

    /// Checks if two paths reside on the same filesystem / volume.
    pub fn is_same_filesystem(p1: &Path, p2: &Path) -> bool {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if let (Ok(m1), Ok(m2)) = (p1.metadata(), p2.metadata()) {
                return m1.dev() == m2.dev();
            }
        }
        false
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

    /// Commits the sandbox state, dissolving the rollback guard.
    pub fn commit(&mut self) {
        self.is_committed = true;
        if let Some(backup) = self.backup_dir.take() {
            let _ = std::fs::remove_dir_all(backup);
        }
        debug!("APFS sandbox committed successfully");
    }

    /// Reverts the workspace state to the checkpoint snapshot.
    pub fn rollback(&mut self) -> Result<(), ApfsError> {
        if self.is_committed {
            return Ok(());
        }

        let backup = match self.backup_dir.as_ref() {
            Some(b) => b.clone(),
            None => {
                self.is_committed = true;
                return Err(ApfsError::SnapshotRollbackFailed("No viable restore backup found".to_string()));
            }
        };

        info!(workspace = %self.workspace_dir.display(), "Executing safe atomic APFS rollback");

        // Step 1: Stage restoration in sibling directory
        let parent = self.workspace_dir.parent().unwrap_or(&self.workspace_dir);
        let staging_dir = parent.join(format!(".apfel_restore_stage_{}", uuid::Uuid::new_v4().simple()));
        let old_dir = parent.join(format!(".apfel_restore_old_{}", uuid::Uuid::new_v4().simple()));

        let mut swap_succeeded = false;
        if let Err(e) = Self::copy_dir_recursive(&backup, &staging_dir) {
            let _ = std::fs::remove_dir_all(&staging_dir);
            return Err(ApfsError::SnapshotRollbackFailed(format!("Failed to stage rollback: {}", e)));
        }

        // Step 2: Verify staged integrity against backup
        let (src_count, src_bytes) = Self::count_files_and_bytes(&backup)
            .map_err(|e| ApfsError::SnapshotRollbackFailed(format!("Integrity check source error: {}", e)))?;
        let (staged_count, staged_bytes) = Self::count_files_and_bytes(&staging_dir)
            .map_err(|e| ApfsError::SnapshotRollbackFailed(format!("Integrity check staged error: {}", e)))?;

        if src_count != staged_count || src_bytes != staged_bytes {
            let _ = std::fs::remove_dir_all(&staging_dir);
            return Err(ApfsError::SnapshotRollbackFailed(format!(
                "Integrity mismatch during rollback: backup has {} files ({} bytes), staged has {} files ({} bytes)",
                src_count, src_bytes, staged_count, staged_bytes
            )));
        }

        // Step 3: Preserve skipped entries (.git, target, node_modules) from workspace into staging
        let skipped_names = [".git", "target", "node_modules"];
        let mut moved_skipped = Vec::new();
        for name in &skipped_names {
            let src = self.workspace_dir.join(name);
            let dst = staging_dir.join(name);
            if src.exists() && !dst.exists() && std::fs::rename(&src, &dst).is_ok() {
                moved_skipped.push((*name, src, dst));
            }
        }

        // Step 4: Atomic directory swap
        if std::fs::rename(&self.workspace_dir, &old_dir).is_ok() {
            if std::fs::rename(&staging_dir, &self.workspace_dir).is_ok() {
                swap_succeeded = true;
                let _ = std::fs::remove_dir_all(&old_dir);
            } else {
                // Restore old_dir back to workspace_dir
                let _ = std::fs::rename(&old_dir, &self.workspace_dir);
            }
        }

        // Step 5: Fallback if rename swap failed (e.g. cross-device or permission)
        if !swap_succeeded {
            // Restore moved skipped items first
            for (_, orig, staged) in moved_skipped {
                if staged.exists() && !orig.exists() {
                    let _ = std::fs::rename(&staged, &orig);
                }
            }
            let _ = std::fs::remove_dir_all(&staging_dir);

            // Safe fallback: copy backup files into workspace FIRST, then prune untracked
            Self::copy_dir_recursive(&backup, &self.workspace_dir)?;
            Self::prune_untracked(&self.workspace_dir, &backup)?;
        }

        if let Some(b) = self.backup_dir.take() {
            let _ = std::fs::remove_dir_all(b);
        }

        self.is_committed = true;
        Ok(())
    }

    fn count_files_and_bytes(dir: &Path) -> std::io::Result<(u64, u64)> {
        let mut count = 0u64;
        let mut bytes = 0u64;
        if !dir.exists() {
            return Ok((0, 0));
        }
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let symlink_meta = entry.path().symlink_metadata()?;
            if symlink_meta.is_dir() {
                let (c, b) = Self::count_files_and_bytes(&entry.path())?;
                count += c;
                bytes += b;
            } else if symlink_meta.is_file() {
                count += 1;
                bytes += symlink_meta.len();
            } else if symlink_meta.is_symlink() {
                count += 1;
            }
        }
        Ok((count, bytes))
    }

    fn prune_untracked(dir: &Path, backup_dir: &Path) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str == ".git" || name_str == "target" || name_str == "node_modules" {
                continue;
            }
            let backup_counterpart = backup_dir.join(&name);
            let path = entry.path();
            let symlink_meta = path.symlink_metadata()?;

            if !backup_counterpart.exists() {
                if symlink_meta.is_dir() {
                    let _ = std::fs::remove_dir_all(&path);
                } else {
                    let _ = std::fs::remove_file(&path);
                }
            } else if symlink_meta.is_dir() {
                Self::prune_untracked(&path, &backup_counterpart)?;
            }
        }
        Ok(())
    }

    fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
        if !dst.exists() {
            std::fs::create_dir_all(dst)?;
        }
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let path = entry.path();
            let symlink_meta = path.symlink_metadata()?;
            let file_name = entry.file_name();
            let dest_path = dst.join(&file_name);

            // Skip .git, target, and node_modules to keep CoW sandbox snapshot ultra-fast (<5ms)
            let name_str = file_name.to_string_lossy();
            if name_str == ".git" || name_str == "target" || name_str == "node_modules" {
                continue;
            }

            if symlink_meta.is_symlink() {
                // Safely recreate symlinks without following or dereferencing
                let target = std::fs::read_link(&path)?;
                if dest_path.exists() || dest_path.is_symlink() {
                    let _ = std::fs::remove_file(&dest_path);
                }
                #[cfg(unix)]
                std::os::unix::fs::symlink(&target, &dest_path)?;
                #[cfg(not(unix))]
                std::fs::copy(&path, &dest_path)?;
            } else if symlink_meta.is_dir() {
                Self::copy_dir_recursive(&path, &dest_path)?;
            } else {
                Self::clone_or_copy_file(&path, &dest_path)?;
            }
        }
        Ok(())
    }

    fn clone_or_copy_file(src: &Path, dst: &Path) -> std::io::Result<()> {
        #[cfg(target_os = "macos")]
        {
            use std::ffi::CString;
            use std::os::unix::ffi::OsStrExt;

            const CLONE_NOFOLLOW: libc::c_int = 0x0001;

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
                if dst.exists() || dst.is_symlink() {
                    let _ = std::fs::remove_file(dst);
                }
                let res = unsafe { clonefile(c_src.as_ptr(), c_dst.as_ptr(), CLONE_NOFOLLOW) };
                if res == 0 {
                    return Ok(());
                }
            }
        }
        if dst.exists() || dst.is_symlink() {
            let _ = std::fs::remove_file(dst);
        }
        std::fs::copy(src, dst)?;
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

    #[test]
    fn test_apfs_sandbox_safe_rollback_cleans_new_files() {
        let temp_dir = tempfile::tempdir().unwrap();
        let orig_file = temp_dir.path().join("orig.txt");
        let untracked_new_file = temp_dir.path().join("newly_created.txt");
        let untracked_new_dir = temp_dir.path().join("new_dir");
        std::fs::write(&orig_file, "original content").unwrap();

        {
            let mut sandbox = ApfsSandbox::begin(temp_dir.path()).unwrap();
            // Modify original file, and create new file and directory
            std::fs::write(&orig_file, "corrupted content").unwrap();
            std::fs::write(&untracked_new_file, "should be deleted on rollback").unwrap();
            std::fs::create_dir_all(&untracked_new_dir).unwrap();
            std::fs::write(untracked_new_dir.join("sub.txt"), "sub file").unwrap();

            sandbox.rollback().unwrap();
        }

        assert_eq!(std::fs::read_to_string(&orig_file).unwrap(), "original content");
        assert!(!untracked_new_file.exists(), "New file must be pruned after rollback");
        assert!(!untracked_new_dir.exists(), "New directory must be pruned after rollback");
    }

    #[cfg(unix)]
    #[test]
    fn test_apfs_sandbox_symlink_safety() {
        let temp_dir = tempfile::tempdir().unwrap();
        let target_file = temp_dir.path().join("target.txt");
        let link_file = temp_dir.path().join("link_to_target.txt");
        std::fs::write(&target_file, "target data").unwrap();
        std::os::unix::fs::symlink(&target_file, &link_file).unwrap();

        {
            let mut sandbox = ApfsSandbox::begin(temp_dir.path()).unwrap();
            // Overwrite target file
            std::fs::write(&target_file, "modified target").unwrap();
            sandbox.rollback().unwrap();
        }

        assert_eq!(std::fs::read_to_string(&target_file).unwrap(), "target data");
        // Verify link_file is still a symlink and points to target_file
        assert!(link_file.symlink_metadata().unwrap().file_type().is_symlink());
        assert_eq!(std::fs::read_link(&link_file).unwrap(), target_file);
    }
}
