//! Tauri v2 application host wiring Rust core crates to Desktop and Mobile UIs.

pub mod commands;
pub mod state;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub mod watcher;

use state::AppState;

/// Configures and starts the Tauri v2 application.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();

    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::vault_get_default_path,
            commands::vault_init_sample,
            commands::vault_open,
            commands::vault_reindex,
            commands::vault_stats,
            commands::doc_list,
            commands::doc_get,
            commands::doc_save,
            commands::doc_delete,
            commands::doc_patch_properties,
            commands::doc_toggle_task,
            commands::template_list,
            commands::doc_create_from_template,
            commands::view_list,
            commands::view_query,
            commands::search_fts,
            commands::tags_list,
            commands::wikilink_resolve,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Nodikle application");
}
