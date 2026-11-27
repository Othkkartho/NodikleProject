//! Storage abstractions, path validation, and local file store implementations.

pub mod atomic;
pub mod fs_desktop;
pub mod memory;
pub mod traits;

pub use atomic::{blake3_hex, is_ignored_path, normalize_prefix, normalize_rel_path};
pub use fs_desktop::DesktopFsStore;
pub use memory::{InMemorySecretStore, MemoryFileStore};
pub use traits::{LocalFileMeta, LocalFileStore, SecretStore};
