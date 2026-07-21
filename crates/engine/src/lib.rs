// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! EasySearch Engine — unified in-process search engine.
//!
//! This crate provides a thread-safe search engine that can be embedded directly
//! in the GUI process or used by the daemon. It manages MFT index building, USN
//! journal polling, and provides a rich search interface.
//!
//! # Features
//!
//! - **Event-driven**: Consumers receive [`EngineEvent`]s for progress/status.
//! - **Rich queries**: [`SearchQuery`] supports filters, sort orders, and
//!   Everything-compatible pattern normalization.
//! - **Hot-plug drives**: Add/remove drives at runtime without restart.
//! - **Metrics**: Built-in latency/throughput tracking for diagnostics.
//! - **Graceful shutdown**: Signal background threads to exit cleanly.
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────────────┐
//! │              SearchEngine (public API)            │
//! │  search() / enumerate() / add_drive() / status() │
//! └────────────────────────┬─────────────────────────┘
//!                          │ Arc<RwLock<DriveManager>>
//! ┌────────────────────────▼─────────────────────────┐
//! │              Background threads                   │
//! │  • Index builder (per-drive MFT read)            │
//! │  • USN poller (1s interval, incremental)         │
//! │  • Hot-plug listener (command channel)           │
//! └──────────────────────────────────────────────────┘
//! ```

pub mod config;
pub mod drive_manager;
pub mod event;
pub mod metrics;
pub mod query;
mod search_session;
pub mod usn_source;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::{Duration, Instant};

pub use config::EngineConfig;
pub use drive_manager::{DriveManager, build_index};
pub use easysearch_core::EsSearchResult;
pub use event::{EngineEvent, EventReceiver, EventSender, event_channel};
pub use metrics::{EngineMetrics, MetricsSnapshot};
pub use query::{SearchFilter, SearchQuery, SortOrder, normalize_query};
pub use search_session::{SearchSession, SearchSessionMode};

/// Commands sent to the background worker for hot-plug drive management.
#[allow(dead_code)]
enum WorkerCommand {
    /// Add a new drive to index.
    AddDrive(char),
    /// Remove a drive's index.
    RemoveDrive(char),
    /// Force rebuild a drive's index.
    RebuildDrive(char),
    /// Shut down the background worker.
    Shutdown,
}

/// Per-drive indexing status visible to consumers.
#[derive(Debug, Clone)]
pub struct DriveStatus {
    /// Drive letter (uppercase).
    pub drive: char,
    /// Current state.
    pub state: DriveState,
    /// Number of records in the index (0 if not ready).
    pub records: u64,
    /// How long the last build took.
    pub build_time: Option<Duration>,
}

/// State of a single drive's index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriveState {
    /// Index is being built.
    Indexing,
    /// Index is ready and being kept up to date via USN.
    Ready,
    /// Index build failed.
    Error,
}

/// Overall engine status snapshot.
#[derive(Debug, Clone)]
pub struct EngineStatus {
    /// Whether all initial drives are indexed.
    pub ready: bool,
    /// Per-drive status.
    pub drives: Vec<DriveStatus>,
    /// Total records across all drives.
    pub total_records: u64,
    /// Engine metrics snapshot.
    pub metrics: MetricsSnapshot,
}

/// Thread-safe search engine that manages indexes across multiple drives.
///
/// The engine runs index building and USN polling in background threads,
/// while exposing a lock-free (read-lock only) search interface.
pub struct SearchEngine {
    inner: Arc<RwLock<DriveManager>>,
    ready: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
    config: EngineConfig,
    metrics: Arc<EngineMetrics>,
    /// Global gate: at most one drive may compact at a time.
    compact_in_flight: Arc<AtomicBool>,
    /// Channel to send commands to the background worker.
    command_tx: std::sync::mpsc::Sender<WorkerCommand>,
    /// Event sender (cloned to background threads).
    event_tx: Option<EventSender>,
}

impl SearchEngine {
    /// Create a new engine instance with an event channel.
    ///
    /// Does not block — indexes are built in the background after calling
    /// [`start_background`](Self::start_background).
    #[must_use]
    pub fn new(config: EngineConfig, event_tx: Option<EventSender>) -> Self {
        let (command_tx, _) = std::sync::mpsc::channel();
        Self {
            inner: Arc::new(RwLock::new(DriveManager::new())),
            ready: Arc::new(AtomicBool::new(false)),
            shutdown: Arc::new(AtomicBool::new(false)),
            config,
            metrics: Arc::new(EngineMetrics::new()),
            compact_in_flight: Arc::new(AtomicBool::new(false)),
            command_tx,
            event_tx,
        }
    }

