# EasySearch Architecture Refactor Map

## Progress

### Search Responsiveness Work

- `Router` now owns one persistent background-query worker instead of spawning
  one thread per debounced query.
- Cancellation now propagates through `Plugin::query_with_cancel`,
  `FileSearchPlugin`, `SearchEngine`, `DriveManager`, and `EsIndex`. Long index
  scans check for obsolete queries every 1024 records.
- Input debounce is 100 ms. Search/icon progress animation starts only after the
  debounced background query begins and runs at roughly 30 FPS instead of 60 FPS.
- Icon extraction uses two long-lived workers instead of one OS thread per icon;
  stale icon completions populate the cache without repainting the current view.
- Periodic Program and Bookmark index refreshes run in background threads, so a
  timed refresh cannot block the Win32 input handler.
- Added core tests for pre-cancelled index searches and latest-query-wins worker
  behavior.

#### Candidate Search Session

- A 3+ character cold query still scans all logical MFT records, but now collects
  every matching logical record ID separately from the bounded Top-N results.
- Strict query extensions filter the previous complete candidate set. Backspace
  can reuse one of four retained prefix snapshots; middle edits and unrelated
  queries start a new cold-search branch.
- Candidate snapshots are grouped by drive and tagged with `DriveManager`
  generation. Drive install/removal and non-empty USN batches invalidate them
  before logical IDs can be reused against changed index state.
- Retained candidate IDs are capped at 32 MiB across at most four snapshots.
  When a cold scan exceeds the budget, results remain valid but the incomplete
  candidate set is explicitly marked uncacheable.
- Empty/1-2 character queries, path-search mode, window hide, and explicit
  router session reset release the cache. Resetting the file plugin never waits
  for an obsolete background scan on the UI thread.
- ASCII filename matching now compares bytes case-insensitively without a
  per-record lowercase allocation; Unicode keeps the existing folding fallback.

This preserves substring-search semantics without restoring the removed
~600 MiB trigram postings index.

### Completed Extractions

| Module | Extracted From | Responsibility |
|--------|--------------|----------------|
| `app_state.rs` | `window.rs` | `AppState` struct, `ViewMode`, `DeferredQuery`, thread-local storage, `with_app_mut` / `with_app_ref` / `init` access helpers |
| `messages.rs` | `window.rs` | Win32 custom message IDs, timer IDs, engine event payloads, icon payload structs |
| `plugin_bridge.rs` | `window.rs` | Plugin router construction, PluginResult→DisplayItem conversion, home screen building, history key mapping |
| `engine_bridge.rs` | `window.rs` | SearchEngine initialization from settings, background event thread spawning and PostMessageW forwarding |
| `search_flow.rs` | `window.rs` | Input change handling, debounced search dispatch, deferred result merging/dedup/pinning, window resize |
| `settings_sync.rs` | `window.rs` | Settings diff application (theme, language, hotkey, drives, autostart), hotkey string parsing |
| `execution.rs` | `window.rs` | Item activation: `execute_selected_safe`, `open_folder_or_containing_safe`, `execute_action_safe` — extracts action under a short borrow, records history, hides window, and dispatches (open / quick-launch toggle / path search / native context menu) outside the borrow to avoid RefCell re-entrancy |
| `render_bridge.rs` | `window.rs` | Render orchestration: prepares renderer parameters from `AppState`, computes placeholder/preview state, schedules icon-loading animation timers, and posts async icon load completions back to the window thread |
| `key_command.rs` | `window.rs` | `KeyCommand` enum, `decode_key_command` (VK → command), `execute_key_command` (immediate state mutation + `DeferredAction` for Win32 calls outside borrow) — split `handle_keydown`'s 200-line decode+execute into pure functions |
| `clipboard.rs` | `window.rs` | `get_text` / `set_text` — clipboard access wrappers used by `key_command` module |

### `window.rs` Reduction

- Started at ~2821 lines
- Now at ~1125 lines (~1700 lines removed, **~60% reduction**)
- `window.rs` no longer defines state types, owns the thread-local, holds action-dispatch logic, or orchestrates rendering — it is now closer to pure message dispatch and Win32 coordination
- `open_context_actions`, `show_native_context_menu_safe`, and `sync_active_items` are now `pub(super)` so `execution.rs` can reuse them

### Test Coverage Added

To guard the core logic that the extracted execution path depends on:

