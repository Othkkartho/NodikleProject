//! SQLite index handle and incremental Vault scanner.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use chrono::{DateTime, Utc};
use nodikle_core::{hash_bytes, parse_markdown_document};
use nodikle_model::{
    DocumentMeta, NodikleError, ParsedDocument, PropertyValue, ReindexReport, Result, VaultStats,
};
use nodikle_storage::{LocalFileMeta, LocalFileStore};
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::migrations::initialize_connection;

/// Thread-safe local SQLite + FTS5 index store.
#[derive(Clone)]
pub struct SqliteIndex {
    pub(crate) conn: Arc<Mutex<Connection>>,
    pub(crate) vault_path: String,
}

impl SqliteIndex {
    /// Opens (or creates) a persistent SQLite index at `db_path`.
    pub fn open(db_path: impl AsRef<Path>, vault_path: impl Into<String>) -> Result<Self> {
        if let Some(parent) = db_path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path).map_err(|e| NodikleError::IndexDb(e.to_string()))?;
        initialize_connection(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            vault_path: vault_path.into(),
        })
    }

    /// Opens an isolated in-memory SQLite index for unit tests.
    pub fn open_in_memory(vault_path: impl Into<String>) -> Result<Self> {
        let conn =
            Connection::open_in_memory().map_err(|e| NodikleError::IndexDb(e.to_string()))?;
        initialize_connection(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            vault_path: vault_path.into(),
        })
    }

    /// Incrementally scans all `.md` documents in `store` and updates the SQLite index.
    pub async fn reindex_vault(
        &self,
        store: &dyn LocalFileStore,
        full_rebuild: bool,
    ) -> Result<ReindexReport> {
        let started = Instant::now();
        let entries = store.list_entries("").await?;
        let md_entries: Vec<LocalFileMeta> = entries
            .into_iter()
            .filter(|e| !e.is_dir && is_indexable_markdown(&e.rel_path))
            .collect();

        let existing = self.load_existing_stat_map()?;
        let mut to_upsert: Vec<(LocalFileMeta, String, ParsedDocument)> = Vec::new();
        let mut touch_mtime: Vec<(String, i64)> = Vec::new();
        let mut skipped = 0u64;
        let mut disk_paths = HashSet::with_capacity(md_entries.len());

        for entry in &md_entries {
            disk_paths.insert(entry.rel_path.clone());
            if !full_rebuild {
                if let Some((prev_hash, prev_mtime, prev_size)) = existing.get(&entry.rel_path) {
                    if *prev_mtime == entry.mtime_ms && *prev_size == entry.size_bytes {
                        skipped += 1;
                        continue;
                    }
                    let bytes = store.read_bytes(&entry.rel_path).await?;
                    let hash = hash_bytes(&bytes);
                    if &hash == prev_hash {
                        touch_mtime.push((entry.rel_path.clone(), entry.mtime_ms));
                        skipped += 1;
                        continue;
                    }
                    let raw = String::from_utf8_lossy(&bytes).into_owned();
                    let parsed = parse_markdown_document(&raw)?;
                    to_upsert.push((entry.clone(), hash, parsed));
                    continue;
                }
            }

            let bytes = store.read_bytes(&entry.rel_path).await?;
            let hash = hash_bytes(&bytes);
            let raw = String::from_utf8_lossy(&bytes).into_owned();
            let parsed = parse_markdown_document(&raw)?;
            to_upsert.push((entry.clone(), hash, parsed));
        }

        let removed_paths: Vec<String> = existing
            .keys()
            .filter(|p| full_rebuild || !disk_paths.contains(*p))
            .cloned()
            .collect();

        let updated = to_upsert.len() as u64;
        let removed = if full_rebuild {
            0
        } else {
            removed_paths.len() as u64
        };
        let scanned = md_entries.len() as u64;

        let conn_arc = self.conn.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let mut conn = conn_arc
                .lock()
                .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
            let tx = conn
                .transaction()
                .map_err(|e| NodikleError::IndexDb(e.to_string()))?;

            if full_rebuild {
                tx.execute("DELETE FROM fts_documents", [])
                    .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
                tx.execute("DELETE FROM documents", [])
                    .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
            } else {
                for path in &removed_paths {
                    delete_doc_by_path_tx(&tx, path)?;
                }
                for (path, mtime_ms) in &touch_mtime {
                    tx.execute(
                        "UPDATE documents SET mtime_ms = ?1 WHERE rel_path = ?2",
                        params![mtime_ms, path],
                    )
                    .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
                }
            }

            for (meta, hash, parsed) in &to_upsert {
                upsert_parsed_doc_tx(&tx, meta, hash, parsed)?;
            }

            resolve_all_links_tx(&tx)?;
            tx.commit()
                .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
            Ok(())
        })
        .await
        .map_err(|e| NodikleError::IndexDb(e.to_string()))??;

        Ok(ReindexReport {
            scanned,
            updated,
            skipped,
            removed,
            elapsed_ms: started.elapsed().as_millis() as u64,
        })
    }

    /// Indexes or refreshes a single Markdown file immediately after save or watcher event.
    pub fn index_single_file(&self, meta: &LocalFileMeta, raw_md: &str) -> Result<DocumentMeta> {
        if !is_indexable_markdown(&meta.rel_path) {
            return Err(NodikleError::InvalidPath(format!(
                "{} is not an indexable markdown document",
                meta.rel_path
            )));
        }
        let parsed = parse_markdown_document(raw_md)?;
        let hash = parsed.content_hash.clone();
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
        let tx = conn
            .transaction()
            .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
        let doc_meta = upsert_parsed_doc_tx(&tx, meta, &hash, &parsed)?;
        resolve_all_links_tx(&tx)?;
        tx.commit()
            .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
        Ok(doc_meta)
    }

    /// Removes a deleted Markdown document from the index.
    pub fn remove_document(&self, rel_path: &str) -> Result<()> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
        let tx = conn
            .transaction()
            .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
        delete_doc_by_path_tx(&tx, rel_path)?;
        resolve_all_links_tx(&tx)?;
        tx.commit()
            .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
        Ok(())
    }

    /// Returns aggregate vault statistics.
    pub fn stats(&self) -> Result<VaultStats> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
        let doc_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM documents", [], |r| r.get(0))
            .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
        let tag_count: i64 = conn
            .query_row("SELECT COUNT(DISTINCT tag) FROM tags", [], |r| r.get(0))
            .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
        let link_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM links", [], |r| r.get(0))
            .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
        let task_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0))
            .map_err(|e| NodikleError::IndexDb(e.to_string()))?;

        Ok(VaultStats {
            vault_path: self.vault_path.clone(),
            doc_count: doc_count as u64,
            tag_count: tag_count as u64,
            link_count: link_count as u64,
            task_count: task_count as u64,
        })
    }

    fn load_existing_stat_map(&self) -> Result<HashMap<String, (String, i64, u64)>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT rel_path, content_hash, mtime_ms, size_bytes FROM documents")
            .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| {
                let rel_path: String = row.get(0)?;
                let hash: String = row.get(1)?;
                let mtime_ms: i64 = row.get(2)?;
                let size_bytes: i64 = row.get(3)?;
                Ok((rel_path, (hash, mtime_ms, size_bytes.max(0) as u64)))
            })
            .map_err(|e| NodikleError::IndexDb(e.to_string()))?;

        let mut map = HashMap::new();
        for item in rows {
            let (k, v) = item.map_err(|e| NodikleError::IndexDb(e.to_string()))?;
            map.insert(k, v);
        }
        Ok(map)
    }
}