    /// Create a new engine with a fresh event channel, returning both the
    /// engine and the event receiver.
    #[must_use]
    pub fn with_events(config: EngineConfig) -> (Self, EventReceiver) {
        let (tx, rx) = event_channel();
        (Self::new(config, Some(tx)), rx)
    }

    /// Start background index building and USN polling threads.
    ///
    /// This spawns worker threads and returns immediately.
    pub fn start_background(&self) {
        let inner = Arc::clone(&self.inner);
        let ready = Arc::clone(&self.ready);
        let shutdown = Arc::clone(&self.shutdown);
        let config = self.config.clone();
        let metrics = Arc::clone(&self.metrics);
        let compact_in_flight = Arc::clone(&self.compact_in_flight);
        let event_tx = self.event_tx.clone();

        thread::Builder::new()
            .name("engine-index".to_string())
            .spawn(move || {
                // Phase 1: Build indexes for all configured drives
                for &drive_letter in &config.auto_index_drives {
                    if shutdown.load(Ordering::Acquire) {
                        break;
                    }

                    emit(&event_tx, EngineEvent::DriveIndexing { drive: drive_letter });

                    let start = Instant::now();
                    match build_index(drive_letter, config.cache_dir.as_deref()) {
                        Ok(index) => {
                            let elapsed = start.elapsed();
                            let record_count = index.records_len() as u64;
                            if let Ok(mut mgr) = inner.write() {
                                mgr.install(drive_letter, index);
                            }
                            metrics.record_build(drive_letter, elapsed);
                            emit(
                                &event_tx,
                                EngineEvent::DriveReady {
                                    drive: drive_letter,
                                    records: record_count,
                                    elapsed,
                                },
                            );
                        }
                        Err(err) => {
                            emit(
                                &event_tx,
                                EngineEvent::DriveError {
                                    drive: drive_letter,
                                    error: err.clone(),
                                },
                            );
                            easysearch_core::log_error!(
                                "{drive_letter}: index build failed: {err}"
                            );
                        }
                    }
                }

                // Mark ready
                ready.store(true, Ordering::Release);
                emit(&event_tx, EngineEvent::AllReady);

                // Phase 2: USN journal polling loop
                emit_log(&event_tx, easysearch_core::logging::LogLevel::Info, format!(
                    "[easysearch-engine] USN polling started (interval=1s, drives={:?})",
                    config.auto_index_drives
                ));

                // Track consecutive failures per drive to avoid log spam
                let mut fail_counts: std::collections::HashMap<char, u32> = std::collections::HashMap::new();
                let mut poll_cycle: u64 = 0;

                while !shutdown.load(Ordering::Acquire) {
                    thread::sleep(Duration::from_secs(1));
                    poll_cycle += 1;

                    // Log a heartbeat every 60 cycles (≈1 minute) so users know polling is alive
                    if poll_cycle % 60 == 0 {
                        let total_records = inner.read().map(|mgr| mgr.record_count()).unwrap_or(0);
                        emit_log(&event_tx, easysearch_core::logging::LogLevel::Debug, format!(
                            "[easysearch-engine] USN poll heartbeat: cycle={}, total_records={}",
                            poll_cycle, total_records
                        ));
                    }

                    let drives: Vec<char> = config.auto_index_drives.clone();
                    for drive_letter in drives {
                        if shutdown.load(Ordering::Acquire) {
                            break;
                        }

                        let cursor = inner
                            .read()
                            .ok()
                            .and_then(|mgr| mgr.cursor(drive_letter));

                        let Some((_journal_id, last_usn)) = cursor else {
                            // Only log once per drive when cursor is missing
                            let count = fail_counts.entry(drive_letter).or_insert(0);
                            if *count == 0 {
                                emit_log(&event_tx, easysearch_core::logging::LogLevel::Warn, format!(
                                    "[easysearch-engine] {drive_letter}: no cursor available (index not loaded?), skipping poll"
                                ));
                            }
                            *count += 1;
                            continue;
                        };

                        match usn_source::poll_drive(drive_letter, last_usn) {
                            Ok(poll) => {
                                // Reset failure counter on success
                                fail_counts.remove(&drive_letter);

                                let event_count = poll.events.len();
                                let cursor_advanced = poll.new_last_usn != last_usn;

                                if event_count > 0 {
                                    emit_log(&event_tx, easysearch_core::logging::LogLevel::Debug, format!(
                                        "[easysearch-engine] {drive_letter}: USN poll found {} events (usn {} -> {})",
                                        event_count, last_usn, poll.new_last_usn
                                    ));
                                    if let Ok(mut mgr) = inner.write() {
                                        mgr.apply(
                                            drive_letter,
                                            &poll.events,
                                            poll.new_last_usn,
                                            poll.journal_id,
                                        );
                                    }
                                    schedule_compact(
                                        &inner,
                                        &compact_in_flight,
                                        config.cache_dir.clone(),
                                    );
                                    metrics.record_usn_events(event_count);
                                    emit(
                                        &event_tx,
                                        EngineEvent::UsnUpdate {
                                            drive: drive_letter,
                                            events_applied: event_count,
                                        },
                                    );
                                } else if cursor_advanced {
                                    // Cursor moved but no meaningful events (e.g. only
                                    // close/security-change records that we don't track)
                                    if let Ok(mut mgr) = inner.write() {
                                        mgr.apply(
                                            drive_letter,
                                            &poll.events,
                                            poll.new_last_usn,
                                            poll.journal_id,
                                        );
                                    }
                                }
                            }
                            Err(err) => {
                                let count = fail_counts.entry(drive_letter).or_insert(0);
                                *count += 1;
                                // Log first failure and then every 30 consecutive failures
                                if *count == 1 || *count % 30 == 0 {
                                    emit_log(&event_tx, easysearch_core::logging::LogLevel::Warn, format!(
                                        "[easysearch-engine] {drive_letter}: USN poll failed (count={}): {}",
                                        count, err
                                    ));
                                }
                            }
                        }
                    }
                }

                emit(&event_tx, EngineEvent::Shutdown);
            })
            .expect("failed to spawn engine-index thread");
    }

