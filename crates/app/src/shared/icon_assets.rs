// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Embedded icon assets used by both the search UI and the settings UI.

use std::path::Path;

const APP_PNG: &[u8] = include_bytes!("../../assets/icons/flow/app.png");
const MISSING_PNG: &[u8] = include_bytes!("../../assets/icons/flow/app_missing_img.png");
const FILE_PNG: &[u8] = include_bytes!("../../assets/icons/flow/file.png");
const FOLDER_PNG: &[u8] = include_bytes!("../../assets/icons/flow/folder.png");
const LOADING_PNG: &[u8] = include_bytes!("../../assets/icons/flow/loading.png");
const PLUGIN_PNG: &[u8] = include_bytes!("../../assets/icons/flow/plugin.png");
const SETTINGS_PNG: &[u8] = include_bytes!("../../assets/icons/flow/settings.png");
const CMD_PNG: &[u8] = include_bytes!("../../assets/icons/flow/cmd.png");
const BROWSER_PNG: &[u8] = include_bytes!("../../assets/icons/flow/Browser.png");
const HISTORY_PNG: &[u8] = include_bytes!("../../assets/icons/flow/history.png");
const RESTART_PNG: &[u8] = include_bytes!("../../assets/icons/flow/restart.png");
const SHUTDOWN_PNG: &[u8] = include_bytes!("../../assets/icons/flow/shutdown.png");
const LOCK_PNG: &[u8] = include_bytes!("../../assets/icons/flow/lock.png");
const SLEEP_PNG: &[u8] = include_bytes!("../../assets/icons/flow/sleep.png");
const LOGOFF_PNG: &[u8] = include_bytes!("../../assets/icons/flow/logoff.png");
const RECYCLEBIN_PNG: &[u8] = include_bytes!("../../assets/icons/flow/recyclebin.png");

fn normalize_named_icon(name: &str) -> Option<&'static str> {
    match name.trim().to_ascii_lowercase().as_str() {
        "app" | "program" | "uwp-app" | "builtin:program" => Some("app"),
        "file" | "builtin:file" => Some("file"),
        "folder" | "builtin:folder" => Some("folder"),
        "missing" | "builtin:missing" => Some("missing"),
        "loading" => Some("loading"),
        "plugin" => Some("plugin"),
        "settings" => Some("settings"),
        "system" | "cmd" => Some("cmd"),
        "bookmark" | "browser" => Some("browser"),
        "star" | "quicklaunch" | "quick-launch" => Some("history"),
        "restart" => Some("restart"),
        "shutdown" => Some("shutdown"),
        "lock" => Some("lock"),
        "sleep" | "hibernate" => Some("sleep"),
        "logoff" | "logout" => Some("logoff"),
        "recyclebin" | "recycle-bin" => Some("recyclebin"),
        _ => None,
    }
}

pub fn named_icon_bytes(name: &str) -> Option<&'static [u8]> {
    match normalize_named_icon(name)? {
        "app" => Some(APP_PNG),
        "file" => Some(FILE_PNG),
        "folder" => Some(FOLDER_PNG),
        "missing" => Some(MISSING_PNG),
        "loading" => Some(LOADING_PNG),
        "plugin" => Some(PLUGIN_PNG),
        "settings" => Some(SETTINGS_PNG),
        "cmd" => Some(CMD_PNG),
        "browser" => Some(BROWSER_PNG),
        "history" => Some(HISTORY_PNG),
        "restart" => Some(RESTART_PNG),
        "shutdown" => Some(SHUTDOWN_PNG),
        "lock" => Some(LOCK_PNG),
        "sleep" => Some(SLEEP_PNG),
        "logoff" => Some(LOGOFF_PNG),
        "recyclebin" => Some(RECYCLEBIN_PNG),
        _ => None,
    }
}

pub fn is_named_icon(name: &str) -> bool {
    normalize_named_icon(name).is_some()
}

pub fn is_filesystem_path(value: &str) -> bool {
    if value.trim().is_empty() || is_named_icon(value) || looks_like_uri(value) {
        return false;
    }

    let path = Path::new(value);
    path.is_absolute() || value.starts_with("\\\\") || value.contains('\\')
}

fn looks_like_uri(value: &str) -> bool {
    let Some(colon) = value.find(':') else {
        return false;
    };

    if colon <= 1 {
        return false;
    }

    let scheme = &value[..colon];
    scheme
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'))
}
