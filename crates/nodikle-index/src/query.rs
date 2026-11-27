//! Parameterized SQL query and aggregation engine for database views.

use std::collections::{BTreeMap, HashMap};

use nodikle_model::{
    AggregationOp, FilterExpr, FilterOp, NodikleError, PropertyType, PropertyValue, Result,
    SortDir, SortSpec, ViewGroupSummary, ViewQueryResult, ViewRow, ViewSchema,
};
use rusqlite::params_from_iter;
use rusqlite::types::Value;

use crate::fts::escape_like;
use crate::scanner::SqliteIndex;

impl SqliteIndex {
    /// Executes a database view query with filtering, sorting, grouping, and aggregations.
    pub fn query_view(
        &self,
        schema: &ViewSchema,
        layout_id: Option<&str>,
        runtime_filter: Option<&FilterExpr>,
        page: usize,
        limit: usize,
    ) -> Result<ViewQueryResult> {
        let layout = match layout_id {
            Some(id) => schema
                .views
                .iter()
                .find(|v| v.id == id)
                .ok_or_else(|| NodikleError::InvalidSchema(format!("unknown layout '{id}'")))?,
            None => schema
                .views
                .first()
                .ok_or_else(|| NodikleError::InvalidSchema("view schema has no layouts".into()))?,
        };

        let mut where_parts: Vec<String> = vec!["1=1".to_string()];
        let mut params_vec: Vec<Value> = Vec::new();

        if let Some(folder) = schema
            .source
            .folder
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            let p1 = push_param(&mut params_vec, Value::Text(folder.to_string()));
            let p2 = push_param(
                &mut params_vec,
                Value::Text(format!("{}/%", escape_like(folder))),
            );
            where_parts.push(format!(
                "(d.folder = ?{p1} OR d.folder LIKE ?{p2} ESCAPE '\\')"
            ));
        }

        if let Some(source_filter) = &schema.source.filter {
            let clause = compile_filter_expr(source_filter, schema, &mut params_vec)?;
            where_parts.push(format!("({clause})"));
        }

        if let Some(rt_filter) = runtime_filter {
            let clause = compile_filter_expr(rt_filter, schema, &mut params_vec)?;
            where_parts.push(format!("({clause})"));
        }

        let where_sql = where_parts.join(" AND ");
        let order_sql = build_order_by(&layout.sort, schema, &mut params_vec);

        let conn = self
            .conn
            .lock()
            .map_err(|e| NodikleError::IndexDb(e.to_string()))?;

        // 1. Fetch all matching document metadata in sorted order.
        let doc_sql = format!(
            "SELECT d.id, d.rel_path, d.title, d.updated_at
             FROM documents d
             WHERE {where_sql}
             {order_sql}"
        );