- `search/history.rs`: 8 unit tests covering `record`/`count`, `boost_score` bucketing, `record_full` dedup + newest-first ordering, `MAX_RECENT` capacity bounding, pin/unpin (case-insensitive query), and serde round-trip / partial-JSON deserialization.
- `search/plugin_bridge.rs`: 8 windows-gated unit tests covering `action_to_history_key_static` encoding for all action variants (including `Copy` truncation) and the `history_key_to_action` inverse round-trip for the open/admin/folder/parent family.

Full app suite: **55 tests passing** (`cargo test -p easysearch --release`).

### Build Fix

Enabled the `bundled` feature on `rusqlite` in `plugin-bookmark/Cargo.toml`. Previously the link step failed with `LNK1181: cannot open sqlite3.lib` because no system SQLite import library was present. The `bundled` feature compiles SQLite from source via `cc`, removing the external-lib dependency and unblocking both `cargo build` and `cargo test`.

### Key Architectural Change: `app_state.rs`

All modules now access application state through `app_state::with_app_mut(|app| { ... })` and `app_state::with_app_ref(|app| { ... })` instead of raw `APP_STATE.with(|state| { ... try_borrow ... })`. This:
- Eliminates boilerplate (`try_borrow_mut` + `Option` unwrap) at every call site
- Makes it possible to move any function that operates on `&mut AppState` to any sibling module
- Centralizes borrow-failure handling in one place

### Visibility Changes

- `AppState`, `ViewMode`, `DeferredQuery` live in `app_state.rs` as `pub(super)`
- `sync_active_items` remains `pub(super)` in `window.rs` for use by `search_flow.rs`
- `window.rs` no longer imports `RefCell` or defines any type — it only does Win32 coordination

### What's Next (Phase 3)

- ✅ Extract visibility logic (done — `visibility.rs`)
- ✅ Extract `execute_action_safe` action dispatch into its own module (done — `execution.rs`)
- ✅ Move `do_render` orchestration to a `render_bridge.rs` (done — `render_bridge.rs`)
- ✅ Extract keyboard handling into `key_command.rs` (done — `key_command.rs`)
- ✅ Extract clipboard helpers into `clipboard.rs` (done — `clipboard.rs`)
- Split `wnd_proc` message routing into `message_loop.rs`
- Create app_state directory structure (`navigation.rs`, `resize.rs`)
- Create `platform/ime.rs` + `platform/window_actions.rs`
- Introduce view model structs to decouple renderer from `AppState` internals

## Why This Refactor Exists

The main architectural problem in this repository is not compilation failure. The real issue is that the app layer has become a coordination monolith:

- `crates/app/src/search/window.rs` owns Win32 lifecycle, message dispatch, search orchestration, plugin wiring, async event bridging, timers, selection state, preview state, and part of settings refresh.
- `crates/app/src/search/renderer.rs` is also large and tightly coupled to UI state shape.
- Plugin registration is hardcoded in the app layer, so "plugins" are only partially decoupled.

This makes feature work slower, testing harder, and regressions more likely.

## Current Hotspot

```text
window.rs
  |- create window
  |- register hotkey
  |- tray integration
  |- create renderer
  |- start search engine
  |- listen for engine events
  |- build plugin router
  |- own search state
  |- own context menu state
  |- own preview state
  |- own timers
  |- own input behavior
  |- own Win32 message handling
  |- own result merging
```

This is the primary source of architectural friction.

## Target Shape

```text
crates/app/src/search/
  mod.rs
  window/
    mod.rs                -> public entry, window bootstrap only
    class.rs              -> class registration and hwnd creation
    message_loop.rs       -> top-level wnd_proc routing
    messages.rs           -> custom message ids and payloads
  app_state/
    mod.rs                -> AppState definition and small helpers
    navigation.rs         -> selection / mode transitions
    visibility.rs         -> show/hide/focus behavior
    resize.rs             -> layout-driven size changes
  orchestration/
    mod.rs
    search_flow.rs        -> on_input_changed / debounce / deferred merge
    engine_bridge.rs      -> SearchEngine startup + event forwarding
    plugin_bridge.rs      -> router creation + plugin result mapping
    preview_flow.rs       -> preview async loading and staleness checks
    settings_sync.rs      -> polling and runtime reload
  ui/
    renderer.rs           -> drawing only
    layout.rs             -> geometry only
    icon.rs               -> icon loading/cache only
    preview.rs            -> preview model only
  platform/
    hotkey.rs             -> already shared, keep thin
    tray.rs               -> already shared, keep thin
    shell_context_menu.rs -> native shell menu integration
    ime.rs                -> IME-specific behavior
    window_actions.rs     -> foreground/focus/show helpers
```

