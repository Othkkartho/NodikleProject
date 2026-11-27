//! Typed Tauri v2 IPC commands exposed to the frontend.

use std::collections::BTreeMap;
use std::path::PathBuf;

use nodikle_model::{
    DocumentDetail, DocumentMeta, FilterExpr, IpcError, PropertyPatch, PropertyValue,
    ReindexReport, SaveResult, SearchHit, TagCount, TemplateInfo, VaultStats, ViewQueryResult,
    ViewSchema,
};
use tauri::{AppHandle, Manager, State};

use crate::state::AppState;

type CmdResult<T> = std::result::Result<T, IpcError>;

#[tauri::command]
pub async fn vault_get_default_path(app: AppHandle) -> CmdResult<String> {
    let base = app.path().app_data_dir().ok();
    Ok(AppState::default_vault_dir(base))
}

#[tauri::command]
pub async fn vault_init_sample(
    app: AppHandle,
    state: State<'_, AppState>,
    vault_path: String,
    overwrite: Option<bool>,
) -> CmdResult<VaultStats> {
    let stats = state
        .init_sample_vault(&vault_path, overwrite.unwrap_or(false))
        .await
        .map_err(IpcError::from)?;
    ensure_watcher(&app, &state, &stats.vault_path);
    Ok(stats)
}

#[tauri::command]
pub async fn vault_open(
    app: AppHandle,
    state: State<'_, AppState>,
    vault_path: String,
) -> CmdResult<VaultStats> {
    let db_dir = mobile_db_dir(&app);
    let stats = state
        .open_vault(&vault_path, db_dir)
        .await
        .map_err(IpcError::from)?;
    ensure_watcher(&app, &state, &stats.vault_path);
    Ok(stats)
}

#[tauri::command]
pub async fn vault_reindex(
    state: State<'_, AppState>,
    full_rebuild: Option<bool>,
) -> CmdResult<ReindexReport> {
    state
        .reindex(full_rebuild.unwrap_or(false))
        .await
        .map_err(IpcError::from)
}

#[tauri::command]
pub async fn vault_stats(state: State<'_, AppState>) -> CmdResult<VaultStats> {
    state.stats().await.map_err(IpcError::from)
}

#[tauri::command]
pub async fn doc_list(
    state: State<'_, AppState>,
    folder: Option<String>,
    tag_prefix: Option<String>,
) -> CmdResult<Vec<DocumentMeta>> {
    state
        .list_documents(folder.as_deref(), tag_prefix.as_deref())
        .await
        .map_err(IpcError::from)
}

#[tauri::command]
pub async fn doc_get(state: State<'_, AppState>, rel_path: String) -> CmdResult<DocumentDetail> {
    state.get_document(&rel_path).await.map_err(IpcError::from)
}

#[tauri::command]
pub async fn doc_save(
    state: State<'_, AppState>,
    rel_path: String,
    raw_md: String,
    expected_hash: Option<String>,
) -> CmdResult<SaveResult> {
    state
        .save_document(&rel_path, &raw_md, expected_hash.as_deref())
        .await
        .map_err(IpcError::from)
}

#[tauri::command]
pub async fn doc_delete(state: State<'_, AppState>, rel_path: String) -> CmdResult<()> {
    state
        .delete_document(&rel_path)
        .await
        .map_err(IpcError::from)
}

#[tauri::command]
pub async fn doc_patch_properties(
    state: State<'_, AppState>,
    rel_path: String,
    patches: Vec<PropertyPatch>,
) -> CmdResult<SaveResult> {
    state
        .patch_properties(&rel_path, patches)
        .await
        .map_err(IpcError::from)
}

#[tauri::command]
pub async fn doc_toggle_task(
    state: State<'_, AppState>,
    rel_path: String,
    line_no: usize,
    completed: bool,
) -> CmdResult<SaveResult> {
    state
        .toggle_task(&rel_path, line_no, completed)
        .await
        .map_err(IpcError::from)
}

#[tauri::command]
pub async fn template_list(state: State<'_, AppState>) -> CmdResult<Vec<TemplateInfo>> {
    state.list_templates().await.map_err(IpcError::from)
}

#[tauri::command]
pub async fn doc_create_from_template(
    state: State<'_, AppState>,
    view_id: Option<String>,
    template_path: Option<String>,
    title: String,
    vars: Option<BTreeMap<String, PropertyValue>>,
) -> CmdResult<DocumentDetail> {
    state
        .create_from_template(
            view_id.as_deref(),
            template_path.as_deref(),
            &title,
            vars.unwrap_or_default(),
        )
        .await
        .map_err(IpcError::from)
}

#[tauri::command]
pub async fn view_list(state: State<'_, AppState>) -> CmdResult<Vec<ViewSchema>> {
    state.list_views().await.map_err(IpcError::from)
}

#[tauri::command]
pub async fn view_query(
    state: State<'_, AppState>,
    view_id: String,
    layout_id: Option<String>,
    runtime_filter: Option<FilterExpr>,
    page: Option<usize>,
    limit: Option<usize>,
) -> CmdResult<ViewQueryResult> {
    state
        .query_view(
            &view_id,
            layout_id.as_deref(),
            runtime_filter.as_ref(),
            page.unwrap_or(0),
            limit.unwrap_or(200),
        )
        .await
        .map_err(IpcError::from)
}

#[tauri::command]
pub async fn search_fts(
    state: State<'_, AppState>,
    query: String,
    tag_prefix: Option<String>,
    folder: Option<String>,
    limit: Option<usize>,
) -> CmdResult<Vec<SearchHit>> {
    state
        .search_fts(
            &query,
            tag_prefix.as_deref(),
            folder.as_deref(),
            limit.unwrap_or(50),
        )
        .await
        .map_err(IpcError::from)
}

#[tauri::command]
pub async fn tags_list(state: State<'_, AppState>) -> CmdResult<Vec<TagCount>> {
    state.list_tags().await.map_err(IpcError::from)
}

#[tauri::command]
pub async fn wikilink_resolve(
    state: State<'_, AppState>,
    raw_target: String,
) -> CmdResult<Option<String>> {
    state
        .resolve_wikilink(&raw_target)
        .await
        .map_err(IpcError::from)
}

#[cfg(any(target_os = "android", target_os = "ios"))]
fn mobile_db_dir(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_data_dir().ok().map(|p| p.join("db"))
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn mobile_db_dir(_app: &AppHandle) -> Option<PathBuf> {
    None
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn ensure_watcher(app: &AppHandle, state: &AppState, vault_path: &str) {
    if let Ok(handle) =
        crate::watcher::start_watcher(app.clone(), state.clone(), PathBuf::from(vault_path))
    {
        if let Ok(mut guard) = state.watcher_stop.lock() {
            *guard = Some(handle);
        }
    }
}

#[cfg(any(target_os = "android", target_os = "ios"))]
fn ensure_watcher(_app: &AppHandle, _state: &AppState, _vault_path: &str) {}
