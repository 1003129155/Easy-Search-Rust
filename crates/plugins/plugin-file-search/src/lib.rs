// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! File search plugin: bridges the search engine into the plugin system.
//!
//! This is a thin adapter — it takes user input, calls `SearchEngine::search()`,
//! and converts `EsSearchResult` into `PluginResult`.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use easysearch_core::{
    Action, CancelToken, ContextAction, ContextData, EsSearchResult, Plugin, PluginResult,
};
use easysearch_engine::{SearchEngine, SearchSession};
use quick_launch_store::global_store;

const MAX_FILE_RESULTS: usize = 50;

/// File search plugin that wraps the search engine.
pub struct FileSearchPlugin {
    engine: Arc<SearchEngine>,
    session: Mutex<SearchSession>,
    reset_requested: AtomicBool,
}

impl FileSearchPlugin {
    /// Create a new file search plugin backed by the given engine instance.
    #[must_use]
    pub fn new(engine: Arc<SearchEngine>) -> Self {
        Self {
            engine,
            session: Mutex::new(SearchSession::new()),
            reset_requested: AtomicBool::new(false),
        }
    }

    fn query_inner(&self, query: &str, cancel: Option<&CancelToken>) -> Vec<PluginResult> {
        let q = query.trim();
        if q.is_empty() {
            self.reset_search_session();
            return Vec::new();
        }

        let results = if q.starts_with('\\') {
            self.reset_search_session();
            let path = q.trim_start_matches('\\');
            match cancel {
                Some(token) => self
                    .engine
                    .enumerate_with_cancel(path, "", false, MAX_FILE_RESULTS, token.as_ref())
                    .unwrap_or_default(),
                None => self
                    .engine
                    .enumerate(path, "", false, MAX_FILE_RESULTS)
                    .unwrap_or_default(),
            }
        } else {
            let mut session = self
                .session
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            // ② 搜索开始前：检查在等锁期间 UI 线程是否发过 reset 请求。
            //    swap → false 意味着"我已经处理了这次请求"。
            if self.reset_requested.swap(false, Ordering::AcqRel) {
                session.reset();
            }
            let results = match cancel {
                Some(token) => self.engine.search_with_session_and_cancel(
                    &mut session,
                    q,
                    MAX_FILE_RESULTS,
                    token.as_ref(),
                ),
                None => self
                    .engine
                    .search_with_session(&mut session, q, MAX_FILE_RESULTS),
            };
            // ② 搜索结束后、drop 锁之前：处理搜索过程中到达的 reset 请求。
            //    竞态窗口：UI 线程在"搜索开始前 swap" 和这里之间调用了 reset_search_session，
            //    try_lock 失败，flag 被置为 true，需要在这里清掉并 reset。
            if self.reset_requested.swap(false, Ordering::AcqRel) {
                session.reset();
            }
            drop(session);
            // ③ 见 finish_pending_reset 注释。
            self.finish_pending_reset();
            results
        };

        results.into_iter().map(es_result_to_plugin).collect()
    }

    fn finish_pending_reset(&self) {
        // ③ 封堵最后一个极短竞态窗口：
        //    时序：worker 执行"②搜索结束后 swap → false"（无 reset 请求）
        //          → UI 线程此时调用 reset_search_session，try_lock 仍失败（锁未释放）→ flag = true
        //          → worker drop(session) 释放锁
        //          → 来到这里：swap → true，重新获锁并 reset。
        //    如果没有这一步，这个窗口里的 reset 请求会被永久丢失。
        if self.reset_requested.swap(false, Ordering::AcqRel) {
            let mut session = self
                .session
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            session.reset();
        }
    }
}

impl Plugin for FileSearchPlugin {
    fn default_keyword(&self) -> Option<&str> {
        None // always participates in search (no keyword prefix)
    }

    fn matches(&self, query: &str) -> bool {
        // Participate when engine is ready and query is non-empty
        !query.trim().is_empty() && self.engine.is_ready()
    }

    fn needs_background(&self) -> bool {
        true // MFT index search is I/O-heavy; must run off the UI thread
    }

    fn query(&self, query: &str) -> Vec<PluginResult> {
        self.query_inner(query, None)
    }

    fn query_with_cancel(&self, query: &str, cancel: &CancelToken) -> Vec<PluginResult> {
        self.query_inner(query, Some(cancel))
    }

