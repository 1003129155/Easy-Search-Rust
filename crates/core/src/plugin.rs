// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Plugin system core types.
//!
//! Defines the [`Plugin`] trait and common types shared by all plugins.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// A single result item produced by a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginResult {
    /// Display title (primary text).
    pub title: String,
    /// Subtitle / secondary text.
    pub subtitle: String,
    /// Icon identifier (path or built-in name).
    pub icon: String,
    /// Action to perform when the user selects this result.
    pub action: Action,
    /// Relevance score (higher = better match).
    pub score: u32,
    /// Highlight ranges as byte offsets `[start, len]` in `title`.
    #[serde(default)]
    pub highlight: Vec<[u32; 2]>,
    /// Secondary actions available from the context actions page.
    #[serde(default)]
    pub context_actions: Vec<ContextAction>,
    /// Optional metadata for building richer context actions in the app.
    #[serde(default)]
    pub context_data: Option<ContextData>,
}

/// What happens when a result is executed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    /// Open a URL or file path via the system shell.
    Open(String),
    /// Open the target's containing folder, selecting it when appropriate.
    OpenContainingFolder(String),
    /// Open the direct parent folder of the target.
    OpenParentFolder(String),
    /// Replace the current query with a filesystem path search.
    EnterPathSearch(String),
    /// Copy text to the clipboard.
    Copy(String),
    /// Run a shell command.
    RunCommand { cmd: String, keep_open: bool },
    /// Send a search query to the daemon (file search).
    DaemonSearch(String),
    /// Execute a system command (shutdown, lock, etc.).
    SystemCommand(SystemCmd),
    /// Add or remove an item from Quick Launch.
    ToggleQuickLaunch { path: String, title: String },
    /// Show the native Windows context menu for a file or folder.
    ShowFileContextMenu { path: String, is_dir: bool },
    /// No action (informational result).
    None,
}

/// Secondary action entry shown in the context actions page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextAction {
    pub label: String,
    pub action: Action,
    #[serde(default)]
    pub shortcut_hint: String,
}

/// Metadata attached to a result so the app can build richer actions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextData {
    pub is_directory: bool,
    pub file_path: String,
    pub parent_path: String,
}

/// System-level commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemCmd {
    Shutdown,
    Restart,
    Lock,
    Sleep,
    Hibernate,
    Logout,
    EmptyRecycleBin,
}

/// The plugin trait. Each built-in plugin implements this.
pub trait Plugin: Send + Sync {
    /// Default keyword prefix that triggers this plugin (e.g. "> ", "kill ").
    /// Return `None` for plugins that match based on content (calculator, URL).
    /// This is the compile-time default; runtime keyword is managed by Router.
    fn default_keyword(&self) -> Option<&str>;

    /// Whether this plugin wants to handle the given raw query.
    /// Called only for plugins with no keyword (global match),
    /// or for keyword plugins that also return `true` from `also_global()`.
    fn matches(&self, query: &str) -> bool {
        let _ = query;
        false
    }

    /// Whether this keyword plugin also participates in global (non-keyword) queries.
    /// When true, the plugin's `matches()` and `query()` will be called even when
    /// no keyword prefix is present. Default is `false`.
    fn also_global(&self) -> bool {
        false
    }

    /// Produce results for the given query.
    /// For keyword-triggered plugins, `query` has the keyword stripped.
    ///
    /// For plugins that return `true` from `needs_background()`, this method
    /// will be called from a background thread, not the UI thread.
    fn query(&self, query: &str) -> Vec<PluginResult>;

    /// Human-readable name of this plugin.
    fn name(&self) -> &str;

    /// Localized display name shown in UI surfaces.
    fn display_name(&self, _locale: &str) -> String {
        self.name().to_string()
    }

    /// Plugin description shown in the settings panel.
    fn description(&self) -> &str {
        ""
    }

    /// Localized plugin description shown in UI surfaces.
    fn description_for_locale(&self, _locale: &str) -> String {
        self.description().to_string()
    }

    /// Plugin icon identifier.
    fn icon(&self) -> &str {
        "plugin"
    }

    /// Whether this plugin's `query()` may block and should be run on a
    /// background thread. Default is `false` for fast in-memory plugins.
    ///
    /// Set to `true` for plugins that do I/O-heavy work (e.g. file search
    /// against an MFT index, network requests, etc.).
    fn needs_background(&self) -> bool {
        false
    }

    /// Settings schema — defines what options this plugin exposes in the settings UI.
    /// Return `None` if the plugin has no configurable settings.
    fn settings_schema(&self) -> Option<Vec<SettingItem>> {
        None
    }

