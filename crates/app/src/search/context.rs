// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Context actions page helpers.

use super::renderer::DisplayItem;

#[cfg(windows)]
pub fn build_context_items(source: &DisplayItem) -> Vec<DisplayItem> {
    source
        .context_actions
        .iter()
        .map(|entry| DisplayItem {
            title: entry.label.clone(),
            subtitle: source.subtitle.clone(),
            icon: source.icon.clone(),
            shortcut: entry.shortcut_hint.clone(),
            action: entry.action.clone(),
            context_actions: Vec::new(),
            context_data: source.context_data.clone(),
            icon_path: source.icon_path.clone(),
            is_directory: source.is_directory,
            highlight: Vec::new(),
            score: source.score,
        })
        .collect()
}
