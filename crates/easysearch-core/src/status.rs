// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Runtime status for a loaded index.

use serde::{Deserialize, Serialize};

/// High-level state of one index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EsIndexState {
    /// No cache or in-memory index is loaded.
    Empty,
    /// A full MFT build or snapshot rebuild is in progress.
    Indexing,
    /// The index is searchable.
    Ready,
    /// The index failed to build or load.
    Error,
}

/// Status snapshot returned to the daemon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EsIndexStatus {
    /// Current high-level state.
    pub state: EsIndexState,
    /// Number of records known to the index.
    pub records: u64,
    /// USN cursor represented by the loaded snapshot.
    pub last_usn: i64,
    /// USN journal id this cursor belongs to (`0` when unknown).
    pub journal_id: u64,
}

impl EsIndexStatus {
    /// Empty status used before an index is loaded.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            state: EsIndexState::Empty,
            records: 0,
            last_usn: 0,
            journal_id: 0,
        }
    }

    /// Ready status for a loaded index.
    #[must_use]
    pub const fn ready(records: u64, last_usn: i64) -> Self {
        Self {
            state: EsIndexState::Ready,
            records,
            last_usn,
            journal_id: 0,
        }
    }

    /// Returns `true` when queries can be served.
    #[must_use]
    pub const fn ready_for_queries(self) -> bool {
        matches!(self.state, EsIndexState::Ready)
    }

    /// Returns `true` when the index is currently building.
    #[must_use]
    pub const fn indexing(self) -> bool {
        matches!(self.state, EsIndexState::Indexing)
    }
}

impl Default for EsIndexStatus {
    fn default() -> Self {
        Self::empty()
    }
}