    /// Localized settings schema for the settings UI.
    fn settings_schema_for_locale(&self, _locale: &str) -> Option<Vec<SettingItem>> {
        self.settings_schema()
    }

    /// Plugin priority for result ranking. Higher priority plugins get their
    /// scores boosted by `priority * 150` (same formula as FlowLauncher).
    /// Default is 0 (no boost). Range: -10 to 10 recommended.
    fn priority(&self) -> i32 {
        0
    }

    /// Called when a setting value changes from the UI.
    /// `key` is the `SettingItem.key`, `value` is the new JSON-serialized value.
    fn on_setting_changed(&mut self, _key: &str, _value: &str) {
        // default: no-op
    }

    /// Get current setting values as key-value pairs (for populating the UI).
    /// Values are JSON-serialized strings.
    fn setting_values(&self) -> Vec<(String, String)> {
        Vec::new()
    }
}

// ─── Settings Schema Types ───────────────────────────────────────────────────

/// A single setting item that the plugin exposes to the settings UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingItem {
    /// Unique key for this setting (e.g. "shell_type", "leave_open").
    pub key: String,
    /// Display label in the UI.
    pub label: String,
    /// Optional description / help text shown below the control.
    pub description: String,
    /// The type of UI control to render.
    pub control: SettingControl,
}

/// What kind of UI control to use for a setting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SettingControl {
    /// On/off toggle switch.
    Toggle { default: bool },
    /// Dropdown with a list of options. `options` is a list of (value, display_label).
    Dropdown {
        options: Vec<(String, String)>,
        default: String,
    },
    /// Text input field.
    TextInput {
        placeholder: String,
        default: String,
    },
    /// Numeric spinner (integer).
    Number { min: i64, max: i64, default: i64 },
}

/// Router dispatches queries to the appropriate plugin(s).
pub struct Router {
    plugins: Vec<PluginSlot>,
}

/// Cancellation token for background plugin queries.
/// Set to `true` to signal the background thread to abort early.
pub type CancelToken = Arc<AtomicBool>;

/// A registered plugin with its runtime-configurable keyword.
struct PluginSlot {
    plugin: Arc<dyn Plugin>,
    /// Runtime keyword override. `None` means use `plugin.default_keyword()`.
    keyword_override: Option<Option<String>>,
    /// Whether this plugin is enabled.
    enabled: bool,
}

impl PluginSlot {
    /// Get the effective keyword for routing.
    fn effective_keyword(&self) -> Option<&str> {
        match &self.keyword_override {
            Some(ovr) => ovr.as_deref(),
            None => self.plugin.default_keyword(),
        }
    }
}

impl Router {
    /// Create a new empty router.
    #[must_use]
    pub fn new() -> Self {
        Self { plugins: Vec::new() }
    }

    /// Register a plugin with the router (uses default keyword).
    pub fn register(&mut self, plugin: Box<dyn Plugin>) {
        self.plugins.push(PluginSlot {
            plugin: Arc::from(plugin),
            keyword_override: None,
            enabled: true,
        });
    }

    /// Set a custom keyword for a plugin by name.
    /// Pass `Some("kw ")` to set a keyword, or `None` to make it a global plugin.
    pub fn set_keyword(&mut self, plugin_name: &str, keyword: Option<String>) {
        if let Some(slot) = self.plugins.iter_mut().find(|s| s.plugin.name() == plugin_name) {
            slot.keyword_override = Some(keyword);
        }
    }

    /// Enable or disable a plugin by name.
    pub fn set_enabled(&mut self, plugin_name: &str, enabled: bool) {
        if let Some(slot) = self.plugins.iter_mut().find(|s| s.plugin.name() == plugin_name) {
            slot.enabled = enabled;
        }
    }

    /// Get a mutable reference to a plugin by name (for settings changes).
    /// Only succeeds when the plugin is uniquely owned (no other Arc references).
    pub fn plugin_mut(&mut self, name: &str) -> Option<&mut dyn Plugin> {
        for slot in &mut self.plugins {
            if slot.plugin.name() == name {
                // SAFETY: Arc::get_mut returns a reference tied to the Arc's
                // lifetime. Since we have &mut self, we know no other references
                // to the plugin exist. The 'static bound is satisfied because
                // the Arc keeps the plugin alive.
                return Arc::get_mut(&mut slot.plugin).map(|p| p as &mut dyn Plugin);
            }
        }
        None
    }

    /// Get plugin metadata for the settings UI.
    pub fn plugin_infos(&self) -> Vec<PluginRouterInfo> {
        self.plugin_infos_for_locale("en")
    }

