//! Document metadata, wikilink, task, and full-document DTOs.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::property::PropertyValue;

/// Indexed document summary metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentMeta {
    pub id: String,
    pub rel_path: String,
    pub stem: String,
    pub folder: String,
    pub title: String,
    pub doc_type: Option<String>,
    pub content_hash: String,
    pub mtime_ms: i64,
    pub size_bytes: u64,
    pub created_at: String,
    pub updated_at: String,
}

/// Extracted `[[wikilink]]` or `![[embed]]` reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wikilink {
    pub raw_target: String,
    pub anchor: Option<String>,
    pub alias: Option<String>,
    pub is_embed: bool,
}

/// Incoming link summary shown in a document's backlinks panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BacklinkEntry {
    pub source_doc_id: String,
    pub source_rel_path: String,
    pub source_title: String,
    pub raw_target: String,
    pub anchor: Option<String>,
    pub alias: Option<String>,
}

/// Extracted Markdown task checklist item (`- [ ]` / `- [x]`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskItem {
    /// 1-based line number in the original Markdown file.
    pub line_no: usize,
    pub completed: bool,
    pub body: String,
}

/// Result of parsing a raw Markdown document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedDocument {
    pub id: Option<String>,
    pub title: Option<String>,
    pub doc_type: Option<String>,
    pub properties: BTreeMap<String, PropertyValue>,
    pub ordered_keys: Vec<String>,
    pub tags: Vec<String>,
    pub wikilinks: Vec<Wikilink>,
    pub tasks: Vec<TaskItem>,
    pub body_md: String,
    pub body_plain: String,
    pub content_hash: String,
}

/// Full document payload delivered to the editor UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentDetail {
    pub meta: DocumentMeta,
    pub raw_md: String,
    pub body_md: String,
    pub properties: BTreeMap<String, PropertyValue>,
    pub ordered_keys: Vec<String>,
    pub tags: Vec<String>,
    pub wikilinks: Vec<Wikilink>,
    pub backlinks: Vec<BacklinkEntry>,
    pub tasks: Vec<TaskItem>,
}

/// Result of saving or patching a document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveResult {
    pub rel_path: String,
    pub new_hash: String,
    pub updated_at: String,
}