    /// Search across all indexed drives with a structured query.
    pub fn search_query(&self, query: &SearchQuery) -> Vec<EsSearchResult> {
        self.search_query_inner(query, None)
    }

    /// Search across all indexed drives while observing cancellation.
    pub fn search_query_with_cancel(
        &self,
        query: &SearchQuery,
        cancel: &AtomicBool,
    ) -> Vec<EsSearchResult> {
        self.search_query_inner(query, Some(cancel))
    }

    fn search_query_inner(
        &self,
        query: &SearchQuery,
        cancel: Option<&AtomicBool>,
    ) -> Vec<EsSearchResult> {
        let start = Instant::now();
        let normalized = normalize_query(&query.pattern);

        let raw_results = match self.inner.read() {
            Ok(mgr) => match cancel {
                Some(token) => {
                    mgr.search_with_cancel(&normalized, query.limit.saturating_mul(2), token)
                }
                None => mgr.search(&normalized, query.limit.saturating_mul(2)),
            },
            Err(_) => Vec::new(),
        };

        if cancel.is_some_and(|token| token.load(Ordering::Relaxed)) {
            return Vec::new();
        }

        // Apply filters
        let mut results: Vec<EsSearchResult> = if query.filter.is_empty() {
            raw_results
        } else {
            raw_results
                .into_iter()
                .filter(|r| {
                    let flags = if r.is_directory { 0x10 } else { 0 };
                    query
                        .filter
                        .matches(&r.path, &r.name, r.is_directory, flags)
                })
                .collect()
        };

        // Apply sort
        match query.sort {
            SortOrder::Score => {
                results.sort_unstable_by(|left, right| {
                    right
                        .score
                        .cmp(&left.score)
                        .then_with(|| path_depth(&left.path).cmp(&path_depth(&right.path)))
                        .then_with(|| left.path.len().cmp(&right.path.len()))
                        .then_with(|| left.path.to_lowercase().cmp(&right.path.to_lowercase()))
                });
            }
            SortOrder::Name => {
                results.sort_unstable_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            }
            SortOrder::Path => {
                results.sort_unstable_by(|a, b| a.path.to_lowercase().cmp(&b.path.to_lowercase()));
            }
        }

        results.truncate(query.limit);
        self.metrics.record_search(start.elapsed());
        results
    }

    /// Simple search (backwards-compatible with old API).
    pub fn search(&self, query: &str, limit: usize) -> Vec<EsSearchResult> {
        self.search_query(&SearchQuery::new(query, limit))
    }

    /// Simple search that can stop when superseded by newer input.
    pub fn search_with_cancel(
        &self,
        query: &str,
        limit: usize,
        cancel: &AtomicBool,
    ) -> Vec<EsSearchResult> {
        self.search_query_with_cancel(&SearchQuery::new(query, limit), cancel)
    }