    /// Get plugin metadata for the settings UI in a target locale.
    pub fn plugin_infos_for_locale(&self, locale: &str) -> Vec<PluginRouterInfo> {
        self.plugins
            .iter()
            .map(|slot| PluginRouterInfo {
                id: slot.plugin.name().to_string(),
                name: slot.plugin.display_name(locale),
                description: slot.plugin.description_for_locale(locale),
                icon: slot.plugin.icon().to_string(),
                keyword: slot.effective_keyword().map(|s| s.to_string()),
                default_keyword: slot.plugin.default_keyword().map(|s| s.to_string()),
                enabled: slot.enabled,
            })
            .collect()
    }

    /// Route a query to matching plugins and collect results from fast
    /// (non-background) plugins. Background plugins are skipped — use
    /// [`query_background`](Self::query_background) for those.
    ///
    /// Returns `(results, keyword_matched)` — the second element indicates
    /// whether a keyword-triggered plugin claimed this query. Pass it to
    /// [`query_background`] so background global plugins are suppressed when
    /// an immediate keyword plugin already handled the input.
    pub fn query_immediate(&self, raw_query: &str) -> (Vec<PluginResult>, bool) {
        let mut results = Vec::new();
        let query_trimmed = raw_query.trim();

        if query_trimmed.is_empty() {
            return (results, false);
        }

        let mut keyword_matched = false;

        for slot in &self.plugins {
            if !slot.enabled || slot.plugin.needs_background() {
                continue;
            }

            if let Some(kw) = slot.effective_keyword() {
                let kw_trimmed = kw.trim_end();
                let query_after_kw = if kw.ends_with(' ') {
                    query_trimmed.strip_prefix(kw)
                } else if query_trimmed == kw_trimmed {
                    Some("")
                } else if let Some(rest) = query_trimmed.strip_prefix(kw_trimmed) {
                    if rest.starts_with(' ') {
                        Some(rest)
                    } else {
                        None
                    }
                } else {
                    None
                };

                if let Some(stripped) = query_after_kw {
                    let stripped = stripped.trim_start();
                    let priority_boost = (slot.plugin.priority().max(0) as u32) * 150;
                    let mut plugin_results = slot.plugin.query(stripped);
                    for r in &mut plugin_results {
                        r.score = r.score.saturating_add(priority_boost);
                    }
                    results.extend(plugin_results);
                    keyword_matched = true;
                }
            }
        }

        // Global plugins only participate when no keyword plugin matched.
        if !keyword_matched {
            for slot in &self.plugins {
                if !slot.enabled || slot.plugin.needs_background() {
                    continue;
                }
                if slot.effective_keyword().is_some() && !slot.plugin.also_global() {
                    continue;
                }
                let matches = if slot.effective_keyword().is_some() && slot.plugin.also_global() {
                    slot.plugin.matches(query_trimmed)
                } else if slot.effective_keyword().is_none() {
                    slot.plugin.matches(query_trimmed)
                } else {
                    false
                };
                if matches {
                    let priority_boost = (slot.plugin.priority().max(0) as u32) * 150;
                    let mut plugin_results = slot.plugin.query(query_trimmed);
                    for r in &mut plugin_results {
                        r.score = r.score.saturating_add(priority_boost);
                    }
                    results.extend(plugin_results);
                }
            }
        }

        results.sort_by(|a, b| b.score.cmp(&a.score));
        (results, keyword_matched)
    }

