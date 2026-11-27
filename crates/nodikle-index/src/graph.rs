//! Document listing, nested tag queries, and backlink graph queries.

use nodikle_core::normalize_tag;
use nodikle_model::{BacklinkEntry, DocumentMeta, NodikleError, Result, TagCount};
use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, OptionalExtension};

use crate::fts::escape_like;
use crate::scanner::SqliteIndex;

impl SqliteIndex {
    /// Lists indexed documents, optionally filtered by folder or nested tag prefix.
    pub fn list_documents(
        &self,
        folder: Option<&str>,
        tag_prefix: Option<&str>,
    ) -> Result<Vec<DocumentMeta>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| NodikleError::IndexDb(e.to_string()))?;

        let mut sql = String::from(
            "SELECT id, rel_path, stem, folder, title, doc_type, content_hash, mtime_ms, size_bytes, created_at, updated_at
             FROM documents d WHERE 1=1",
        );
        let mut params_vec: Vec<Value> = Vec::new();

        if let Some(dir) = folder.map(str::trim).filter(|s| !s.is_empty()) {
            let p1 = params_vec.len() + 1;
            let p2 = params_vec.len() + 2;
            sql.push_str(&format!(
                " AND (d.folder = ?{p1} OR d.folder LIKE ?{p2} ESCAPE '\\')"
            ));
            let like_dir = format!("{}/%", escape_like(dir));
            params_vec.push(Value::Text(dir.to_string()));
            params_vec.push(Value::Text(like_dir));
        }

        if let Some(tag) = tag_prefix.and_then(normalize_tag) {
            let p1 = params_vec.len() + 1;
            let p2 = params_vec.len() + 2;
            sql.push_str(&format!(
                " AND EXISTS (SELECT 1 FROM tags t WHERE t.doc_id = d.id AND (t.tag = ?{p1} OR t.tag LIKE ?{p2} ESCAPE '\\'))"
            ));
            let like_tag = format!("{}/%", escape_like(&tag));
            params_vec.push(Value::Text(tag));
            params_vec.push(Value::Text(like_tag));
        }

        sql.push_str(" ORDER BY d.updated_at DESC, d.rel_path ASC");

        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
        let rows = stmt
            .query_map(params_from_iter(params_vec.iter()), map_doc_meta)
            .map_err(|e| NodikleError::IndexDb(e.to_string()))?;

        let mut docs = Vec::new();
        for row in rows {
            docs.push(row.map_err(|e| NodikleError::IndexDb(e.to_string()))?);
        }
        Ok(docs)
    }

    /// Retrieves indexed metadata for a document by its Vault-relative path.
    pub fn get_document_by_path(&self, rel_path: &str) -> Result<Option<DocumentMeta>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
        conn.query_row(
            "SELECT id, rel_path, stem, folder, title, doc_type, content_hash, mtime_ms, size_bytes, created_at, updated_at
             FROM documents WHERE rel_path = ?1",
            params![rel_path],
            map_doc_meta,
        )
        .optional()
        .map_err(|e| NodikleError::IndexDb(e.to_string()))
    }

    /// Returns all incoming backlinks targeting a document ID (or matching its stem/path).
    pub fn get_backlinks(&self, doc_id: &str) -> Result<Vec<BacklinkEntry>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT s.id, s.rel_path, s.title, l.raw_target, l.anchor, l.alias
                 FROM links l
                 JOIN documents s ON s.id = l.source_doc_id
                 WHERE l.target_doc_id = ?1 AND l.source_doc_id != ?1
                 ORDER BY s.title ASC",
            )
            .map_err(|e| NodikleError::IndexDb(e.to_string()))?;

        let rows = stmt
            .query_map(params![doc_id], |r| {
                Ok(BacklinkEntry {
                    source_doc_id: r.get(0)?,
                    source_rel_path: r.get(1)?,
                    source_title: r.get(2)?,
                    raw_target: r.get(3)?,
                    anchor: r.get(4)?,
                    alias: r.get(5)?,
                })
            })
            .map_err(|e| NodikleError::IndexDb(e.to_string()))?;

        let mut backlinks = Vec::new();
        for row in rows {
            backlinks.push(row.map_err(|e| NodikleError::IndexDb(e.to_string()))?);
        }
        Ok(backlinks)
    }

    /// Resolves a wikilink target string to a document relative path if indexed.
    pub fn resolve_wikilink_path(&self, raw_target: &str) -> Result<Option<String>> {
        let normalized = raw_target.trim().replace('\\', "/");
        let without_ext = normalized.strip_suffix(".md").unwrap_or(&normalized);
        let conn = self
            .conn
            .lock()
            .map_err(|e| NodikleError::IndexDb(e.to_string()))?;

        if let Some(path) = conn
            .query_row(
                "SELECT rel_path FROM documents WHERE rel_path = ?1 OR rel_path = ?2 LIMIT 1",
                params![normalized, format!("{without_ext}.md")],
                |r| r.get::<_, String>(0),
            )
            .optional()
            .map_err(|e| NodikleError::IndexDb(e.to_string()))?
        {
            return Ok(Some(path));
        }

        conn.query_row(
            "SELECT rel_path FROM documents WHERE lower(stem) = lower(?1) ORDER BY rel_path ASC LIMIT 1",
            params![without_ext],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| NodikleError::IndexDb(e.to_string()))
    }

    /// Lists all tags with their document counts.
    pub fn list_tags(&self) -> Result<Vec<TagCount>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT tag, COUNT(DISTINCT doc_id) AS cnt FROM tags GROUP BY tag ORDER BY tag ASC",
            )
            .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
        let rows = stmt
            .query_map([], |r| {
                Ok(TagCount {
                    tag: r.get(0)?,
                    count: r.get::<_, i64>(1)? as u64,
                })
            })
            .map_err(|e| NodikleError::IndexDb(e.to_string()))?;

        let mut tags = Vec::new();
        for row in rows {
            tags.push(row.map_err(|e| NodikleError::IndexDb(e.to_string()))?);
        }
        Ok(tags)
    }
}

fn map_doc_meta(row: &rusqlite::Row<'_>) -> rusqlite::Result<DocumentMeta> {
    Ok(DocumentMeta {
        id: row.get(0)?,
        rel_path: row.get(1)?,
        stem: row.get(2)?,
        folder: row.get(3)?,
        title: row.get(4)?,
        doc_type: row.get(5)?,
        content_hash: row.get(6)?,
        mtime_ms: row.get(7)?,
        size_bytes: row.get::<_, i64>(8)?.max(0) as u64,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}