        let mut all_docs: Vec<(String, String, String, String)> = Vec::new();
        {
            let mut stmt = conn
                .prepare(&doc_sql)
                .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
            let rows = stmt
                .query_map(params_from_iter(params_vec.iter()), |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
                })
                .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
            for row in rows {
                all_docs.push(row.map_err(|e| NodikleError::IndexDb(e.to_string()))?);
            }
        }

        let total_count = all_docs.len() as u64;
        if all_docs.is_empty() {
            return Ok(ViewQueryResult {
                view_id: schema.id.clone(),
                layout_id: layout.id.clone(),
                rows: Vec::new(),
                groups: Vec::new(),
                aggregations: BTreeMap::new(),
                total_count: 0,
            });
        }

        // 2. Bulk load properties and task progress for matching documents.
        let doc_ids: Vec<String> = all_docs.iter().map(|d| d.0.clone()).collect();
        let props_by_doc = load_properties_for_docs(&conn, &doc_ids, schema)?;
        let tasks_by_doc = load_task_progress_for_docs(&conn, &doc_ids)?;

        // 3. Compute overall and grouped aggregations across all matching documents.
        let mut all_rows_for_agg: Vec<ViewRow> = Vec::with_capacity(all_docs.len());
        for (doc_id, rel_path, title, updated_at) in &all_docs {
            let props = props_by_doc.get(doc_id).cloned().unwrap_or_default();
            let (task_completed, task_total) = tasks_by_doc.get(doc_id).copied().unwrap_or((0, 0));
            all_rows_for_agg.push(ViewRow {
                doc_id: doc_id.clone(),
                rel_path: rel_path.clone(),
                title: title.clone(),
                updated_at: updated_at.clone(),
                properties: props,
                task_completed,
                task_total,
            });
        }

        let aggregations = compute_aggregations(&all_rows_for_agg, &layout.aggregations);
        let groups = if let Some(group_prop) = &layout.group_by {
            compute_group_summaries(&all_rows_for_agg, group_prop, &layout.aggregations)
        } else {
            Vec::new()
        };

        // 4. Slice requested page.
        let safe_limit = limit.clamp(1, 500);
        let offset = page.saturating_mul(safe_limit);
        let paged_rows = if offset >= all_rows_for_agg.len() {
            Vec::new()
        } else {
            let end = (offset + safe_limit).min(all_rows_for_agg.len());
            all_rows_for_agg[offset..end].to_vec()
        };

        Ok(ViewQueryResult {
            view_id: schema.id.clone(),
            layout_id: layout.id.clone(),
            rows: paged_rows,
            groups,
            aggregations,
            total_count,
        })
    }
}

fn push_param(params_vec: &mut Vec<Value>, val: Value) -> usize {
    params_vec.push(val);
    params_vec.len()
}

fn compile_filter_expr(
    expr: &FilterExpr,
    schema: &ViewSchema,
    params_vec: &mut Vec<Value>,
) -> Result<String> {
    match expr {
        FilterExpr::And { and } => {
            if and.is_empty() {
                return Ok("1=1".to_string());
            }
            let mut clauses = Vec::with_capacity(and.len());
            for sub in and {
                clauses.push(format!(
                    "({})",
                    compile_filter_expr(sub, schema, params_vec)?
                ));
            }
            Ok(clauses.join(" AND "))
        }
        FilterExpr::Or { or } => {
            if or.is_empty() {
                return Ok("1=0".to_string());
            }
            let mut clauses = Vec::with_capacity(or.len());
            for sub in or {
                clauses.push(format!(
                    "({})",
                    compile_filter_expr(sub, schema, params_vec)?
                ));
            }
            Ok(clauses.join(" OR "))
        }
        FilterExpr::Not { not } => {
            let inner = compile_filter_expr(not, schema, params_vec)?;
            Ok(format!("NOT ({inner})"))
        }
        FilterExpr::Condition {
            op,
            property,
            value,
            values,
        } => compile_condition(
            *op,
            property,
            value.as_ref(),
            values.as_deref(),
            schema,
            params_vec,
        ),
    }
}

