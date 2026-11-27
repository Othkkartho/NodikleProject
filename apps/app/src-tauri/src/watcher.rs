//! Desktop file system watcher with 150ms debounce and echo suppression.

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

use nodikle_core::hash_bytes;
use nodikle_storage::{is_ignored_path, LocalFileStore};
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;

use crate::state::AppState;

/// Dropping this handle stops the background file watcher thread.
pub struct WatcherHandle {
    _watcher: RecommendedWatcher,
}

/// Starts watching `vault_root` for external Markdown/schema edits and emits `vault:file_changed`.
pub fn start_watcher(
    app: AppHandle,
    state: AppState,
    vault_root: PathBuf,
) -> Result<WatcherHandle, String> {
    let (tx, mut rx) = mpsc::unbounded_channel::<PathBuf>();
    let root_clone = vault_root.clone();

    let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
        if let Ok(event) = res {
            for path in event.paths {
                let _ = tx.send(path);
            }
        }
    })
    .map_err(|e| e.to_string())?;

    watcher
        .watch(&vault_root, RecursiveMode::Recursive)
        .map_err(|e| e.to_string())?;

    tauri::async_runtime::spawn(async move {
        let mut pending = HashSet::<String>::new();
        loop {
            let Some(first) = rx.recv().await else {
                break;
            };
            collect_rel_path(&root_clone, first, &mut pending);

            // Debounce 150ms to coalesce editor save bursts.
            tokio::time::sleep(Duration::from_millis(150)).await;
            while let Ok(next) = rx.try_recv() {
                collect_rel_path(&root_clone, next, &mut pending);
            }

            if pending.is_empty() {
                continue;
            }

            let Ok(vault) = state.require_vault().await else {
                pending.clear();
                continue;
            };

            let mut changed_paths = Vec::new();
            for rel_path in pending.drain() {
                if !rel_path.ends_with(".md")
                    && !rel_path.ends_with(".yaml")
                    && !rel_path.ends_with(".yml")
                {
                    continue;
                }

                match vault.store.stat(&rel_path).await {
                    Ok(Some(meta)) if !meta.is_dir => {
                        if rel_path.ends_with(".md") && !rel_path.starts_with("_templates/") {
                            if let Ok(bytes) = vault.store.read_bytes(&rel_path).await {
                                let hash = hash_bytes(&bytes);
                                if state.echo_cache.should_suppress(&rel_path, &hash) {
                                    continue;
                                }
                                let raw = String::from_utf8_lossy(&bytes);
                                let _ = vault.index.index_single_file(&meta, &raw);
                                changed_paths.push(rel_path);
                            }
                        } else {
                            changed_paths.push(rel_path);
                        }
                    }
                    Ok(_) => {
                        if rel_path.ends_with(".md") {
                            let _ = vault.index.remove_document(&rel_path);
                        }
                        changed_paths.push(rel_path);
                    }
                    Err(_) => {}
                }
            }

            if !changed_paths.is_empty() {
                let _ = app.emit(
                    "vault:file_changed",
                    serde_json::json!({ "rel_paths": changed_paths }),
                );
            }
        }
    });

    Ok(WatcherHandle { _watcher: watcher })
}

fn collect_rel_path(root: &PathBuf, abs_path: PathBuf, out: &mut HashSet<String>) {
    if let Ok(rel) = abs_path.strip_prefix(root) {
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        if !rel_str.is_empty() && !is_ignored_path(&rel_str) {
            out.insert(rel_str);
        }
    }
}