pub(crate) fn is_indexable_markdown(rel_path: &str) -> bool {
    rel_path.ends_with(".md")
        && !rel_path.starts_with(".nodikle/")
        && !rel_path.starts_with("_templates/")
}

fn delete_doc_by_path_tx(tx: &rusqlite::Transaction<'_>, rel_path: &str) -> Result<()> {
    let existing_id: Option<String> = tx
        .query_row(
            "SELECT id FROM documents WHERE rel_path = ?1",
            params![rel_path],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| NodikleError::IndexDb(e.to_string()))?;

    if let Some(doc_id) = existing_id {
        tx.execute(
            "DELETE FROM fts_documents WHERE doc_id = ?1",
            params![doc_id],
        )
        .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
        tx.execute("DELETE FROM documents WHERE id = ?1", params![doc_id])
            .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
    }
    Ok(())
}

fn upsert_parsed_doc_tx(
    tx: &rusqlite::Transaction<'_>,
    meta: &LocalFileMeta,
    hash: &str,
    parsed: &ParsedDocument,
) -> Result<DocumentMeta> {
    let existing: Option<(String, String)> = tx
        .query_row(
            "SELECT id, created_at FROM documents WHERE rel_path = ?1",
            params![meta.rel_path],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| NodikleError::IndexDb(e.to_string()))?;

    let stem = Path::new(&meta.rel_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(&meta.rel_path)
        .to_string();
    let folder = match meta.rel_path.rsplit_once('/') {
        Some((dir, _)) => dir.to_string(),
        None => String::new(),
    };
    let title = parsed.title.clone().unwrap_or_else(|| stem.clone());
    let updated_iso = parsed
        .properties
        .get("updated_at")
        .and_then(PropertyValue::as_text)
        .unwrap_or_else(|| format_epoch_ms(meta.mtime_ms));
    let created_iso = parsed
        .properties
        .get("created_at")
        .and_then(PropertyValue::as_text)
        .or_else(|| existing.as_ref().map(|(_, c)| c.clone()))
        .unwrap_or_else(|| updated_iso.clone());

    let doc_id = parsed
        .id
        .clone()
        .or_else(|| existing.as_ref().map(|(id, _)| id.clone()))
        .unwrap_or_else(|| Uuid::now_v7().to_string());

    // Remove previous document row if id changed for same rel_path, or duplicate id exists.
    if let Some((old_id, _)) = &existing {
        if old_id != &doc_id {
            tx.execute(
                "DELETE FROM fts_documents WHERE doc_id = ?1",
                params![old_id],
            )
            .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
            tx.execute("DELETE FROM documents WHERE id = ?1", params![old_id])
                .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
        }
    }

    tx.execute(
        "INSERT INTO documents (id, rel_path, stem, folder, title, doc_type, content_hash, mtime_ms, size_bytes, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT(id) DO UPDATE SET
            rel_path = excluded.rel_path,
            stem = excluded.stem,
            folder = excluded.folder,
            title = excluded.title,
            doc_type = excluded.doc_type,
            content_hash = excluded.content_hash,
            mtime_ms = excluded.mtime_ms,
            size_bytes = excluded.size_bytes,
            created_at = excluded.created_at,
            updated_at = excluded.updated_at",
        params![
            doc_id,
            meta.rel_path,
            stem,
            folder,
            title,
            parsed.doc_type,
            hash,
            meta.mtime_ms,
            meta.size_bytes as i64,
            created_iso,
            updated_iso,
        ],
    )
    .map_err(|e| NodikleError::IndexDb(e.to_string()))?;

    // Replace properties, links, tags, tasks, and FTS entry.
    tx.execute("DELETE FROM properties WHERE doc_id = ?1", params![doc_id])
        .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
    for (key, val) in &parsed.properties {
        insert_property_rows_tx(tx, &doc_id, key, val)?;
    }

    tx.execute(
        "DELETE FROM links WHERE source_doc_id = ?1",
        params![doc_id],
    )
    .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
    for link in &parsed.wikilinks {
        tx.execute(
            "INSERT INTO links (source_doc_id, target_doc_id, raw_target, anchor, alias, is_embed)
             VALUES (?1, NULL, ?2, ?3, ?4, ?5)",
            params![
                doc_id,
                link.raw_target,
                link.anchor,
                link.alias,
                i32::from(link.is_embed),
            ],
        )
        .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
    }

    tx.execute("DELETE FROM tags WHERE doc_id = ?1", params![doc_id])
        .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
    for tag in &parsed.tags {
        tx.execute(
            "INSERT OR IGNORE INTO tags (doc_id, tag) VALUES (?1, ?2)",
            params![doc_id, tag],
        )
        .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
    }

    tx.execute("DELETE FROM tasks WHERE doc_id = ?1", params![doc_id])
        .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
    for task in &parsed.tasks {
        tx.execute(
            "INSERT INTO tasks (doc_id, line_no, completed, body) VALUES (?1, ?2, ?3, ?4)",
            params![
                doc_id,
                task.line_no as i64,
                i32::from(task.completed),
                task.body
            ],
        )
        .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
    }

    tx.execute(
        "DELETE FROM fts_documents WHERE doc_id = ?1",
        params![doc_id],
    )
    .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
    tx.execute(
        "INSERT INTO fts_documents (doc_id, title, body_plain) VALUES (?1, ?2, ?3)",
        params![doc_id, title, parsed.body_plain],
    )
    .map_err(|e| NodikleError::IndexDb(e.to_string()))?;

    Ok(DocumentMeta {
        id: doc_id,
        rel_path: meta.rel_path.clone(),
        stem,
        folder,
        title,
        doc_type: parsed.doc_type.clone(),
        content_hash: hash.to_string(),
        mtime_ms: meta.mtime_ms,
        size_bytes: meta.size_bytes,
        created_at: created_iso,
        updated_at: updated_iso,
    })
}

fn insert_property_rows_tx(
    tx: &rusqlite::Transaction<'_>,
    doc_id: &str,
    key: &str,
    val: &PropertyValue,
) -> Result<()> {
    match val {
        PropertyValue::Text(s) => {
            tx.execute(
                "INSERT INTO properties (doc_id, key, ordinal, value_type, val_text) VALUES (?1, ?2, 0, 'text', ?3)",
                params![doc_id, key, s],
            )
            .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
        }
        PropertyValue::Number(n) => {
            tx.execute(
                "INSERT INTO properties (doc_id, key, ordinal, value_type, val_num, val_text) VALUES (?1, ?2, 0, 'number', ?3, ?4)",
                params![doc_id, key, n, val.as_text()],
            )
            .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
        }
        PropertyValue::Date(d) => {
            tx.execute(
                "INSERT INTO properties (doc_id, key, ordinal, value_type, val_date, val_text) VALUES (?1, ?2, 0, 'date', ?3, ?3)",
                params![doc_id, key, d],
            )
            .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
        }
        PropertyValue::Bool(b) => {
            tx.execute(
                "INSERT INTO properties (doc_id, key, ordinal, value_type, val_bool, val_text) VALUES (?1, ?2, 0, 'bool', ?3, ?4)",
                params![doc_id, key, i32::from(*b), if *b { "true" } else { "false" }],
            )
            .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
        }
        PropertyValue::List(items) => {
            for (ordinal, item) in items.iter().enumerate() {
                tx.execute(
                    "INSERT INTO properties (doc_id, key, ordinal, value_type, val_text) VALUES (?1, ?2, ?3, 'text', ?4)",
                    params![doc_id, key, ordinal as i64, item],
                )
                .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
            }
        }
        PropertyValue::Null => {}
    }
    Ok(())
}

/// Resolves `links.target_doc_id` by exact path, unique stem, then case-insensitive stem.
fn resolve_all_links_tx(tx: &rusqlite::Transaction<'_>) -> Result<()> {
    let mut docs_by_path: HashMap<String, String> = HashMap::new();
    let mut docs_by_stem: HashMap<String, Vec<String>> = HashMap::new();
    let mut docs_by_lower_stem: HashMap<String, Vec<String>> = HashMap::new();

    {
        let mut stmt = tx
            .prepare("SELECT id, rel_path, stem FROM documents")
            .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
        for row in rows {
            let (id, rel_path, stem) = row.map_err(|e| NodikleError::IndexDb(e.to_string()))?;
            let without_ext = rel_path
                .strip_suffix(".md")
                .unwrap_or(&rel_path)
                .to_string();
            docs_by_path.insert(rel_path, id.clone());
            docs_by_path.insert(without_ext, id.clone());
            docs_by_stem
                .entry(stem.clone())
                .or_default()
                .push(id.clone());
            docs_by_lower_stem
                .entry(stem.to_lowercase())
                .or_default()
                .push(id);
        }
    }

    let mut link_updates: Vec<(i64, Option<String>)> = Vec::new();
    {
        let mut stmt = tx
            .prepare("SELECT id, raw_target, is_embed FROM links")
            .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i32>(2)?,
                ))
            })
            .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
        for row in rows {
            let (link_id, raw_target, is_embed) =
                row.map_err(|e| NodikleError::IndexDb(e.to_string()))?;
            if is_embed != 0 && !raw_target.ends_with(".md") {
                link_updates.push((link_id, None));
                continue;
            }
            let normalized = raw_target.replace('\\', "/");
            let trimmed = normalized.strip_suffix(".md").unwrap_or(&normalized);

            let resolved = docs_by_path
                .get(&normalized)
                .or_else(|| docs_by_path.get(trimmed))
                .cloned()
                .or_else(|| {
                    docs_by_stem.get(trimmed).and_then(|ids| {
                        if ids.len() == 1 {
                            Some(ids[0].clone())
                        } else {
                            None
                        }
                    })
                })
                .or_else(|| {
                    docs_by_lower_stem
                        .get(&trimmed.to_lowercase())
                        .and_then(|ids| {
                            if ids.len() == 1 {
                                Some(ids[0].clone())
                            } else {
                                None
                            }
                        })
                });

            link_updates.push((link_id, resolved));
        }
    }

    for (link_id, target_id) in link_updates {
        tx.execute(
            "UPDATE links SET target_doc_id = ?1 WHERE id = ?2",
            params![target_id, link_id],
        )
        .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
    }

    Ok(())
}

fn format_epoch_ms(ms: i64) -> String {
    DateTime::<Utc>::from_timestamp_millis(ms)
        .unwrap_or_else(Utc::now)
        .to_rfc3339()
}