fn compile_condition(
    op: FilterOp,
    property: &str,
    value: Option<&serde_json::Value>,
    values: Option<&[serde_json::Value]>,
    schema: &ViewSchema,
    params_vec: &mut Vec<Value>,
) -> Result<String> {
    // Allow direct document-column filters for `title`, `folder`, `type`, `rel_path`.
    if let Some(doc_col) = document_column_for_key(property) {
        return compile_doc_column_condition(op, doc_col, value, values, params_vec);
    }

    let key_idx = push_param(params_vec, Value::Text(property.to_string()));
    let prop_type = schema.properties.get(property).map(|s| s.prop_type);

    let is_numeric = matches!(
        prop_type,
        Some(PropertyType::Number | PropertyType::Currency)
    ) || value.map(|v| v.is_number()).unwrap_or(false);

    match op {
        FilterOp::IsEmpty => Ok(format!(
            "NOT EXISTS (SELECT 1 FROM properties p WHERE p.doc_id = d.id AND p.key = ?{key_idx} AND COALESCE(p.val_text, '') != '')"
        )),
        FilterOp::IsNotEmpty => Ok(format!(
            "EXISTS (SELECT 1 FROM properties p WHERE p.doc_id = d.id AND p.key = ?{key_idx} AND COALESCE(p.val_text, '') != '')"
        )),
        FilterOp::Eq | FilterOp::Neq => {
            let val = value.ok_or_else(|| {
                NodikleError::InvalidSchema(format!("filter '{property}' requires value"))
            })?;
            let (col, sql_val) = json_to_prop_column(val, is_numeric);
            let val_idx = push_param(params_vec, sql_val);
            let cmp = if matches!(op, FilterOp::Eq) { "=" } else { "!=" };
            if matches!(op, FilterOp::Eq) {
                Ok(format!(
                    "EXISTS (SELECT 1 FROM properties p WHERE p.doc_id = d.id AND p.key = ?{key_idx} AND p.{col} = ?{val_idx})"
                ))
            } else {
                Ok(format!(
                    "NOT EXISTS (SELECT 1 FROM properties p WHERE p.doc_id = d.id AND p.key = ?{key_idx} AND p.{col} {cmp} ?{val_idx} IS FALSE)"
                ))
            }
        }
        FilterOp::Gt | FilterOp::Gte | FilterOp::Lt | FilterOp::Lte => {
            let val = value.ok_or_else(|| {
                NodikleError::InvalidSchema(format!("comparison filter '{property}' requires value"))
            })?;
            let (col, sql_val) = json_to_prop_column(val, is_numeric);
            let val_idx = push_param(params_vec, sql_val);
            let sym = match op {
                FilterOp::Gt => ">",
                FilterOp::Gte => ">=",
                FilterOp::Lt => "<",
                FilterOp::Lte => "<=",
                _ => unreachable!(),
            };
            Ok(format!(
                "EXISTS (SELECT 1 FROM properties p WHERE p.doc_id = d.id AND p.key = ?{key_idx} AND p.{col} {sym} ?{val_idx})"
            ))
        }
        FilterOp::Contains | FilterOp::StartsWith => {
            let s = value
                .and_then(|v| v.as_str())
                .ok_or_else(|| NodikleError::InvalidSchema("text filter requires string".into()))?;
            let pat = if matches!(op, FilterOp::Contains) {
                format!("%{}%", escape_like(s))
            } else {
                format!("{}%", escape_like(s))
            };
            let val_idx = push_param(params_vec, Value::Text(pat));
            Ok(format!(
                "EXISTS (SELECT 1 FROM properties p WHERE p.doc_id = d.id AND p.key = ?{key_idx} AND p.val_text LIKE ?{val_idx} ESCAPE '\\')"
            ))
        }
        FilterOp::In => {
            let list = values.ok_or_else(|| {
                NodikleError::InvalidSchema(format!("'in' filter '{property}' requires values"))
            })?;
            if list.is_empty() {
                return Ok("1=0".to_string());
            }
            let mut placeholders = Vec::with_capacity(list.len());
            let mut col_name = "val_text";
            for item in list {
                let (c, sql_val) = json_to_prop_column(item, is_numeric);
                col_name = c;
                let idx = push_param(params_vec, sql_val);
                placeholders.push(format!("?{idx}"));
            }
            Ok(format!(
                "EXISTS (SELECT 1 FROM properties p WHERE p.doc_id = d.id AND p.key = ?{key_idx} AND p.{col_name} IN ({}))",
                placeholders.join(", ")
            ))
        }
        FilterOp::Between => {
            let list = values.ok_or_else(|| {
                NodikleError::InvalidSchema(format!("'between' filter '{property}' requires 2 values"))
            })?;
            if list.len() != 2 {
                return Err(NodikleError::InvalidSchema(
                    "'between' requires exactly 2 values".into(),
                ));
            }
            let (c1, v1) = json_to_prop_column(&list[0], is_numeric);
            let (_c2, v2) = json_to_prop_column(&list[1], is_numeric);
            let i1 = push_param(params_vec, v1);
            let i2 = push_param(params_vec, v2);
            Ok(format!(
                "EXISTS (SELECT 1 FROM properties p WHERE p.doc_id = d.id AND p.key = ?{key_idx} AND p.{c1} BETWEEN ?{i1} AND ?{i2})"
            ))
        }
    }
}

