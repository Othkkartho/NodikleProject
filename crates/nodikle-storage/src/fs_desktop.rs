//! Desktop local filesystem store with atomic writes and optimistic locking.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use async_trait::async_trait;
use nodikle_model::{NodikleError, Result};
use tempfile::NamedTempFile;

use crate::atomic::{
    blake3_hex, is_ignored_path, normalize_prefix, normalize_rel_path, resolve_under_root,
};
use crate::traits::{LocalFileMeta, LocalFileStore};

/// Local filesystem implementation of [`LocalFileStore`] for Desktop vaults.
#[derive(Debug, Clone)]
pub struct DesktopFsStore {
    root: PathBuf,
}

impl DesktopFsStore {
    /// Opens or creates a Desktop vault root directory.
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root_path = root.as_ref().to_path_buf();
        if !root_path.exists() {
            fs::create_dir_all(&root_path)?;
        }
        let canonical = fs::canonicalize(&root_path)?;
        Ok(Self { root: canonical })
    }

    /// Returns the canonicalized Vault root path.
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn collect_entries_sync(&self, prefix: &str) -> Result<Vec<LocalFileMeta>> {
        let norm_prefix = normalize_prefix(prefix)?;
        let mut results = Vec::new();
        self.walk_dir(&self.root, &norm_prefix, &mut results)?;
        results.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
        Ok(results)
    }

    fn walk_dir(&self, dir: &Path, prefix: &str, out: &mut Vec<LocalFileMeta>) -> Result<()> {
        let entries = fs::read_dir(dir)?;
        for entry_res in entries {
            let entry = entry_res?;
            let path = entry.path();
            let file_type = entry.file_type()?;
            if file_type.is_symlink() {
                continue;
            }

            let rel = path
                .strip_prefix(&self.root)
                .map_err(|e| NodikleError::InvalidPath(e.to_string()))?;
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            if is_ignored_path(&rel_str) {
                continue;
            }

            if file_type.is_dir() {
                self.walk_dir(&path, prefix, out)?;
            } else if file_type.is_file() {
                if !prefix.is_empty() && !rel_str.starts_with(prefix) {
                    continue;
                }
                let meta = entry.metadata()?;
                out.push(LocalFileMeta {
                    rel_path: rel_str,
                    size_bytes: meta.len(),
                    mtime_ms: metadata_mtime_ms(&meta),
                    is_dir: false,
                });
            }
        }
        Ok(())
    }
}