    fn reset_search_session(&self) {
        // ① 先无条件标记 reset 请求。
        //    如果 try_lock 失败（background worker 正持锁搜索中），reset flag 会保留，
        //    等 worker 在 query_inner 内部或 finish_pending_reset 里检测到后执行实际 reset。
        self.reset_requested.store(true, Ordering::Release);
        if let Ok(mut session) = self.session.try_lock() {
            // try_lock 成功：锁空闲，可以立即 reset，顺便清掉 flag 避免重复 reset。
            session.reset();
            self.reset_requested.store(false, Ordering::Release);
        }
        // try_lock 失败时刻意不阻塞：UI 线程不能等 MFT 搜索完成。
        // flag 留给 worker 的 ② 或 ③ 处理。
    }

    fn name(&self) -> &str {
        "FileSearch"
    }
}

fn es_result_to_plugin(r: EsSearchResult) -> PluginResult {
    let full_path = r.path.clone();
    let parent_path = Path::new(&full_path)
        .parent()
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_default();
    let icon = if r.is_directory {
        String::from("folder")
    } else {
        full_path.clone() // GUI will extract icon from file extension/path
    };
    let context_actions = build_context_actions(&r.name, &full_path, r.is_directory, &parent_path);

    // Folders: Enter = navigate into (path search mode); Files: Enter = open
    let action = if r.is_directory {
        Action::EnterPathSearch(full_path.clone())
    } else {
        Action::Open(full_path.clone())
    };

    PluginResult {
        title: r.name,
        subtitle: r.path.clone(),
        icon,
        action,
        score: r.score,
        highlight: r.highlight,
        context_actions,
        context_data: Some(ContextData {
            is_directory: r.is_directory,
            file_path: r.path,
            parent_path,
        }),
    }
}

fn build_context_actions(
    title: &str,
    path: &str,
    is_directory: bool,
    parent_path: &str,
) -> Vec<ContextAction> {
    use easysearch_core::context_labels as cl;

    let is_saved = global_store()
        .lock()
        .map(|store| store.contains(path))
        .unwrap_or(false);

    let mut actions = Vec::new();

    if is_directory {
        // For folders: Enter = navigate into, so first context action is "Open in Explorer"
        actions.push(ContextAction {
            label: cl::open_item(true),
            action: Action::Open(path.to_string()),
            shortcut_hint: "Ctrl+Enter".to_string(),
        });
        actions.push(ContextAction {
            label: cl::open_containing_folder(true),
            action: Action::OpenParentFolder(path.to_string()),
            shortcut_hint: String::new(),
        });
    } else {
        // For files: Enter = open file (unchanged)
        actions.push(ContextAction {
            label: cl::open_item(false),
            action: Action::Open(path.to_string()),
            shortcut_hint: "Enter".to_string(),
        });
        actions.push(ContextAction {
            label: cl::open_containing_folder(false),
            action: Action::OpenContainingFolder(path.to_string()),
            shortcut_hint: "Ctrl+Enter".to_string(),
        });
        if !parent_path.is_empty() {
            actions.push(ContextAction {
                label: cl::open_parent_folder(),
                action: Action::OpenParentFolder(path.to_string()),
                shortcut_hint: String::new(),
            });
        }
    }

    actions.push(ContextAction {
        label: cl::copy_path(),
        action: Action::Copy(path.to_string()),
        shortcut_hint: String::new(),
    });
    actions.push(ContextAction {
        label: cl::copy_name(),
        action: Action::Copy(title.to_string()),
        shortcut_hint: String::new(),
    });
    actions.push(ContextAction {
        label: cl::toggle_quick_launch(is_saved),
        action: Action::ToggleQuickLaunch {
            path: path.to_string(),
            title: title.to_string(),
        },
        shortcut_hint: String::new(),
    });

    // "Search in folder" — only for files (folders already use Enter to navigate)
    if !is_directory && !parent_path.is_empty() {
        actions.push(ContextAction {
            label: cl::search_in_folder(),
            action: Action::EnterPathSearch(parent_path.to_string()),
            shortcut_hint: String::new(),
        });
    }

    actions.push(ContextAction {
        label: cl::windows_context_menu(),
        action: Action::ShowFileContextMenu {
            path: path.to_string(),
            is_dir: is_directory,
        },
        shortcut_hint: "Alt+Enter".to_string(),
    });

    actions
}