fn compile_doc_column_condition(
    op: FilterOp,
    col: &str,
    value: Option<&serde_json::Value>,
    values: Option<&[serde_json::Value]>,
    params_vec: &mut Vec<Value>,
) -> Result<String> {
    match op {
        FilterOp::IsEmpty => Ok(format!("COALESCE({col}, '') = ''")),
        FilterOp::IsNotEmpty => Ok(format!("COALESCE({col}, '') != ''")),
        FilterOp::Eq | FilterOp::Neq => {
            let text = json_scalar_to_string(value.ok_or_else(|| {
                NodikleError::InvalidSchema("document filter requires value".into())
            })?);
            let idx = push_param(params_vec, Value::Text(text));
            let sym = if matches!(op, FilterOp::Eq) {
                "="
            } else {
                "!="
            };
            Ok(format!("{col} {sym} ?{idx}"))
        }
        FilterOp::Contains | FilterOp::StartsWith => {
            let s = value
                .and_then(|v| v.as_str())
                .ok_or_else(|| NodikleError::InvalidSchema("text filter requires string".into()))?;
            let pat = if matches!(op, FilterOp::Contains) {
                format!("%{}%", escape_like(s))
            } else {
                format!("{}%", escape_like(s))
            };
            let idx = push_param(params_vec, Value::Text(pat));
            Ok(format!("{col} LIKE ?{idx} ESCAPE '\\'"))
        }
        FilterOp::In => {
            let list = values.unwrap_or(&[]);
            if list.is_empty() {
                return Ok("1=0".to_string());
            }
            let mut ph = Vec::with_capacity(list.len());
            for item in list {
                let idx = push_param(params_vec, Value::Text(json_scalar_to_string(item)));
                ph.push(format!("?{idx}"));
            }
            Ok(format!("{col} IN ({})", ph.join(", ")))
        }
        _ => Err(NodikleError::InvalidSchema(format!(
            "unsupported operator on document column {col}"
        ))),
    }
}

fn document_column_for_key(key: &str) -> Option<&'static str> {
    match key {
        "type" | "doc_type" => Some("d.doc_type"),
        "title" => Some("d.title"),
        "folder" => Some("d.folder"),
        "rel_path" => Some("d.rel_path"),
        _ => None,
    }
}

fn json_to_prop_column(val: &serde_json::Value, prefer_numeric: bool) -> (&'static str, Value) {
    if let Some(n) = val.as_f64() {
        return ("val_num", Value::Real(n));
    }
    if let Some(b) = val.as_bool() {
        return ("val_bool", Value::Integer(i64::from(b)));
    }
    if let Some(s) = val.as_str() {
        if prefer_numeric {
            if let Ok(n) = s.trim().parse::<f64>() {
                return ("val_num", Value::Real(n));
            }
        }
        if nodikle_core::frontmatter::looks_like_iso_date(s) {
            return ("val_date", Value::Text(s.to_string()));
        }
        return ("val_text", Value::Text(s.to_string()));
    }
    ("val_text", Value::Text(val.to_string()))
}

fn json_scalar_to_string(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        other => other.to_string(),
    }
}

