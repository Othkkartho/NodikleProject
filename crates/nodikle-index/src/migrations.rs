//! SQLite PRAGMA initialization and DDL schema migrations.

use nodikle_model::{NodikleError, Result};
use rusqlite::Connection;

const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS documents (
    id            TEXT PRIMARY KEY,
    rel_path      TEXT NOT NULL UNIQUE,
    stem          TEXT NOT NULL,
    folder        TEXT NOT NULL,
    title         TEXT NOT NULL,
    doc_type      TEXT,
    content_hash  TEXT NOT NULL,
    mtime_ms      INTEGER NOT NULL,
    size_bytes    INTEGER NOT NULL,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_documents_folder ON documents(folder);
CREATE INDEX IF NOT EXISTS idx_documents_type ON documents(doc_type);
CREATE INDEX IF NOT EXISTS idx_documents_stem ON documents(stem);

CREATE TABLE IF NOT EXISTS properties (
    doc_id        TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    key           TEXT NOT NULL,
    ordinal       INTEGER NOT NULL DEFAULT 0,
    value_type    TEXT NOT NULL,
    val_text      TEXT,
    val_num       REAL,
    val_date      TEXT,
    val_bool      INTEGER,
    PRIMARY KEY (doc_id, key, ordinal)
);

CREATE INDEX IF NOT EXISTS idx_props_key_text ON properties(key, val_text);
CREATE INDEX IF NOT EXISTS idx_props_key_num  ON properties(key, val_num);
CREATE INDEX IF NOT EXISTS idx_props_key_date ON properties(key, val_date);

CREATE TABLE IF NOT EXISTS links (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    source_doc_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    target_doc_id TEXT REFERENCES documents(id) ON DELETE SET NULL,
    raw_target    TEXT NOT NULL,
    anchor        TEXT,
    alias         TEXT,
    is_embed      INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_links_source ON links(source_doc_id);
CREATE INDEX IF NOT EXISTS idx_links_target ON links(target_doc_id);
CREATE INDEX IF NOT EXISTS idx_links_raw    ON links(raw_target);

CREATE TABLE IF NOT EXISTS tags (
    doc_id        TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    tag           TEXT NOT NULL,
    PRIMARY KEY (doc_id, tag)
);

CREATE INDEX IF NOT EXISTS idx_tags_tag ON tags(tag);

CREATE TABLE IF NOT EXISTS tasks (
    doc_id        TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    line_no       INTEGER NOT NULL,
    completed     INTEGER NOT NULL,
    body          TEXT NOT NULL,
    PRIMARY KEY (doc_id, line_no)
);

CREATE VIRTUAL TABLE IF NOT EXISTS fts_documents USING fts5(
    doc_id UNINDEXED,
    title,
    body_plain,
    tokenize = 'trigram'
);
"#;

/// Applies recommended SQLite connection PRAGMAs and runs schema migrations.
pub fn initialize_connection(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;
         PRAGMA temp_store = MEMORY;
         PRAGMA busy_timeout = 5000;",
    )
    .map_err(|e| NodikleError::IndexDb(e.to_string()))?;

    conn.execute_batch(SCHEMA_SQL)
        .map_err(|e| NodikleError::IndexDb(e.to_string()))?;

    Ok(())
}
