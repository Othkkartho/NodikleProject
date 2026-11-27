//! Path normalization, traversal guard, and ignored-path rules.

use std::path::{Component, Path, PathBuf};

use nodikle_model::{NodikleError, Result};

/// Normalizes a Vault-relative path to `/` separators and rejects path traversal attempts.
pub fn normalize_rel_path(raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(NodikleError::InvalidPath("empty path".into()));
    }
    if trimmed.contains('\0') {
        return Err(NodikleError::InvalidPath("null byte in path".into()));
    }
    if trimmed.starts_with('/') || trimmed.starts_with('\\') {
        return Err(NodikleError::InvalidPath(format!(
            "absolute path not allowed: {raw}"
        )));
    }
    // Reject Windows drive prefix like `C:...`
    let bytes = trimmed.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return Err(NodikleError::InvalidPath(format!(
            "drive-prefixed path not allowed: {raw}"
        )));
    }

    let unified = trimmed.replace('\\', "/");
    let mut parts: Vec<&str> = Vec::new();
    for seg in unified.split('/') {
        match seg {
            "" | "." => continue,
            ".." => {
                return Err(NodikleError::InvalidPath(format!(
                    "parent traversal '..' not allowed: {raw}"
                )));
            }
            valid => parts.push(valid),
        }
    }

    if parts.is_empty() {
        return Err(NodikleError::InvalidPath(format!(
            "path resolves to vault root: {raw}"
        )));
    }

    Ok(parts.join("/"))
}

/// Normalizes an optional listing prefix (empty string means Vault root).
pub fn normalize_prefix(raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    normalize_rel_path(trimmed)
}

/// Resolves a validated relative path under `root`, rejecting any `..` or root components.
pub fn resolve_under_root(root: &Path, rel_path: &str) -> Result<PathBuf> {
    let norm = normalize_rel_path(rel_path)?;
    let mut full = root.to_path_buf();
    for seg in norm.split('/') {
        let comp = Path::new(seg).components().next();
        match comp {
            Some(Component::Normal(_)) => full.push(seg),
            _ => {
                return Err(NodikleError::InvalidPath(format!(
                    "invalid path segment '{seg}' in {rel_path}"
                )));
            }
        }
    }
    Ok(full)
}

/// Returns true if a Vault-relative path must be ignored by indexing and sync.
pub fn is_ignored_path(rel_path: &str) -> bool {
    let norm = rel_path.trim_start_matches('/');
    if norm.starts_with(".git/") || norm == ".git" {
        return true;
    }
    if norm.starts_with(".nodikle/index.db") {
        return true;
    }
    if norm.contains(".nodikle-tmp-") || norm.ends_with(".tmp") {
        return true;
    }
    let file_name = norm.rsplit('/').next().unwrap_or(norm);
    matches!(file_name, ".DS_Store" | "Thumbs.db" | "desktop.ini")
}

/// Computes lowercase hex BLAKE3 digest of bytes.
pub fn blake3_hex(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_traversal_and_absolute_paths() {
        assert!(normalize_rel_path("../secret.md").is_err());
        assert!(normalize_rel_path("notes/../../etc/passwd").is_err());
        assert!(normalize_rel_path("/abs/path.md").is_err());
        assert!(normalize_rel_path("C:\\vault\\note.md").is_err());
        assert_eq!(
            normalize_rel_path("finance\\2026-11-27-lunch.md").expect("valid"),
            "finance/2026-11-27-lunch.md"
        );
    }

    #[test]
    fn recognizes_ignored_paths() {
        assert!(is_ignored_path(".nodikle/index.db"));
        assert!(is_ignored_path(".nodikle/index.db-wal"));
        assert!(is_ignored_path(".git/HEAD"));
        assert!(is_ignored_path("notes/test.tmp"));
        assert!(!is_ignored_path(".nodikle/views/expenses.yaml"));
        assert!(!is_ignored_path("finance/2026-11-27-lunch.md"));
    }
}