fn build_order_by(
    sort_specs: &[SortSpec],
    schema: &ViewSchema,
    params_vec: &mut Vec<Value>,
) -> String {
    if sort_specs.is_empty() {
        return "ORDER BY d.updated_at DESC, d.rel_path ASC".to_string();
    }

    let mut clauses = Vec::with_capacity(sort_specs.len() + 1);
    for spec in sort_specs {
        let dir = match spec.dir {
            SortDir::Asc => "ASC",
            SortDir::Desc => "DESC",
        };
        if let Some(col) = document_column_for_key(&spec.property) {
            clauses.push(format!("{col} {dir}"));
            continue;
        }
        let key_idx = push_param(params_vec, Value::Text(spec.property.clone()));
        let prop_col = match schema.properties.get(&spec.property).map(|p| p.prop_type) {
            Some(PropertyType::Number | PropertyType::Currency) => "val_num",
            Some(PropertyType::Date) => "val_date",
            Some(PropertyType::Checkbox) => "val_bool",
            _ => "val_text",
        };
        clauses.push(format!(
            "(SELECT p.{prop_col} FROM properties p WHERE p.doc_id = d.id AND p.key = ?{key_idx} LIMIT 1) {dir}"
        ));
    }
    clauses.push("d.rel_path ASC".to_string());
    format!("ORDER BY {}", clauses.join(", "))
}

fn load_properties_for_docs(
    conn: &rusqlite::Connection,
    doc_ids: &[String],
    schema: &ViewSchema,
) -> Result<HashMap<String, BTreeMap<String, PropertyValue>>> {
    if doc_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let placeholders: Vec<String> = (1..=doc_ids.len()).map(|i| format!("?{i}")).collect();
    let sql = format!(
        "SELECT doc_id, key, ordinal, value_type, val_text, val_num, val_date, val_bool
         FROM properties
         WHERE doc_id IN ({})
         ORDER BY doc_id, key, ordinal ASC",
        placeholders.join(", ")
    );
    let params_vec: Vec<Value> = doc_ids.iter().cloned().map(Value::Text).collect();
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| NodikleError::IndexDb(e.to_string()))?;

    type RawPropertyRow = (
        String,
        Option<String>,
        Option<f64>,
        Option<String>,
        Option<i64>,
    );
    let mut map: HashMap<String, BTreeMap<String, Vec<RawPropertyRow>>> = HashMap::new();
    let rows = stmt
        .query_map(params_from_iter(params_vec.iter()), |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<f64>>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, Option<i64>>(7)?,
            ))
        })
        .map_err(|e| NodikleError::IndexDb(e.to_string()))?;

    for row in rows {
        let (doc_id, key, vtype, vtext, vnum, vdate, vbool) =
            row.map_err(|e| NodikleError::IndexDb(e.to_string()))?;
        map.entry(doc_id)
            .or_default()
            .entry(key)
            .or_default()
            .push((vtype, vtext, vnum, vdate, vbool));
    }

    let mut result: HashMap<String, BTreeMap<String, PropertyValue>> = HashMap::new();
    for (doc_id, prop_map) in map {
        let mut typed_map = BTreeMap::new();
        for (key, entries) in prop_map {
            let is_multi = matches!(
                schema.properties.get(&key).map(|s| s.prop_type),
                Some(PropertyType::MultiSelect)
            ) || entries.len() > 1;

            if is_multi {
                let list: Vec<String> = entries
                    .into_iter()
                    .filter_map(|(_, vt, _, _, _)| vt)
                    .collect();
                typed_map.insert(key, PropertyValue::List(list));
            } else if let Some((vtype, vt, vn, vd, vb)) = entries.into_iter().next() {
                let val = match vtype.as_str() {
                    "number" => vn.map(PropertyValue::Number).unwrap_or(PropertyValue::Null),
                    "date" => vd.map(PropertyValue::Date).unwrap_or(PropertyValue::Null),
                    "bool" => vb
                        .map(|b| PropertyValue::Bool(b != 0))
                        .unwrap_or(PropertyValue::Null),
                    _ => vt.map(PropertyValue::Text).unwrap_or(PropertyValue::Null),
                };
                typed_map.insert(key, val);
            }
        }
        result.insert(doc_id, typed_map);
    }

    Ok(result)
}

