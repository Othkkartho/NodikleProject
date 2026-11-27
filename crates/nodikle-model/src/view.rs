//! Database view schemas, filter AST, query results, and vault stats.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::property::{PropertySchema, PropertyValue};

/// Supported database view layout kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewType {
    Table,
    Board,
    Calendar,
}

/// Sort direction for database view queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortDir {
    Asc,
    Desc,
}

/// Single sort clause on a property or document field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SortSpec {
    pub property: String,
    pub dir: SortDir,
}

/// Filter comparison operators supported by the SQL query builder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterOp {
    Eq,
    Neq,
    Gt,
    Gte,
    Lt,
    Lte,
    Contains,
    StartsWith,
    In,
    Between,
    IsEmpty,
    IsNotEmpty,
}

/// Recursive filter AST used in view definitions and runtime UI filters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FilterExpr {
    And {
        and: Vec<FilterExpr>,
    },
    Or {
        or: Vec<FilterExpr>,
    },
    Not {
        not: Box<FilterExpr>,
    },
    Condition {
        op: FilterOp,
        property: String,
        #[serde(default)]
        value: Option<serde_json::Value>,
        #[serde(default)]
        values: Option<Vec<serde_json::Value>>,
    },
}

/// Supported aggregation functions for table footers and calendar badges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AggregationOp {
    Count,
    Sum,
    Avg,
    Min,
    Max,
    CheckedRatio,
}

/// Calendar badge aggregation configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BadgeAggregation {
    pub property: String,
    pub op: AggregationOp,
}

/// Individual layout definition inside a collection view schema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewLayout {
    pub id: String,
    #[serde(rename = "type")]
    pub view_type: ViewType,
    pub name: String,
    #[serde(default)]
    pub group_by: Option<String>,
    #[serde(default)]
    pub sort: Vec<SortSpec>,
    #[serde(default)]
    pub columns: Vec<String>,
    #[serde(default)]
    pub card_fields: Vec<String>,
    #[serde(default)]
    pub aggregations: BTreeMap<String, AggregationOp>,
    #[serde(default)]
    pub date_property: Option<String>,
    #[serde(default)]
    pub end_date_property: Option<String>,
    #[serde(default)]
    pub badge_aggregation: Option<BadgeAggregation>,
}

/// Source selection rules for a database collection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewSource {
    #[serde(default)]
    pub folder: Option<String>,
    #[serde(default)]
    pub filter: Option<FilterExpr>,
}

/// Top-level collection view schema loaded from `.nodikle/views/*.yaml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewSchema {
    pub id: String,
    pub name: String,
    pub source: ViewSource,
    #[serde(default)]
    pub properties: BTreeMap<String, PropertySchema>,
    #[serde(default)]
    pub views: Vec<ViewLayout>,
    #[serde(default)]
    pub default_template: Option<String>,
    #[serde(default)]
    pub filename_pattern: Option<String>,
}

/// Single document row returned by `view_query`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewRow {
    pub doc_id: String,
    pub rel_path: String,
    pub title: String,
    pub updated_at: String,
    pub properties: BTreeMap<String, PropertyValue>,
    pub task_completed: u32,
    pub task_total: u32,
}

/// Grouped aggregation summary returned alongside rows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewGroupSummary {
    pub group_key: String,
    pub count: u64,
    pub aggregations: BTreeMap<String, f64>,
}

/// Query result payload for database views.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewQueryResult {
    pub view_id: String,
    pub layout_id: String,
    pub rows: Vec<ViewRow>,
    pub groups: Vec<ViewGroupSummary>,
    pub aggregations: BTreeMap<String, f64>,
    pub total_count: u64,
}

/// Full-text search hit with highlighted snippet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchHit {
    pub doc_id: String,
    pub rel_path: String,
    pub title: String,
    pub doc_type: Option<String>,
    pub snippet: String,
}

/// Summary tag entry with document count.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagCount {
    pub tag: String,
    pub count: u64,
}

/// Vault-level statistics returned after opening or reindexing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultStats {
    pub vault_path: String,
    pub doc_count: u64,
    pub tag_count: u64,
    pub link_count: u64,
    pub task_count: u64,
}

/// Incremental reindex execution summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReindexReport {
    pub scanned: u64,
    pub updated: u64,
    pub skipped: u64,
    pub removed: u64,
    pub elapsed_ms: u64,
}

/// Available document template metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateInfo {
    pub rel_path: String,
    pub name: String,
}
