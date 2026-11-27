//! Platform-independent storage and secret store trait definitions.

use async_trait::async_trait;
use nodikle_model::Result;

/// Metadata for a file or directory inside a Vault.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalFileMeta {
    pub rel_path: String,
    pub size_bytes: u64,
    pub mtime_ms: i64,
    pub is_dir: bool,
}

/// Abstract local file store implemented per platform (Desktop fs, Android SAF, in-memory).
#[async_trait]
pub trait LocalFileStore: Send + Sync {
    /// Lists all non-ignored file entries under `prefix` (empty string lists entire Vault).
    async fn list_entries(&self, prefix: &str) -> Result<Vec<LocalFileMeta>>;

    /// Returns metadata for `rel_path`, or `None` if it does not exist.
    async fn stat(&self, rel_path: &str) -> Result<Option<LocalFileMeta>>;

    /// Reads the raw bytes of `rel_path`.
    async fn read_bytes(&self, rel_path: &str) -> Result<Vec<u8>>;

    /// Atomically writes `bytes` to `rel_path` after optional optimistic hash check, returning new BLAKE3 hex.
    async fn write_atomic(
        &self,
        rel_path: &str,
        bytes: &[u8],
        expected_hash: Option<&str>,
    ) -> Result<String>;

    /// Deletes a file at `rel_path`.
    async fn delete_entry(&self, rel_path: &str) -> Result<()>;

    /// Renames or moves a file inside the Vault.
    async fn rename_entry(&self, from: &str, to: &str) -> Result<()>;
}

/// Abstract secure credential store (OS Keyring / Android Keystore / in-memory test mock).
#[async_trait]
pub trait SecretStore: Send + Sync {
    async fn get_secret(&self, key: &str) -> Result<Option<String>>;
    async fn set_secret(&self, key: &str, value: &str) -> Result<()>;
    async fn delete_secret(&self, key: &str) -> Result<()>;
}