fn metadata_mtime_ms(meta: &fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[async_trait]
impl LocalFileStore for DesktopFsStore {
    async fn list_entries(&self, prefix: &str) -> Result<Vec<LocalFileMeta>> {
        let this = self.clone();
        let prefix = prefix.to_string();
        tokio::task::spawn_blocking(move || this.collect_entries_sync(&prefix))
            .await
            .map_err(|e| NodikleError::StorageIo(e.to_string()))?
    }

    async fn stat(&self, rel_path: &str) -> Result<Option<LocalFileMeta>> {
        let norm = normalize_rel_path(rel_path)?;
        let full = resolve_under_root(&self.root, &norm)?;
        tokio::task::spawn_blocking(move || match fs::metadata(&full) {
            Ok(meta) => Ok(Some(LocalFileMeta {
                rel_path: norm,
                size_bytes: meta.len(),
                mtime_ms: metadata_mtime_ms(&meta),
                is_dir: meta.is_dir(),
            })),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(NodikleError::StorageIo(err.to_string())),
        })
        .await
        .map_err(|e| NodikleError::StorageIo(e.to_string()))?
    }

    async fn read_bytes(&self, rel_path: &str) -> Result<Vec<u8>> {
        let norm = normalize_rel_path(rel_path)?;
        let full = resolve_under_root(&self.root, &norm)?;
        tokio::task::spawn_blocking(move || match fs::read(&full) {
            Ok(bytes) => Ok(bytes),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                Err(NodikleError::NotFound(norm))
            }
            Err(err) => Err(NodikleError::StorageIo(err.to_string())),
        })
        .await
        .map_err(|e| NodikleError::StorageIo(e.to_string()))?
    }

    async fn write_atomic(
        &self,
        rel_path: &str,
        bytes: &[u8],
        expected_hash: Option<&str>,
    ) -> Result<String> {
        let norm = normalize_rel_path(rel_path)?;
        let full = resolve_under_root(&self.root, &norm)?;
        let data = bytes.to_vec();
        let expected = expected_hash.map(str::to_string);

        tokio::task::spawn_blocking(move || {
            if let Some(exp) = expected {
                if full.exists() {
                    let existing = fs::read(&full)?;
                    let actual = blake3_hex(&existing);
                    if actual != exp {
                        return Err(NodikleError::OptimisticLock {
                            path: norm,
                            expected: exp,
                            actual,
                        });
                    }
                }
            }

            let parent = full
                .parent()
                .ok_or_else(|| NodikleError::InvalidPath(norm.clone()))?;
            fs::create_dir_all(parent)?;

            let mut tmp = NamedTempFile::new_in(parent)?;
            tmp.write_all(&data)?;
            tmp.as_file().sync_all()?;
            tmp.persist(&full)
                .map_err(|e| NodikleError::StorageIo(e.error.to_string()))?;

            Ok(blake3_hex(&data))
        })
        .await
        .map_err(|e| NodikleError::StorageIo(e.to_string()))?
    }

    async fn delete_entry(&self, rel_path: &str) -> Result<()> {
        let norm = normalize_rel_path(rel_path)?;
        let full = resolve_under_root(&self.root, &norm)?;
        tokio::task::spawn_blocking(move || match fs::remove_file(&full) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                Err(NodikleError::NotFound(norm))
            }
            Err(err) => Err(NodikleError::StorageIo(err.to_string())),
        })
        .await
        .map_err(|e| NodikleError::StorageIo(e.to_string()))?
    }

    async fn rename_entry(&self, from: &str, to: &str) -> Result<()> {
        let from_norm = normalize_rel_path(from)?;
        let to_norm = normalize_rel_path(to)?;
        let from_full = resolve_under_root(&self.root, &from_norm)?;
        let to_full = resolve_under_root(&self.root, &to_norm)?;

        tokio::task::spawn_blocking(move || {
            if !from_full.exists() {
                return Err(NodikleError::NotFound(from_norm));
            }
            if let Some(parent) = to_full.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::rename(&from_full, &to_full)?;
            Ok(())
        })
        .await
        .map_err(|e| NodikleError::StorageIo(e.to_string()))?
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[tokio::test]
    async fn desktop_store_atomic_write_optimistic_lock_and_delete() {
        let tmp = TempDir::new().expect("tempdir");
        let store = DesktopFsStore::open(tmp.path()).expect("open");

        let h1 = store
            .write_atomic("notes/hello.md", b"# Hello\n", None)
            .await
            .expect("write");
        let entries = store.list_entries("").await.expect("list");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].rel_path, "notes/hello.md");

        // Wrong expected hash must trigger ErrOptimisticLock.
        let err = store
            .write_atomic("notes/hello.md", b"# Changed\n", Some("wrong_hash"))
            .await
            .expect_err("must reject stale hash");
        assert_eq!(err.code(), nodikle_model::ErrorCode::ErrOptimisticLock);

        // Matching expected hash succeeds.
        let h2 = store
            .write_atomic("notes/hello.md", b"# Changed\n", Some(&h1))
            .await
            .expect("write with matching hash");
        assert_ne!(h1, h2);

        store
            .rename_entry("notes/hello.md", "notes/moved.md")
            .await
            .expect("rename");
        assert!(store.stat("notes/hello.md").await.expect("stat").is_none());
        assert!(store.stat("notes/moved.md").await.expect("stat").is_some());

        store.delete_entry("notes/moved.md").await.expect("delete");
        assert!(store.list_entries("").await.expect("list").is_empty());
    }
}