    /// Search with a per-input-session candidate cache.
    pub fn search_with_session(
        &self,
        session: &mut SearchSession,
        query: &str,
        limit: usize,
    ) -> Vec<EsSearchResult> {
        let cancel = AtomicBool::new(false);
        self.search_with_session_and_cancel(session, query, limit, &cancel)
    }

    /// Search with a candidate cache while observing cancellation.
    pub fn search_with_session_and_cancel(
        &self,
        session: &mut SearchSession,
        query: &str,
        limit: usize,
        cancel: &AtomicBool,
    ) -> Vec<EsSearchResult> {
        let start = Instant::now();
        let normalized = normalize_query(query);
        let mut results = match self.inner.read() {
            Ok(manager) => session.search(&manager, &normalized, limit.saturating_mul(2), cancel),
            Err(_) => Vec::new(),
        };

        if cancel.load(Ordering::Relaxed) {
            return Vec::new();
        }

        results.sort_unstable_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| path_depth(&left.path).cmp(&path_depth(&right.path)))
                .then_with(|| left.path.len().cmp(&right.path.len()))
                .then_with(|| left.path.to_lowercase().cmp(&right.path.to_lowercase()))
        });
        results.truncate(limit);
        self.metrics.record_search(start.elapsed());
        results
    }

    /// Enumerate a directory path. Thread-safe (acquires read lock).
    pub fn enumerate(
        &self,
        path: &str,
        query: &str,
        recursive: bool,
        limit: usize,
    ) -> Result<Vec<EsSearchResult>, String> {
        match self.inner.read() {
            Ok(mgr) => mgr.enumerate(path, query, recursive, limit),
            Err(_) => Ok(Vec::new()),
        }
    }

    /// Enumerate a directory path while observing cancellation.
    pub fn enumerate_with_cancel(
        &self,
        path: &str,
        query: &str,
        recursive: bool,
        limit: usize,
        cancel: &AtomicBool,
    ) -> Result<Vec<EsSearchResult>, String> {
        if cancel.load(Ordering::Relaxed) {
            return Ok(Vec::new());
        }
        match self.inner.read() {
            Ok(mgr) => mgr.enumerate_with_cancel(path, query, recursive, limit, cancel),
            Err(_) => Ok(Vec::new()),
        }
    }

    /// Returns `true` once all initial drives have been indexed.
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }

    /// Total indexed record count across all drives.
    #[must_use]
    pub fn record_count(&self) -> u64 {
        match self.inner.read() {
            Ok(mgr) => mgr.record_count(),
            Err(_) => 0,
        }
    }

    /// Get loaded drive labels (e.g. `["C:", "D:"]`).
    #[must_use]
    pub fn drive_labels(&self) -> Vec<String> {
        match self.inner.read() {
            Ok(mgr) => mgr.drive_labels(),
            Err(_) => Vec::new(),
        }
    }

    /// Get a snapshot of engine metrics.
    #[must_use]
    pub fn metrics(&self) -> MetricsSnapshot {
        self.metrics.snapshot()
    }

    /// Get the overall engine status.
    #[must_use]
    pub fn status(&self) -> EngineStatus {
        let ready = self.is_ready();
        let metrics = self.metrics.snapshot();
        let total_records = self.record_count();

        let drives = match self.inner.read() {
            Ok(mgr) => mgr
                .drive_letters()
                .iter()
                .map(|&drive| {
                    let records = mgr
                        .index_for(drive)
                        .map(|idx| idx.records_len() as u64)
                        .unwrap_or(0);
                    let build_time = metrics.build_times.get(&drive).copied();
                    DriveStatus {
                        drive,
                        state: if records > 0 {
                            DriveState::Ready
                        } else {
                            DriveState::Indexing
                        },
                        records,
                        build_time,
                    }
                })
                .collect(),
            Err(_) => Vec::new(),
        };

        EngineStatus {
            ready,
            drives,
            total_records,
            metrics,
        }
    }

    /// Hot-add a drive for indexing at runtime.
    ///
    /// The drive will be indexed in the background. Listen for
    /// [`EngineEvent::DriveReady`] to know when it's done.
    pub fn add_drive(&self, drive: char) {
        let letter = drive.to_ascii_uppercase();
        let _ = self.command_tx.send(WorkerCommand::AddDrive(letter));

        // For the initial implementation, do it inline on a new thread
        let inner = Arc::clone(&self.inner);
        let config = self.config.clone();
        let metrics = Arc::clone(&self.metrics);
        let event_tx = self.event_tx.clone();

        emit(&event_tx, EngineEvent::DriveAdded { drive: letter });
        emit(&event_tx, EngineEvent::DriveIndexing { drive: letter });

        thread::Builder::new()
            .name(format!("engine-add-{letter}"))
            .spawn(move || {
                let start = Instant::now();
                match build_index(letter, config.cache_dir.as_deref()) {
                    Ok(index) => {
                        let elapsed = start.elapsed();
                        let records = index.records_len() as u64;
                        if let Ok(mut mgr) = inner.write() {
                            mgr.install(letter, index);
                        }
                        metrics.record_build(letter, elapsed);
                        emit(
                            &event_tx,
                            EngineEvent::DriveReady {
                                drive: letter,
                                records,
                                elapsed,
                            },
                        );
                    }
                    Err(err) => {
                        emit(
                            &event_tx,
                            EngineEvent::DriveError {
                                drive: letter,
                                error: err,
                            },
                        );
                    }
                }
            })
            .ok();
    }

    /// Remove a drive's index from memory.
    pub fn remove_drive(&self, drive: char) {
        let letter = drive.to_ascii_uppercase();
        if let Ok(mut mgr) = self.inner.write() {
            mgr.remove(letter);
        }
        if let Some(ref tx) = self.event_tx {
            let _ = tx.send(EngineEvent::DriveRemoved { drive: letter });
        }
    }

    /// Force rebuild a drive's index (drop cache, re-read MFT).
    pub fn rebuild_drive(&self, drive: char) {
        let letter = drive.to_ascii_uppercase();
        if let Ok(mut mgr) = self.inner.write() {
            mgr.remove(letter);
        }

        let inner = Arc::clone(&self.inner);
        let metrics = Arc::clone(&self.metrics);
        let event_tx = self.event_tx.clone();
        let cache_dir = self.config.cache_dir.clone();
        emit(&event_tx, EngineEvent::DriveIndexing { drive: letter });

        thread::Builder::new()
            .name(format!("engine-rebuild-{letter}"))
            .spawn(move || {
                let start = Instant::now();
                // Passing no cache directory is intentional: a forced rebuild
                // must always read the live MFT, never race with an old cache.
                match build_index(letter, None) {
                    Ok(index) => {
                        let elapsed = start.elapsed();
                        let records = index.records_len() as u64;
                        persist_compact_cache(letter, &index, cache_dir.as_deref());
                        if let Ok(mut mgr) = inner.write() {
                            mgr.install(letter, index);
                        }
                        metrics.record_build(letter, elapsed);
                        emit(
                            &event_tx,
                            EngineEvent::DriveReady {
                                drive: letter,
                                records,
                                elapsed,
                            },
                        );
                    }
                    Err(error) => emit(
                        &event_tx,
                        EngineEvent::DriveError {
                            drive: letter,
                            error,
                        },
                    ),
                }
            })
            .ok();
    }

    /// Delete persisted flow caches and rebuild every configured drive from MFT.
    ///
    /// Returns the number of drives whose rebuild was started.
    pub fn clear_cache_and_rebuild(&self) -> Result<usize, String> {
        if let Some(cache_dir) = self.config.cache_dir.as_deref()
            && cache_dir.exists()
        {
            for entry in std::fs::read_dir(cache_dir)
                .map_err(|error| format!("read cache directory: {error}"))?
            {
                let entry = entry.map_err(|error| format!("read cache entry: {error}"))?;
                let path = entry.path();
                let is_flow_cache = path.extension().is_some_and(|ext| ext == "flowcache")
                    || path
                        .file_name()
                        .is_some_and(|name| name.to_string_lossy().ends_with(".flowcache.tmp"));
                if is_flow_cache {
                    std::fs::remove_file(&path)
                        .map_err(|error| format!("remove cache {}: {error}", path.display()))?;
                }
            }
        }

        let drives: Vec<char> = self
            .drive_labels()
            .into_iter()
            .filter_map(|label| label.chars().next())
            .collect();
        for &drive in &drives {
            self.rebuild_drive(drive);
        }
        Ok(drives.len())
    }

    /// Signal the engine to shut down gracefully.
    ///
    /// Background threads will exit on their next iteration.
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Release);
        let _ = self.command_tx.send(WorkerCommand::Shutdown);
    }

    /// Returns `true` if shutdown has been requested.
    #[must_use]
    pub fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::Acquire)
    }

    /// Get the USN cursor for a drive (for external polling if needed).
    #[must_use]
    pub fn cursor(&self, drive: char) -> Option<(u64, i64)> {
        let letter = drive.to_ascii_uppercase();
        self.inner.read().ok()?.cursor(letter)
    }
}

