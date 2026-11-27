//! Shared application state, Vault lifecycle, and self-write echo suppression.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use nodikle_core::{
    parse_markdown_document, patch_frontmatter, render_template, slugify, starter_vault_files,
    toggle_task_line,
};
use nodikle_index::SqliteIndex;
use nodikle_model::{
    DocumentDetail, DocumentMeta, FilterExpr, NodikleError, PropertyPatch, PropertyValue,
    ReindexReport, Result, SaveResult, SearchHit, TagCount, TemplateInfo, VaultStats,
    ViewQueryResult, ViewSchema,
};
use nodikle_storage::{normalize_rel_path, DesktopFsStore, LocalFileStore};
use tokio::sync::RwLock;

/// Active Vault handle combining file storage and local SQLite index.
#[derive(Clone)]
pub struct ActiveVault {
    pub store: DesktopFsStore,
    pub index: SqliteIndex,
    pub vault_path: String,
}

/// Short-lived cache of `(rel_path -> (blake3_hash, timestamp))` to suppress duplicate file-watcher events.
#[derive(Debug, Default, Clone)]
pub struct WriteEchoCache {
    entries: Arc<Mutex<HashMap<String, (String, Instant)>>>,
}

impl WriteEchoCache {
    pub fn record(&self, rel_path: &str, hash: &str) {
        if let Ok(mut guard) = self.entries.lock() {
            guard.insert(rel_path.to_string(), (hash.to_string(), Instant::now()));
        }
    }

    pub fn should_suppress(&self, rel_path: &str, hash: &str) -> bool {
        let Ok(mut guard) = self.entries.lock() else {
            return false;
        };
        let now = Instant::now();
        guard.retain(|_, (_, ts)| now.duration_since(*ts) <= Duration::from_secs(2));
        if let Some((recorded_hash, ts)) = guard.get(rel_path) {
            if recorded_hash == hash && now.duration_since(*ts) <= Duration::from_secs(1) {
                return true;
            }
        }
        false
    }
}

/// Global Tauri application state.
#[derive(Clone)]
pub struct AppState {
    pub active_vault: Arc<RwLock<Option<ActiveVault>>>,
    pub echo_cache: WriteEchoCache,
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub watcher_stop: Arc<Mutex<Option<crate::watcher::WatcherHandle>>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            active_vault: Arc::new(RwLock::new(None)),
            echo_cache: WriteEchoCache::default(),
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            watcher_stop: Arc::new(Mutex::new(None)),
        }
    }
}

