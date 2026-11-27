//! Full-text search using SQLite FTS5 (`trigram`) with short-query `LIKE` fallback.

use nodikle_core::normalize_tag;
use nodikle_model::{NodikleError, Result, SearchHit};
use rusqlite::params_from_iter;
use rusqlite::types::Value;

use crate::scanner::SqliteIndex;

impl SqliteIndex {
    /// Performs full-text search across document titles and bodies.
    ///
    /// Queries of 3+ characters use FTS5 `trigram` with snippet highlighting;
    /// 1-2 character queries fall back to parameterized `LIKE` so short Hangul/CJK queries match.
    pub fn search_fts(
        &self,
        query: &str,
        tag_prefix: Option<&str>,
        folder: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SearchHit>> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }
        let max_hits = limit.clamp(1, 200) as i64;
        let conn = self
            .conn
            .lock()
            .map_err(|e| NodikleError::IndexDb(e.to_string()))?;

        let mut where_extra = String::new();
        let mut params_vec: Vec<Value> = Vec::new();

        if trimmed.chars().count() >= 3 {
            let quoted = quote_fts_phrase(trimmed);
            params_vec.push(Value::Text(quoted));

            let mut sql = String::from(
                "SELECT d.id, d.rel_path, d.title, d.doc_type,
                        snippet(fts_documents, 2, '<mark>', '</mark>', '...', 24) AS snip
                 FROM fts_documents f
                 JOIN documents d ON d.id = f.doc_id
                 WHERE fts_documents MATCH ?1",
            );

            append_scope_filters(tag_prefix, folder, &mut where_extra, &mut params_vec);
            sql.push_str(&where_extra);
            let limit_idx = params_vec.len() + 1;
            sql.push_str(&format!(
                " ORDER BY bm25(fts_documents, 2.0, 1.0) LIMIT ?{limit_idx}"
            ));
            params_vec.push(Value::Integer(max_hits));

            let mut stmt = conn
                .prepare(&sql)
                .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
            let rows = stmt
                .query_map(params_from_iter(params_vec.iter()), |r| {
                    Ok(SearchHit {
                        doc_id: r.get(0)?,
                        rel_path: r.get(1)?,
                        title: r.get(2)?,
                        doc_type: r.get(3)?,
                        snippet: r.get::<_, Option<String>>(4)?.unwrap_or_default(),
                    })
                })
                .map_err(|e| NodikleError::IndexDb(e.to_string()))?;

            let mut hits = Vec::new();
            for row in rows {
                hits.push(row.map_err(|e| NodikleError::IndexDb(e.to_string()))?);
            }
            return Ok(hits);
        }

        // Short-query fallback (< 3 chars).
        let like_pat = format!("%{}%", escape_like(trimmed));
        params_vec.push(Value::Text(like_pat));
        let mut sql = String::from(
            "SELECT d.id, d.rel_path, d.title, d.doc_type,
                    substr(f.body_plain, 1, 140) AS snip
             FROM documents d
             JOIN fts_documents f ON f.doc_id = d.id
             WHERE (d.title LIKE ?1 ESCAPE '\\' OR f.body_plain LIKE ?1 ESCAPE '\\')",
        );
        append_scope_filters(tag_prefix, folder, &mut where_extra, &mut params_vec);
        sql.push_str(&where_extra);
        let limit_idx = params_vec.len() + 1;
        sql.push_str(&format!(" ORDER BY d.updated_at DESC LIMIT ?{limit_idx}"));
        params_vec.push(Value::Integer(max_hits));

        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
        let rows = stmt
            .query_map(params_from_iter(params_vec.iter()), |r| {
                Ok(SearchHit {
                    doc_id: r.get(0)?,
                    rel_path: r.get(1)?,
                    title: r.get(2)?,
                    doc_type: r.get(3)?,
                    snippet: r.get::<_, Option<String>>(4)?.unwrap_or_default(),
                })
            })
            .map_err(|e| NodikleError::IndexDb(e.to_string()))?;

        let mut hits = Vec::new();
        for row in rows {
            hits.push(row.map_err(|e| NodikleError::IndexDb(e.to_string()))?);
        }
        Ok(hits)
    }
}

fn append_scope_filters(
    tag_prefix: Option<&str>,
    folder: Option<&str>,
    sql: &mut String,
    params_vec: &mut Vec<Value>,
) {
    if let Some(prefix) = tag_prefix.and_then(normalize_tag) {
        let p1 = params_vec.len() + 1;
        let p2 = params_vec.len() + 2;
        sql.push_str(&format!(
            " AND EXISTS (SELECT 1 FROM tags t WHERE t.doc_id = d.id AND (t.tag = ?{p1} OR t.tag LIKE ?{p2} ESCAPE '\\'))"
        ));
        let like_prefix = format!("{}/%", escape_like(&prefix));
        params_vec.push(Value::Text(prefix));
        params_vec.push(Value::Text(like_prefix));
    }

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
}

fn quote_fts_phrase(q: &str) -> String {
    let escaped = q.replace('"', "\"\"");
    format!("\"{escaped}\"")
}

pub(crate) fn escape_like(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}
