// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Cache maintenance actions initiated by the tray menu.

#[cfg(windows)]
pub(super) fn clear_cache_and_rebuild() {
    let result = super::app_state::with_app_mut(|app| {
        app.index_ready = false;
        app.index_error = None;
        app.index_status = app.i18n.get("placeholder_indexing").to_string();
        app.engine
            .as_ref()
            .ok_or_else(|| "search engine is not initialized".to_string())?
            .clear_cache_and_rebuild()
    });

    match result {
        Some(Ok(count)) => {
            easysearch_core::log_info!("cleared encrypted cache; rebuilding {count} drive(s)");
            super::render_bridge::request_render();
        }
        Some(Err(error)) => {
            easysearch_core::log_error!("failed to clear cache and rebuild: {error}");
        }
        None => easysearch_core::log_error!("failed to access app state for cache rebuild"),
    }
}
