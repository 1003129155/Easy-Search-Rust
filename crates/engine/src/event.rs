// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Engine event system for progress reporting and state changes.
//!
//! Consumers (GUI, daemon, tests) subscribe via a [`std::sync::mpsc::Receiver`]
//! and get notified of indexing progress, errors, and USN updates without
//! polling.

use std::time::Duration;

/// Events emitted by the engine during its lifecycle.
#[derive(Debug, Clone)]
pub enum EngineEvent {
    /// A drive has started indexing.
    DriveIndexing { drive: char },
    /// A drive's index is ready.
    DriveReady {
        drive: char,
        records: u64,
        elapsed: Duration,
    },
    /// A drive's index build failed.
    DriveError { drive: char, error: String },
    /// All configured drives have been indexed (or attempted).
    AllReady,
    /// USN journal events were applied to a drive.
    UsnUpdate { drive: char, events_applied: usize },
    /// A drive was hot-added at runtime.
    DriveAdded { drive: char },
    /// A drive was removed at runtime.
    DriveRemoved { drive: char },
    /// The engine is shutting down.
    Shutdown,
    /// A log message from the engine (for persistence by the consumer).
    Log { message: String },
}

/// Sender half for engine events.
pub type EventSender = std::sync::mpsc::Sender<EngineEvent>;

/// Receiver half for engine events.
pub type EventReceiver = std::sync::mpsc::Receiver<EngineEvent>;

/// Create a new event channel.
#[must_use]
pub fn event_channel() -> (EventSender, EventReceiver) {
    std::sync::mpsc::channel()
}