## Dependency Direction

Use this dependency rule during refactor:

```text
platform -> orchestration -> app_state -> ui
platform -> ui
window bootstrap -> orchestration

Never:
ui -> orchestration
ui -> platform side effects
renderer -> SearchEngine
renderer -> Router
plugin implementation -> app window state
```

In plain words:

- Rendering code may read prepared view data, but should not decide business flow.
- Win32 message code may translate events, but should not contain full search logic.
- Plugin composition should be isolated from window creation.
- Engine startup and engine event forwarding should be isolated from drawing code.

## Refactor Phases

### Phase 1: Stop Further Growth

Goal: no new behavior goes directly into `window.rs` unless it is pure message dispatch glue.

Rules:

- New search behavior goes into `orchestration/search_flow.rs`.
- New engine-related behavior goes into `orchestration/engine_bridge.rs`.
- New plugin registration or plugin-to-display mapping goes into `orchestration/plugin_bridge.rs`.
- New window-state-only helpers go into `app_state/*`.

This phase can happen without changing behavior.

### Phase 2: Extract Stable Seams

Extract in this order:

1. Custom message constants and payload structs.
2. Router construction and plugin result mapping.
3. Debounced search flow and deferred query polling.
4. Engine event thread startup and `PostMessageW` bridge.
5. Visibility and resize helpers.

Reason: these areas have high code volume and comparatively clear boundaries.

### Phase 3: Split State Mutation from Message Dispatch

Create message handlers that only do one of these:

- decode raw Win32 input
- call a state transition helper
- request a render

Avoid handlers that both decode Win32 state and also perform multi-step search orchestration inline.

### Phase 4: Introduce View Models

Before rendering, normalize active app state into small display/view structs.

Examples:

- `WindowViewModel`
- `ResultsViewModel`
- `ContextActionsViewModel`
- `StatusBannerViewModel`

This helps keep renderer changes local and testable.

### Phase 5: Reduce Hardcoded Plugin Wiring

Keep built-in plugins if needed, but move composition into one isolated place:

- one file owns registration order
- one file owns settings-driven enable/disable
- one file owns plugin metadata refresh

The window layer should only consume a ready router/service.

## File Ownership Guidelines

Use these rules when choosing where code belongs.

### `window/*`

Belongs here:

- window class registration
- hwnd creation
- Win32 callback entrypoints
- top-level message routing

Does not belong here:

- search ranking decisions
- plugin composition
- deferred search orchestration
- preview loading policy

### `app_state/*`

Belongs here:

- selected index updates
- mode switching between results and context actions
- cached derived flags
- state reset helpers

Does not belong here:

- Win32 calls except very small wrappers
- engine startup
- plugin queries

### `orchestration/*`

Belongs here:

- when a search starts
- when old async work is cancelled
- how immediate and deferred results merge
- how engine events mutate app state
- how settings refresh rebuilds plugin/router state

This layer is the "brain" of the app.

### `ui/*`

Belongs here:

- rendering
- icon decode/cache
- layout math
- preview presentation data

Does not belong here:

- thread spawning
- timers
- plugin dispatch
- engine access

## Rules For Future Contributors

If you are adding a feature, ask:

1. Is this Win32 plumbing?
2. Is this app state transition logic?
3. Is this orchestration logic?
4. Is this rendering or layout only?

If the answer is "a bit of all four", split the work before adding code.

## Definition Of Success

The refactor is working when these become true:

- `window.rs` becomes a thin entry layer instead of the system brain.
- Search flow can be read without reading Win32 boilerplate.
- Plugin composition can change without touching rendering code.
- Renderer can evolve without touching engine startup code.
- New contributors can identify a file destination in under one minute.

## What Not To Do

- Do not rewrite the whole app in one pass.
- Do not mix visual cleanup with architecture extraction in the same step unless necessary.
- Do not move code into new files while preserving the same hidden coupling blindly.
- Do not add more responsibilities to `window.rs` just because it is already large.
