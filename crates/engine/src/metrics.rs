// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Engine runtime metrics for diagnostics and the "About" page.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Thread-safe metrics collector for the search engine.
#[derive(Debug)]
pub struct EngineMetrics {
    /// Per-drive index build durations.
    build_times: std::sync::Mutex<HashMap<char, Duration>>,
    /// Total USN events applied since start.
    usn_events_applied: AtomicU64,
    /// Total search queries served.
    search_count: AtomicU64,
    /// Cumulative search latency in microseconds.
    search_latency_us: AtomicU64,
    /// Engine start time.
    started_at: Instant,
}

impl EngineMetrics {
    /// Create a new metrics instance.
    #[must_use]
    pub fn new() -> Self {
        Self {
            build_times: std::sync::Mutex::new(HashMap::new()),
            usn_events_applied: AtomicU64::new(0),
            search_count: AtomicU64::new(0),
            search_latency_us: AtomicU64::new(0),
            started_at: Instant::now(),
        }
    }

    /// Record that a drive's index was built in `elapsed` time.
    pub fn record_build(&self, drive: char, elapsed: Duration) {
        if let Ok(mut map) = self.build_times.lock() {
            map.insert(drive, elapsed);
        }
    }

    /// Record that USN events were applied.
    pub fn record_usn_events(&self, count: usize) {
        self.usn_events_applied
            .fetch_add(count as u64, Ordering::Relaxed);
    }

    /// Record a search query with its latency.
    pub fn record_search(&self, latency: Duration) {
        self.search_count.fetch_add(1, Ordering::Relaxed);
        self.search_latency_us
            .fetch_add(latency.as_micros() as u64, Ordering::Relaxed);
    }

    /// Take a snapshot of current metrics.
    #[must_use]
    pub fn snapshot(&self) -> MetricsSnapshot {
        let build_times = self
            .build_times
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default();
        let search_count = self.search_count.load(Ordering::Relaxed);
        let total_latency_us = self.search_latency_us.load(Ordering::Relaxed);
        let avg_search_latency = if search_count > 0 {
            Duration::from_micros(total_latency_us / search_count)
        } else {
            Duration::ZERO
        };

        MetricsSnapshot {
            uptime: self.started_at.elapsed(),
            build_times,
            usn_events_applied: self.usn_events_applied.load(Ordering::Relaxed),
            search_count,
            avg_search_latency,
        }
    }
}

impl Default for EngineMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// A point-in-time snapshot of engine metrics.
#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    /// How long the engine has been running.
    pub uptime: Duration,
    /// Per-drive index build durations.
    pub build_times: HashMap<char, Duration>,
    /// Total USN events applied since start.
    pub usn_events_applied: u64,
    /// Total search queries served.
    pub search_count: u64,
    /// Average search latency.
    pub avg_search_latency: Duration,
}
