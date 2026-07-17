// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! In-memory overlay for USN-driven changes.
//!
//! The overlay sits above the immutable base snapshot (`EsIndex::records`)
//! and records the deltas produced by replaying the USN journal:
//!
//! * `inserted` — files/directories created after the snapshot. Each is
//!   addressed by a *logical index* `base_len + position`.
//! * `renamed` — basename replacements keyed by logical index.
//! * `moved` — parent replacements keyed by logical index.
//! * `deleted` — tombstoned logical indices (base or inserted).
//!
//! Snapshot compaction folds the overlay back into a fresh base and clears it.

use std::collections::{BTreeMap, BTreeSet, HashMap};

/// A record created after the base snapshot.
///
/// Stored with an owned basename so the overlay is self-contained and does not
/// need a separate delta names blob for the first version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InsertedRecord {
    /// NTFS file reference of the new record.
    pub file_ref: u64,
    /// NTFS file reference of the parent directory.
    ///
    /// The parent is resolved to a logical index lazily at query time via the
    /// index's `file_ref_map`, so a child created before its parent in the
    /// same poll batch still links up once both are applied.
    pub parent_ref: u64,
    /// Basename of the new record.
    pub name: String,
    /// Record flags (see [`crate::record::flags`]).
    pub flags: u16,
}

/// Mutable overlay applied above the mmap-friendly base snapshot.
#[derive(Debug, Clone, Default)]
pub struct EsDeltaOverlay {
    /// Records created after the base snapshot (logical idx = `base_len + i`).
    pub inserted: Vec<InsertedRecord>,
    /// Basename replacements keyed by logical index.
    pub renamed: BTreeMap<u32, String>,
    /// Parent replacements keyed by logical index.
    pub moved: BTreeMap<u32, u32>,
    /// Tombstoned logical indices.
    pub deleted: BTreeSet<u32>,
    /// O(1) file-reference → logical-index overrides layered above the base
    /// snapshot's immutable `FileRefMap`.
    ///
    /// `Some(idx)` shadows or adds a mapping (a newly inserted record, or a
    /// reused MFT record number pointing at a new logical index). `None` is a
    /// tombstone that hides a base mapping (the file was deleted).
    ///
    /// This exists so applying a single USN event is O(1): the previous design
    /// rebuilt the entire `FileRefMap` (export all pairs, linear-scan, re-sort)
    /// on every create/delete, which is O(n log n) per event and dominated CPU
    /// on busy volumes. The base map stays immutable; only this overlay moves.
    pub ref_overrides: HashMap<u64, Option<u32>>,
}

impl EsDeltaOverlay {
    /// Returns `true` if no pending changes exist.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inserted.is_empty()
            && self.renamed.is_empty()
            && self.moved.is_empty()
            && self.deleted.is_empty()
            && self.ref_overrides.is_empty()
    }

    /// Total number of pending change entries (used for snapshot triggers).
    #[must_use]
    pub fn event_count(&self) -> usize {
        self.inserted
            .len()
            .saturating_add(self.renamed.len())
            .saturating_add(self.moved.len())
            .saturating_add(self.deleted.len())
    }

    /// Mark a logical index as deleted in the overlay.
    pub fn mark_deleted(&mut self, index: u32) {
        self.deleted.insert(index);
    }

    /// Returns `true` if `index` is deleted in the overlay.
    #[must_use]
    pub fn is_deleted(&self, index: u32) -> bool {
        self.deleted.contains(&index)
    }
}