/// Emit an event if the sender is available (best-effort, non-blocking).
fn emit(tx: &Option<EventSender>, event: EngineEvent) {
    if let Some(sender) = tx {
        let _ = sender.send(event);
    }
}

/// Emit a typed log message and retain the event for diagnostics consumers.
fn emit_log(tx: &Option<EventSender>, level: easysearch_core::logging::LogLevel, msg: String) {
    easysearch_core::logging::write(level, module_path!(), &msg);
    emit(tx, EngineEvent::Log { message: msg });
}

fn schedule_compact(
    manager: &Arc<RwLock<DriveManager>>,
    compact_in_flight: &Arc<AtomicBool>,
    cache_dir: Option<std::path::PathBuf>,
) {
    if compact_in_flight
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }

    let candidate = manager
        .read()
        .ok()
        .and_then(|manager| manager.compact_candidate());
    let Some(candidate) = candidate else {
        compact_in_flight.store(false, Ordering::Release);
        return;
    };
    let Ok(candidate) = candidate else {
        easysearch_core::log_warn!("failed to capture compact snapshot");
        compact_in_flight.store(false, Ordering::Release);
        return;
    };

    let manager = Arc::clone(manager);
    let gate = Arc::clone(compact_in_flight);
    let spawn = thread::Builder::new()
        .name(format!("engine-compact-{}", candidate.drive))
        .spawn(move || {
            let _gate = CompactGate(gate);
            let started = Instant::now();
            easysearch_core::log_info!(
                "{}: compact started (delta={}, base={}, threshold=5%)",
                candidate.drive,
                candidate.delta_events,
                candidate.base_records
            );
            let mut rebuilt = match candidate.snapshot.rebuild() {
                Ok(index) => index,
                Err(error) => {
                    easysearch_core::log_warn!(
                        "{}: compact rebuild failed: {error}",
                        candidate.drive
                    );
                    return;
                }
            };
            rebuilt.status.journal_id = candidate.journal_id;
            rebuilt.status.last_usn = candidate.last_usn;

            let rebuilt_records = rebuilt.records_len();
            let committed = manager.write().ok().and_then(|mut manager| {
                if !manager.compact_revision_matches(candidate.drive, candidate.revision) {
                    return None;
                }
                let ok = manager.commit_compact(candidate.drive, candidate.revision, rebuilt);
                ok.then_some(())
            });
            if committed.is_none() {
                easysearch_core::log_debug!(
                    "{}: compact discarded because the index changed during rebuild",
                    candidate.drive
                );
                return;
            }

            // Write cache outside the write lock so search is never blocked by disk I/O.
            // Acquire a short read lock to access the freshly committed index.
            if let Ok(mgr) = manager.read() {
                if let Some(index) = mgr.index_for(candidate.drive) {
                    persist_compact_cache(candidate.drive, index, cache_dir.as_deref());
                }
            }

            easysearch_core::log_info!(
                "{}: compact finished in {:.2}s (records={})",
                candidate.drive,
                started.elapsed().as_secs_f64(),
                rebuilt_records
            );
        });
    if let Err(error) = spawn {
        compact_in_flight.store(false, Ordering::Release);
        easysearch_core::log_warn!("failed to spawn compact worker: {error}");
    }
}

struct CompactGate(Arc<AtomicBool>);

impl Drop for CompactGate {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[cfg(windows)]
fn persist_compact_cache(
    drive: char,
    index: &easysearch_core::EsIndex,
    cache_dir: Option<&std::path::Path>,
) {
    let Some(cache_dir) = cache_dir else {
        return;
    };
    let Some(volume_serial) = drive_manager::volume_serial(drive) else {
        return;
    };
    if let Err(error) = easysearch_core::cache::write_flow_cache(
        index,
        cache_dir,
        volume_serial,
        index.status.journal_id,
    ) {
        easysearch_core::log_warn!("{drive}: compact cache write failed: {error}");
    }
}

#[cfg(not(windows))]
fn persist_compact_cache(
    _drive: char,
    _index: &easysearch_core::EsIndex,
    _cache_dir: Option<&std::path::Path>,
) {
}

fn path_depth(path: &str) -> usize {
    path.chars().filter(|&ch| ch == '\\' || ch == '/').count()
}