    /// Run background (expensive) plugins on a separate thread and return
    /// results via a channel along with a cancellation token.
    ///
    /// The channel will receive one batch of all background plugin results,
    /// already sorted by score descending. Set the cancel token to `true`
    /// to abort the thread early (it checks between plugins).
    ///
    /// Returns `None` if there are no matching background plugins for this query.
    ///
    /// When `keyword_matched_immediate` is `true`, global (non-keyword)
    /// background plugins are suppressed — only background plugins whose
    /// keyword matches the query will run.
    pub fn query_background(
        &self,
        raw_query: &str,
        keyword_matched_immediate: bool,
    ) -> Option<(std::sync::mpsc::Receiver<Vec<PluginResult>>, CancelToken)> {
        let query_trimmed = raw_query.trim().to_string();
        if query_trimmed.is_empty() {
            return None;
        }

        // Collect matching background plugins and their stripped queries
        let mut tasks: Vec<(String, Arc<dyn Plugin>)> = Vec::new();
        let mut keyword_matched = false;

        for slot in &self.plugins {
            if !slot.enabled || !slot.plugin.needs_background() {
                continue;
            }

            if let Some(kw) = slot.effective_keyword() {
                let kw_trimmed = kw.trim_end();
                let query_after_kw = if kw.ends_with(' ') {
                    query_trimmed.strip_prefix(kw)
                } else if query_trimmed == kw_trimmed {
                    Some("")
                } else if let Some(rest) = query_trimmed.strip_prefix(kw_trimmed) {
                    if rest.starts_with(' ') {
                        Some(rest)
                    } else {
                        None
                    }
                } else {
                    None
                };

                if let Some(stripped) = query_after_kw {
                    tasks.push((stripped.trim_start().to_string(), Arc::clone(&slot.plugin)));
                    keyword_matched = true;
                }
            }
        }

        // Skip global background plugins when an immediate keyword plugin
        // already claimed the query (prevents file-search from overwriting
        // plugin results like quick-launch, bookmark, etc.)
        if !keyword_matched && !keyword_matched_immediate {
            for slot in &self.plugins {
                if !slot.enabled || !slot.plugin.needs_background() {
                    continue;
                }
                if slot.effective_keyword().is_some() && !slot.plugin.also_global() {
                    continue;
                }
                let matches = if slot.effective_keyword().is_some() && slot.plugin.also_global() {
                    slot.plugin.matches(&query_trimmed)
                } else if slot.effective_keyword().is_none() {
                    slot.plugin.matches(&query_trimmed)
                } else {
                    false
                };
                if matches {
                    tasks.push((query_trimmed.clone(), Arc::clone(&slot.plugin)));
                }
            }
        }

        if tasks.is_empty() {
            return None;
        }

        let (tx, rx) = std::sync::mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_clone = Arc::clone(&cancel);

        std::thread::Builder::new()
            .name("plugin-background".into())
            .spawn(move || {
                let mut all_results = Vec::new();
                for (stripped_query, plugin) in tasks {
                    // Check cancellation before each plugin invocation
                    if cancel_clone.load(Ordering::Relaxed) {
                        return; // cancelled — drop without sending
                    }
                    all_results.extend(plugin.query(&stripped_query));
                }
                // Check one more time before the final sort + send
                if !cancel_clone.load(Ordering::Relaxed) {
                    all_results.sort_by(|a, b| b.score.cmp(&a.score));
                    let _ = tx.send(all_results);
                }
            })
            .ok();

        Some((rx, cancel))
    }

    /// Route a query to matching plugins and collect results (compatibility
    /// wrapper — calls both immediate and background synchronously).
    /// Prefer [`query_immediate`] + [`query_background`] for new code.
    pub fn query(&self, raw_query: &str) -> Vec<PluginResult> {
        let (mut results, immediate_keyword_matched) = self.query_immediate(raw_query);

        // Run background plugins inline (blocking — only for non-UI use)
        let query_trimmed = raw_query.trim().to_string();
        if query_trimmed.is_empty() {
            return results;
        }

        let mut tasks: Vec<(String, &dyn Plugin)> = Vec::new();
        let mut keyword_matched = false;

        for slot in &self.plugins {
            if !slot.enabled || !slot.plugin.needs_background() {
                continue;
            }

            if let Some(kw) = slot.effective_keyword() {
                let kw_trimmed = kw.trim_end();
                let query_after_kw = if kw.ends_with(' ') {
                    query_trimmed.strip_prefix(kw)
                } else if query_trimmed == kw_trimmed {
                    Some("")
                } else if let Some(rest) = query_trimmed.strip_prefix(kw_trimmed) {
                    if rest.starts_with(' ') {
                        Some(rest)
                    } else {
                        None
                    }
                } else {
                    None
                };

                if let Some(stripped) = query_after_kw {
                    tasks.push((stripped.trim_start().to_string(), slot.plugin.as_ref()));
                    keyword_matched = true;
                }
            }
        }

        if !keyword_matched && !immediate_keyword_matched {
            for slot in &self.plugins {
                if !slot.enabled || !slot.plugin.needs_background() {
                    continue;
                }
                if slot.effective_keyword().is_some() && !slot.plugin.also_global() {
                    continue;
                }
                let matches = if slot.effective_keyword().is_some() && slot.plugin.also_global() {
                    slot.plugin.matches(&query_trimmed)
                } else if slot.effective_keyword().is_none() {
                    slot.plugin.matches(&query_trimmed)
                } else {
                    false
                };
                if matches {
                    tasks.push((query_trimmed.clone(), slot.plugin.as_ref()));
                }
            }
        }

        for (stripped_query, plugin) in tasks {
            results.extend(plugin.query(&stripped_query));
        }

        results.sort_by(|a, b| b.score.cmp(&a.score));
        results
    }
}

/// Info about a registered plugin (for the settings UI).
#[derive(Debug, Clone)]
pub struct PluginRouterInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    /// Current effective keyword (None = global match).
    pub keyword: Option<String>,
    /// The plugin's built-in default keyword.
    pub default_keyword: Option<String>,
    /// Whether the plugin is enabled.
    pub enabled: bool,
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}