impl AppState {
    /// Returns a sensible default Vault directory for the current platform.
    pub fn default_vault_dir(app_data_dir: Option<PathBuf>) -> String {
        let base = app_data_dir.unwrap_or_else(|| {
            std::env::var_os("USERPROFILE")
                .or_else(|| std::env::var_os("HOME"))
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir)
        });
        base.join("NodikleVault").to_string_lossy().to_string()
    }

    /// Initializes starter templates, views, and sample documents if missing.
    pub async fn init_sample_vault(&self, vault_path: &str, overwrite: bool) -> Result<VaultStats> {
        let store = DesktopFsStore::open(vault_path)?;
        for (rel_path, content) in starter_vault_files() {
            if !overwrite && store.stat(rel_path).await?.is_some() {
                continue;
            }
            let hash = store
                .write_atomic(rel_path, content.as_bytes(), None)
                .await?;
            self.echo_cache.record(rel_path, &hash);
        }
        self.open_vault(vault_path, None).await
    }

    /// Opens a Vault directory, opens its SQLite index, and performs an incremental reindex.
    pub async fn open_vault(
        &self,
        vault_path: &str,
        db_dir: Option<PathBuf>,
    ) -> Result<VaultStats> {
        let store = DesktopFsStore::open(vault_path)?;
        let canonical = store.root().to_string_lossy().to_string();

        let db_path = match db_dir {
            Some(dir) => {
                let vault_hash = nodikle_core::hash_bytes(canonical.as_bytes());
                dir.join(format!("vault-{}.db", &vault_hash[..12]))
            }
            None => store.root().join(".nodikle").join("index.db"),
        };

        let index = SqliteIndex::open(&db_path, canonical.clone())?;
        let _ = index.reindex_vault(&store, false).await?;
        let stats = index.stats()?;

        let mut guard = self.active_vault.write().await;
        *guard = Some(ActiveVault {
            store,
            index,
            vault_path: canonical,
        });

        Ok(stats)
    }

    pub async fn require_vault(&self) -> Result<ActiveVault> {
        self.active_vault
            .read()
            .await
            .clone()
            .ok_or_else(|| NodikleError::NotFound("no active vault opened".into()))
    }

    pub async fn reindex(&self, full_rebuild: bool) -> Result<ReindexReport> {
        let vault = self.require_vault().await?;
        vault.index.reindex_vault(&vault.store, full_rebuild).await
    }

    pub async fn stats(&self) -> Result<VaultStats> {
        let vault = self.require_vault().await?;
        vault.index.stats()
    }

    pub async fn list_documents(
        &self,
        folder: Option<&str>,
        tag_prefix: Option<&str>,
    ) -> Result<Vec<DocumentMeta>> {
        let vault = self.require_vault().await?;
        vault.index.list_documents(folder, tag_prefix)
    }

    pub async fn get_document(&self, rel_path: &str) -> Result<DocumentDetail> {
        let vault = self.require_vault().await?;
        let norm = normalize_rel_path(rel_path)?;
        let bytes = vault.store.read_bytes(&norm).await?;
        let raw_md = String::from_utf8_lossy(&bytes).into_owned();
        let meta = match vault.index.get_document_by_path(&norm)? {
            Some(m) => m,
            None => {
                let st = vault
                    .store
                    .stat(&norm)
                    .await?
                    .ok_or_else(|| NodikleError::NotFound(norm.clone()))?;
                vault.index.index_single_file(&st, &raw_md)?
            }
        };

        let parsed = parse_markdown_document(&raw_md)?;
        let backlinks = vault.index.get_backlinks(&meta.id)?;

        Ok(DocumentDetail {
            meta,
            raw_md,
            body_md: parsed.body_md,
            properties: parsed.properties,
            ordered_keys: parsed.ordered_keys,
            tags: parsed.tags,
            wikilinks: parsed.wikilinks,
            backlinks,
            tasks: parsed.tasks,
        })
    }

    pub async fn save_document(
        &self,
        rel_path: &str,
        raw_md: &str,
        expected_hash: Option<&str>,
    ) -> Result<SaveResult> {
        let vault = self.require_vault().await?;
        let norm = normalize_rel_path(rel_path)?;
        let new_hash = vault
            .store
            .write_atomic(&norm, raw_md.as_bytes(), expected_hash)
            .await?;
        self.echo_cache.record(&norm, &new_hash);

        let meta = vault
            .store
            .stat(&norm)
            .await?
            .ok_or_else(|| NodikleError::NotFound(norm.clone()))?;
        let doc_meta = vault.index.index_single_file(&meta, raw_md)?;

        Ok(SaveResult {
            rel_path: norm,
            new_hash,
            updated_at: doc_meta.updated_at,
        })
    }

    pub async fn delete_document(&self, rel_path: &str) -> Result<()> {
        let vault = self.require_vault().await?;
        let norm = normalize_rel_path(rel_path)?;
        vault.store.delete_entry(&norm).await?;
        vault.index.remove_document(&norm)?;
        Ok(())
    }

    pub async fn patch_properties(
        &self,
        rel_path: &str,
        patches: Vec<PropertyPatch>,
    ) -> Result<SaveResult> {
        let vault = self.require_vault().await?;
        let norm = normalize_rel_path(rel_path)?;
        let bytes = vault.store.read_bytes(&norm).await?;
        let current_hash = nodikle_core::hash_bytes(&bytes);
        let raw_md = String::from_utf8_lossy(&bytes).into_owned();
        let patched = patch_frontmatter(&raw_md, &patches)?;
        self.save_document(&norm, &patched, Some(&current_hash))
            .await
    }

    pub async fn toggle_task(
        &self,
        rel_path: &str,
        line_no: usize,
        completed: bool,
    ) -> Result<SaveResult> {
        let vault = self.require_vault().await?;
        let norm = normalize_rel_path(rel_path)?;
        let bytes = vault.store.read_bytes(&norm).await?;
        let current_hash = nodikle_core::hash_bytes(&bytes);
        let raw_md = String::from_utf8_lossy(&bytes).into_owned();
        let updated = toggle_task_line(&raw_md, line_no, completed)?;
        self.save_document(&norm, &updated, Some(&current_hash))
            .await
    }

    pub async fn list_templates(&self) -> Result<Vec<TemplateInfo>> {
        let vault = self.require_vault().await?;
        let entries = vault.store.list_entries("_templates").await?;
        Ok(entries
            .into_iter()
            .filter(|e| !e.is_dir && e.rel_path.ends_with(".md"))
            .map(|e| {
                let name = e
                    .rel_path
                    .rsplit('/')
                    .next()
                    .unwrap_or(&e.rel_path)
                    .trim_end_matches(".md")
                    .to_string();
                TemplateInfo {
                    rel_path: e.rel_path,
                    name,
                }
            })
            .collect())
    }

    pub async fn create_from_template(
        &self,
        view_id: Option<&str>,
        template_path: Option<&str>,
        title: &str,
        vars: std::collections::BTreeMap<String, PropertyValue>,
    ) -> Result<DocumentDetail> {
        let vault = self.require_vault().await?;
        let schema = match view_id {
            Some(id) => Some(self.get_view_schema(id).await?),
            None => None,
        };

        let tpl_rel = template_path
            .map(str::to_string)
            .or_else(|| schema.as_ref().and_then(|s| s.default_template.clone()))
            .ok_or_else(|| NodikleError::TemplateRender("no template path specified".into()))?;

        let tpl_bytes = vault.store.read_bytes(&tpl_rel).await?;
        let tpl_raw = String::from_utf8_lossy(&tpl_bytes).into_owned();
        let rendered = render_template(&tpl_raw, title, &vars);

        let filename_pattern = schema
            .as_ref()
            .and_then(|s| s.filename_pattern.clone())
            .unwrap_or_else(|| "notes/{{date}}-{{slug}}.md".to_string());
        let candidate = render_template(&filename_pattern, title, &vars);
        let norm_candidate = normalize_rel_path(&candidate)?;
        let final_path = ensure_unique_path(&vault.store, &norm_candidate).await?;

        self.save_document(&final_path, &rendered, None).await?;
        self.get_document(&final_path).await
    }

    pub async fn list_views(&self) -> Result<Vec<ViewSchema>> {
        let vault = self.require_vault().await?;
        let entries = vault.store.list_entries(".nodikle/views").await?;
        let mut schemas = Vec::new();
        for entry in entries {
            if entry.is_dir
                || (!entry.rel_path.ends_with(".yaml") && !entry.rel_path.ends_with(".yml"))
            {
                continue;
            }
            let bytes = vault.store.read_bytes(&entry.rel_path).await?;
            let yaml = String::from_utf8_lossy(&bytes);
            let schema: ViewSchema = serde_yaml::from_str(&yaml)
                .map_err(|e| NodikleError::InvalidSchema(format!("{}: {e}", entry.rel_path)))?;
            schemas.push(schema);
        }
        schemas.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(schemas)
    }

    pub async fn get_view_schema(&self, view_id: &str) -> Result<ViewSchema> {
        let views = self.list_views().await?;
        views
            .into_iter()
            .find(|v| v.id == view_id)
            .ok_or_else(|| NodikleError::NotFound(format!("view schema '{view_id}'")))
    }

    pub async fn query_view(
        &self,
        view_id: &str,
        layout_id: Option<&str>,
        runtime_filter: Option<&FilterExpr>,
        page: usize,
        limit: usize,
    ) -> Result<ViewQueryResult> {
        let vault = self.require_vault().await?;
        let schema = self.get_view_schema(view_id).await?;
        vault
            .index
            .query_view(&schema, layout_id, runtime_filter, page, limit)
    }

    pub async fn search_fts(
        &self,
        query: &str,
        tag_prefix: Option<&str>,
        folder: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SearchHit>> {
        let vault = self.require_vault().await?;
        vault.index.search_fts(query, tag_prefix, folder, limit)
    }

    pub async fn list_tags(&self) -> Result<Vec<TagCount>> {
        let vault = self.require_vault().await?;
        vault.index.list_tags()
    }

    pub async fn resolve_wikilink(&self, raw_target: &str) -> Result<Option<String>> {
        let vault = self.require_vault().await?;
        vault.index.resolve_wikilink_path(raw_target)
    }
}