fn load_task_progress_for_docs(
    conn: &rusqlite::Connection,
    doc_ids: &[String],
) -> Result<HashMap<String, (u32, u32)>> {
    if doc_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let placeholders: Vec<String> = (1..=doc_ids.len()).map(|i| format!("?{i}")).collect();
    let sql = format!(
        "SELECT doc_id, SUM(completed), COUNT(*) FROM tasks WHERE doc_id IN ({}) GROUP BY doc_id",
        placeholders.join(", ")
    );
    let params_vec: Vec<Value> = doc_ids.iter().cloned().map(Value::Text).collect();
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| NodikleError::IndexDb(e.to_string()))?;
    let rows = stmt
        .query_map(params_from_iter(params_vec.iter()), |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?.max(0) as u32,
                r.get::<_, i64>(2)?.max(0) as u32,
            ))
        })
        .map_err(|e| NodikleError::IndexDb(e.to_string()))?;

    let mut map = HashMap::new();
    for row in rows {
        let (doc_id, done, total) = row.map_err(|e| NodikleError::IndexDb(e.to_string()))?;
        map.insert(doc_id, (done, total));
    }
    Ok(map)
}

fn compute_aggregations(
    rows: &[ViewRow],
    specs: &BTreeMap<String, AggregationOp>,
) -> BTreeMap<String, f64> {
    let mut out = BTreeMap::new();
    for (prop, op) in specs {
        let val = match op {
            AggregationOp::Count => rows.len() as f64,
            AggregationOp::Sum => rows
                .iter()
                .filter_map(|r| r.properties.get(prop).and_then(PropertyValue::as_number))
                .sum(),
            AggregationOp::Avg => {
                let nums: Vec<f64> = rows
                    .iter()
                    .filter_map(|r| r.properties.get(prop).and_then(PropertyValue::as_number))
                    .collect();
                if nums.is_empty() {
                    0.0
                } else {
                    nums.iter().sum::<f64>() / nums.len() as f64
                }
            }
            AggregationOp::Min => rows
                .iter()
                .filter_map(|r| r.properties.get(prop).and_then(PropertyValue::as_number))
                .fold(f64::INFINITY, f64::min)
                .pipe_finite_or_zero(),
            AggregationOp::Max => rows
                .iter()
                .filter_map(|r| r.properties.get(prop).and_then(PropertyValue::as_number))
                .fold(f64::NEG_INFINITY, f64::max)
                .pipe_finite_or_zero(),
            AggregationOp::CheckedRatio => {
                let done: u32 = rows.iter().map(|r| r.task_completed).sum();
                let total: u32 = rows.iter().map(|r| r.task_total).sum();
                if total == 0 {
                    0.0
                } else {
                    f64::from(done) / f64::from(total)
                }
            }
        };
        out.insert(prop.clone(), val);
    }
    out
}

trait FiniteOrZero {
    fn pipe_finite_or_zero(self) -> f64;
}

impl FiniteOrZero for f64 {
    fn pipe_finite_or_zero(self) -> f64 {
        if self.is_finite() {
            self
        } else {
            0.0
        }
    }
}

fn compute_group_summaries(
    rows: &[ViewRow],
    group_prop: &str,
    specs: &BTreeMap<String, AggregationOp>,
) -> Vec<ViewGroupSummary> {
    let mut grouped: BTreeMap<String, Vec<ViewRow>> = BTreeMap::new();
    for row in rows {
        let key = row
            .properties
            .get(group_prop)
            .and_then(PropertyValue::as_text)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "미분류".to_string());
        grouped.entry(key).or_default().push(row.clone());
    }

    grouped
        .into_iter()
        .map(|(group_key, group_rows)| ViewGroupSummary {
            group_key,
            count: group_rows.len() as u64,
            aggregations: compute_aggregations(&group_rows, specs),
        })
        .collect()
}
