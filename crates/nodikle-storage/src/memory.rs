//! In-memory file and secret stores for deterministic unit tests.

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use nodikle_model::{NodikleError, Result};

use crate::atomic::{blake3_hex, is_ignored_path, normalize_prefix, normalize_rel_path};
use crate::traits::{LocalFileMeta, LocalFileStore, SecretStore};

#[derive(Debug, Clone)]
struct StoredFile {
    bytes: Vec<u8>,
    mtime_ms: i64,
}

/// Thread-safe in-memory [`LocalFileStore`] implementation.
#[derive(Debug, Default, Clone)]
pub struct MemoryFileStore {
    files: Arc<RwLock<BTreeMap<String, StoredFile>>>,
}

impl MemoryFileStore {
    pub fn new() -> Self {
        Self::default()
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[async_trait]
impl LocalFileStore for MemoryFileStore {
    async fn list_entries(&self, prefix: &str) -> Result<Vec<LocalFileMeta>> {
        let norm_prefix = normalize_prefix(prefix)?;
        let guard = self
            .files
            .read()
            .map_err(|e| NodikleError::StorageIo(e.to_string()))?;
        let entries = guard
            .iter()
            .filter(|(path, _)| {
                !is_ignored_path(path) && (norm_prefix.is_empty() || path.starts_with(&norm_prefix))
            })
            .map(|(path, file)| LocalFileMeta {
                rel_path: path.clone(),
                size_bytes: file.bytes.len() as u64,
                mtime_ms: file.mtime_ms,
                is_dir: false,
            })
            .collect();
        Ok(entries)
    }

    async fn stat(&self, rel_path: &str) -> Result<Option<LocalFileMeta>> {
        let norm = normalize_rel_path(rel_path)?;
        let guard = self
            .files
            .read()
            .map_err(|e| NodikleError::StorageIo(e.to_string()))?;
        Ok(guard.get(&norm).map(|file| LocalFileMeta {
            rel_path: norm,
            size_bytes: file.bytes.len() as u64,
            mtime_ms: file.mtime_ms,
            is_dir: false,
        }))
    }

    async fn read_bytes(&self, rel_path: &str) -> Result<Vec<u8>> {
        let norm = normalize_rel_path(rel_path)?;
        let guard = self
            .files
            .read()
            .map_err(|e| NodikleError::StorageIo(e.to_string()))?;
        guard
            .get(&norm)
            .map(|f| f.bytes.clone())
            .ok_or(NodikleError::NotFound(norm))
    }

    async fn write_atomic(
        &self,
        rel_path: &str,
        bytes: &[u8],
        expected_hash: Option<&str>,
    ) -> Result<String> {
        let norm = normalize_rel_path(rel_path)?;
        let mut guard = self
            .files
            .write()
            .map_err(|e| NodikleError::StorageIo(e.to_string()))?;

        if let Some(exp) = expected_hash {
            if let Some(existing) = guard.get(&norm) {
                let actual = blake3_hex(&existing.bytes);
                if actual != exp {
                    return Err(NodikleError::OptimisticLock {
                        path: norm,
                        expected: exp.to_string(),
                        actual,
                    });
                }
            }
        }

        let new_hash = blake3_hex(bytes);
        guard.insert(
            norm,
            StoredFile {
                bytes: bytes.to_vec(),
                mtime_ms: now_ms(),
            },
        );
        Ok(new_hash)
    }

    async fn delete_entry(&self, rel_path: &str) -> Result<()> {
        let norm = normalize_rel_path(rel_path)?;
        let mut guard = self
            .files
            .write()
            .map_err(|e| NodikleError::StorageIo(e.to_string()))?;
        guard
            .remove(&norm)
            .map(|_| ())
            .ok_or(NodikleError::NotFound(norm))
    }

    async fn rename_entry(&self, from: &str, to: &str) -> Result<()> {
        let from_norm = normalize_rel_path(from)?;
        let to_norm = normalize_rel_path(to)?;
        let mut guard = self
            .files
            .write()
            .map_err(|e| NodikleError::StorageIo(e.to_string()))?;
        let file = guard
            .remove(&from_norm)
            .ok_or_else(|| NodikleError::NotFound(from_norm.clone()))?;
        guard.insert(to_norm, file);
        Ok(())
    }
}

/// Thread-safe in-memory [`SecretStore`] for testing.
#[derive(Debug, Default, Clone)]
pub struct InMemorySecretStore {
    secrets: Arc<RwLock<BTreeMap<String, String>>>,
}

#[async_trait]
impl SecretStore for InMemorySecretStore {
    async fn get_secret(&self, key: &str) -> Result<Option<String>> {
        let guard = self
            .secrets
            .read()
            .map_err(|e| NodikleError::StorageIo(e.to_string()))?;
        Ok(guard.get(key).cloned())
    }

    async fn set_secret(&self, key: &str, value: &str) -> Result<()> {
        let mut guard = self
            .secrets
            .write()
            .map_err(|e| NodikleError::StorageIo(e.to_string()))?;
        guard.insert(key.to_string(), value.to_string());
        Ok(())
    }

    async fn delete_secret(&self, key: &str) -> Result<()> {
        let mut guard = self
            .secrets
            .write()
            .map_err(|e| NodikleError::StorageIo(e.to_string()))?;
        guard.remove(key);
        Ok(())
    }
}
