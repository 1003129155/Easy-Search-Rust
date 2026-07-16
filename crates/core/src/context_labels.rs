// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Localized labels for context actions.
//!
//! Provides a global locale setting and helper functions so that all plugins
//! can produce localized context-action labels without depending on the app
//! crate's I18nEngine.

use std::sync::RwLock;

static CURRENT_LOCALE: RwLock<String> = RwLock::new(String::new());

/// Set the global locale used by context label helpers.
/// Call this from the app crate at startup and whenever the user changes language.
pub fn set_locale(locale: &str) {
    if let Ok(mut lock) = CURRENT_LOCALE.write() {
        *lock = locale.to_string();
    }
}

/// Get the current locale prefix (e.g. "zh", "ja", "en").
fn locale_prefix() -> String {
    let locale = CURRENT_LOCALE
        .read()
        .map(|s| s.clone())
        .unwrap_or_default();
    locale
        .split('-')
        .next()
        .unwrap_or("en")
        .to_string()
}

/// Get the current locale prefix (public version for use by plugins).
pub fn get_locale_prefix() -> String {
    locale_prefix()
}

// ─── File / Folder context actions ──────────────────────────────────────────

/// "Open file" or "Open folder"
pub fn open_item(is_directory: bool) -> String {
    match locale_prefix().as_str() {
        "zh" => {
            if is_directory { "打开文件夹" } else { "打开文件" }
        }
        "ja" => {
            if is_directory { "フォルダーを開く" } else { "ファイルを開く" }
        }
        _ => {
            if is_directory { "Open folder" } else { "Open file" }
        }
    }
    .to_string()
}

/// "Open containing folder" or "Open parent folder"
pub fn open_containing_folder(is_directory: bool) -> String {
    match locale_prefix().as_str() {
        "zh" => {
            if is_directory { "打开上级文件夹" } else { "打开所在文件夹" }
        }
        "ja" => {
            if is_directory { "親フォルダーを開く" } else { "格納フォルダーを開く" }
        }
        _ => {
            if is_directory { "Open parent folder" } else { "Open containing folder" }
        }
    }
    .to_string()
}

/// "Open parent folder" (for files that also want a separate parent-folder entry)
pub fn open_parent_folder() -> String {
    match locale_prefix().as_str() {
        "zh" => "打开上级文件夹",
        "ja" => "親フォルダーを開く",
        _ => "Open parent folder",
    }
    .to_string()
}

/// "Copy path"
pub fn copy_path() -> String {
    match locale_prefix().as_str() {
        "zh" => "复制路径",
        "ja" => "パスをコピー",
        _ => "Copy path",
    }
    .to_string()
}

/// "Copy name"
pub fn copy_name() -> String {
    match locale_prefix().as_str() {
        "zh" => "复制名称",
        "ja" => "名前をコピー",
        _ => "Copy name",
    }
    .to_string()
}

/// "Add to Quick Launch" / "Remove from Quick Launch"
pub fn toggle_quick_launch(is_saved: bool) -> String {
    match locale_prefix().as_str() {
        "zh" => {
            if is_saved { "从快速启动移除" } else { "添加到快速启动" }
        }
        "ja" => {
            if is_saved { "クイック起動から削除" } else { "クイック起動に追加" }
        }
        _ => {
            if is_saved { "Remove from Quick Launch" } else { "Add to Quick Launch" }
        }
    }
    .to_string()
}

/// "Search in this folder"
pub fn search_in_folder() -> String {
    match locale_prefix().as_str() {
        "zh" => "在此文件夹中搜索",
        "ja" => "このフォルダー内を検索",
        _ => "Search in this folder",
    }
    .to_string()
}

/// "Windows context menu"
pub fn windows_context_menu() -> String {
    match locale_prefix().as_str() {
        "zh" => "Windows 右键菜单",
        "ja" => "Windows コンテキストメニュー",
        _ => "Windows context menu",
    }
    .to_string()
}

// ─── Program-specific context actions ───────────────────────────────────────

/// "Run as administrator"
pub fn run_as_admin() -> String {
    match locale_prefix().as_str() {
        "zh" => "以管理员身份运行",
        "ja" => "管理者として実行",
        _ => "Run as administrator",
    }
    .to_string()
}

/// "Open file location"
pub fn open_file_location() -> String {
    match locale_prefix().as_str() {
        "zh" => "打开文件位置",
        "ja" => "ファイルの場所を開く",
        _ => "Open file location",
    }
    .to_string()
}
