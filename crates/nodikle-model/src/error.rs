//! Error taxonomy and IPC error mapping for Nodikle.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Machine-readable error codes returned across crate and IPC boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    ErrInvalidPath,
    ErrInvalidFrontmatter,
    ErrOptimisticLock,
    ErrNotFound,
    ErrStorageIo,
    ErrIndexDb,
    ErrInvalidSchema,
    ErrTemplateRender,
}

/// Unified domain error type for Nodikle crates.
#[derive(Debug, Error)]
pub enum NodikleError {
    #[error("invalid vault relative path: {0}")]
    InvalidPath(String),

    #[error("invalid frontmatter YAML: {0}")]
    InvalidFrontmatter(String),

    #[error("optimistic lock mismatch for '{path}': expected {expected}, actual {actual}")]
    OptimisticLock {
        path: String,
        expected: String,
        actual: String,
    },

    #[error("resource not found: {0}")]
    NotFound(String),

    #[error("storage I/O failure: {0}")]
    StorageIo(String),

    #[error("index database failure: {0}")]
    IndexDb(String),

    #[error("invalid view schema: {0}")]
    InvalidSchema(String),

    #[error("template render error: {0}")]
    TemplateRender(String),
}

impl NodikleError {
    /// Returns the stable machine-readable error code for this error.
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::InvalidPath(_) => ErrorCode::ErrInvalidPath,
            Self::InvalidFrontmatter(_) => ErrorCode::ErrInvalidFrontmatter,
            Self::OptimisticLock { .. } => ErrorCode::ErrOptimisticLock,
            Self::NotFound(_) => ErrorCode::ErrNotFound,
            Self::StorageIo(_) => ErrorCode::ErrStorageIo,
            Self::IndexDb(_) => ErrorCode::ErrIndexDb,
            Self::InvalidSchema(_) => ErrorCode::ErrInvalidSchema,
            Self::TemplateRender(_) => ErrorCode::ErrTemplateRender,
        }
    }
}

/// Serializable IPC error payload exposed to the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IpcError {
    pub code: ErrorCode,
    pub message: String,
}

impl From<NodikleError> for IpcError {
    fn from(err: NodikleError) -> Self {
        Self {
            code: err.code(),
            message: err.to_string(),
        }
    }
}

impl From<std::io::Error> for NodikleError {
    fn from(err: std::io::Error) -> Self {
        Self::StorageIo(err.to_string())
    }
}

/// Convenience result alias for Nodikle domain operations.
pub type Result<T> = std::result::Result<T, NodikleError>;
