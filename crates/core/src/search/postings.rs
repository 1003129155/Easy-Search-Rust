// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Posting lists — REMOVED.
//!
//! The trigram posting list index was consuming ~600MB for 5M records.
//! Replaced by parallel linear scan over the contiguous names blob
//! (Everything-style). This file is kept as a zero-size placeholder so
//! existing `mod postings` in `search/mod.rs` compiles without changes.

/// Empty placeholder — trigram postings have been removed.
#[derive(Debug, Clone, Default)]
pub struct PostingsStore;

impl PostingsStore {
    /// No-op: trigram postings are no longer stored.
    pub fn add(&mut self, _key: String, _record_idx: u32) {}

    /// Always returns `None` — linear scan is used instead.
    #[must_use]
    pub fn get(&self, _key: &str) -> Option<&[u32]> {
        None
    }
}