async fn ensure_unique_path(store: &DesktopFsStore, rel_path: &str) -> Result<String> {
    if store.stat(rel_path).await?.is_none() {
        return Ok(rel_path.to_string());
    }
    let stem = rel_path.strip_suffix(".md").unwrap_or(rel_path);
    for idx in 2..1000 {
        let candidate = format!("{stem}-{idx}.md");
        if store.stat(&candidate).await?.is_none() {
            return Ok(candidate);
        }
    }
    let fallback = format!(
        "{stem}-{}.md",
        slugify(&chrono::Local::now().format("%H%M%S").to_string())
    );
    Ok(fallback)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn write_echo_cache_suppresses_matching_hash_briefly() {
        let cache = WriteEchoCache::default();
        cache.record("notes/a.md", "hash123");
        assert!(cache.should_suppress("notes/a.md", "hash123"));
        assert!(!cache.should_suppress("notes/a.md", "other"));
    }

    #[tokio::test]
    async fn app_state_sample_vault_template_patch_and_query_flow() {
        let tmp = TempDir::new().expect("tempdir");
        let state = AppState::default();
        let vault_dir = tmp.path().to_string_lossy().to_string();

        let stats = state
            .init_sample_vault(&vault_dir, false)
            .await
            .expect("init sample vault");
        assert_eq!(stats.doc_count, 7);

        // Create a new expense entry from template.
        let mut vars = BTreeMap::new();
        vars.insert("amount".into(), PropertyValue::Number(15000.0));
        vars.insert("category".into(), PropertyValue::Text("식비".into()));
        vars.insert("date".into(), PropertyValue::Text("2026-11-27".into()));

        let created = state
            .create_from_template(Some("expenses"), None, "저녁 우동", vars)
            .await
            .expect("create from template");
        assert!(created.meta.rel_path.starts_with("finance/2026-11-27-"));

        // Patch amount and verify view query sum reflects update.
        state
            .patch_properties(
                &created.meta.rel_path,
                vec![PropertyPatch {
                    key: "amount".into(),
                    value: Some(PropertyValue::Number(20000.0)),
                }],
            )
            .await
            .expect("patch");

        let res = state
            .query_view("expenses", Some("monthly-table"), None, 0, 50)
            .await
            .expect("query expenses");
        assert_eq!(res.total_count, 4);
        assert_eq!(res.aggregations.get("amount").copied(), Some(94700.0));

        // Toggle an incomplete task on Kyoto trip and verify task progress updates.
        let kyoto_doc = state
            .get_document("trips/2026-12-kyoto.md")
            .await
            .expect("get kyoto doc");
        let incomplete_line = kyoto_doc
            .tasks
            .iter()
            .find(|t| !t.completed)
            .expect("has incomplete task")
            .line_no;
        state
            .toggle_task("trips/2026-12-kyoto.md", incomplete_line, true)
            .await
            .expect("toggle task");
        let trip_res = state
            .query_view("trips", Some("trip-board"), None, 0, 50)
            .await
            .expect("query trips");
        let kyoto = trip_res
            .rows
            .iter()
            .find(|r| r.rel_path == "trips/2026-12-kyoto.md")
            .expect("kyoto");
        assert_eq!(kyoto.task_completed, 3);
        assert_eq!(kyoto.task_total, 4);
    }
}
