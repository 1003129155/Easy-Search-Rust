// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! In-memory representation of an EasySearch index.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::builder::EsIndexBuilder;
use crate::delta::{EsDeltaOverlay, InsertedRecord};
use crate::error::{EsError, Result};
use crate::path::normalize_path_for_lookup;
use crate::record::{EsRecord, PARENT_NONE, flags as es_flags, mft_record_number};
use crate::search::{EsSearchIndex, EsSearchResult, score_name};
use crate::status::EsIndexStatus;
use crate::usn::{EsUsnEvent, EsUsnEventKind};

const MISSING_INDEX: u32 = u32::MAX;

/// Result of a cache-aware search pass.
///
/// `candidate_ids` contains every matching logical record ID when the set fit
/// within the caller's memory budget. `None` means results are valid, but the
/// complete candidate set overflowed and must not be reused as a cache.
#[derive(Debug, Default)]
pub struct EsCandidateSearch {
    /// Ranked and materialized top results.
    pub results: Vec<EsSearchResult>,
    /// Complete matching record IDs, or `None` when collection overflowed.
    pub candidate_ids: Option<Vec<u32>>,
}

/// Immutable logical view captured for lock-free index compaction.
#[derive(Debug)]
pub struct EsCompactSnapshot {
    records: Vec<EsCompactRecord>,
    status: EsIndexStatus,
}

#[derive(Debug)]
struct EsCompactRecord {
    logical_index: u32,
    file_ref: u64,
    parent_logical_index: Option<u32>,
    name: String,
    flags: u16,
    rank: u16,
}

impl EsCompactSnapshot {
    /// Rebuild the captured logical view as a fresh base index with no delta.
    pub fn rebuild(self) -> Result<EsIndex> {
        let mut old_to_new = BTreeMap::new();
        for (new_index, record) in self.records.iter().enumerate() {
            old_to_new.insert(
                record.logical_index,
                u32::try_from(new_index).map_err(|_| EsError::RecordCountTooLarge {
                    len: self.records.len(),
                })?,
            );
        }

        let mut builder = EsIndexBuilder::with_capacity(self.records.len());
        for record in self.records {
            let parent = record
                .parent_logical_index
                .and_then(|old_parent| old_to_new.get(&old_parent).copied())
                .unwrap_or(PARENT_NONE);
            builder.add_record(
                record.file_ref,
                parent,
                &record.name,
                record.flags,
                record.rank,
            )?;
        }
        let mut index = builder.finish()?;
        index.status = self.status;
        index.status.records = u64::try_from(index.records.len()).unwrap_or(u64::MAX);
        Ok(index)
    }
}

/// Sorted file-reference lookup entry.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct FileRefEntry {
    /// NTFS file reference or low 48-bit record number.
    pub file_ref: u64,
    /// Record index in [`EsIndex::records`].
    pub idx: u32,
    /// Explicit padding for 8-byte alignment.
    #[expect(
        clippy::pub_underscore_fields,
        reason = "bytemuck Pod requires all fields same visibility"
    )]
    pub _pad: u32,
}

/// Lookup table from NTFS file reference to record index.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum FileRefMap {
    /// Empty map.
    #[default]
    Empty,
    /// Dense map keyed by low 48-bit MFT record number.
    Dense {
        /// First MFT record number represented by `slots[0]`.
        base_record_number: u64,
        /// Slot values; [`MISSING_INDEX`] means absent.
        slots: Vec<u32>,
    },
    /// Sorted sparse entries.
    Sorted(Vec<FileRefEntry>),
}

impl FileRefMap {
    /// Build a dense or sorted lookup from `(file_ref, idx)` pairs.
    #[must_use]
    pub fn from_pairs(pairs: &[(u64, u32)]) -> Self {
        if pairs.is_empty() {
            return Self::Empty;
        }

        let mut record_numbers: Vec<u64> = pairs
            .iter()
            .map(|(file_ref, _)| mft_record_number(*file_ref))
            .collect();
        record_numbers.sort_unstable();
        let first = record_numbers[0];
        let last = record_numbers[record_numbers.len() - 1];
        let span = last.saturating_sub(first).saturating_add(1);

        if let Ok(span_len) = usize::try_from(span) {
            if span_len <= pairs.len().saturating_mul(4).max(1_024) {
                let mut slots = vec![MISSING_INDEX; span_len];
                for (file_ref, idx) in pairs {
                    let record_number = mft_record_number(*file_ref);
                    let slot_idx =
                        usize::try_from(record_number.saturating_sub(first)).unwrap_or(usize::MAX);
                    if let Some(slot) = slots.get_mut(slot_idx) {
                        *slot = *idx;
                    }
                }
                return Self::Dense {
                    base_record_number: first,
                    slots,
                };
            }
        }

        let mut entries: Vec<FileRefEntry> = pairs
            .iter()
            .map(|(file_ref, idx)| FileRefEntry {
                file_ref: *file_ref,
                idx: *idx,
                _pad: 0,
            })
            .collect();
        entries.sort_unstable_by_key(|entry| mft_record_number(entry.file_ref));
        Self::Sorted(entries)
    }

    /// Return the record index for `file_ref`.
    #[must_use]
    pub fn get(&self, file_ref: u64) -> Option<u32> {
        let record_number = mft_record_number(file_ref);
        match self {
            Self::Empty => None,
            Self::Dense {
                base_record_number,
                slots,
            } => {
                let slot_idx =
                    usize::try_from(record_number.checked_sub(*base_record_number)?).ok()?;
                slots
                    .get(slot_idx)
                    .copied()
                    .filter(|idx| *idx != MISSING_INDEX)
            }
            Self::Sorted(entries) => entries
                .binary_search_by_key(&record_number, |entry| mft_record_number(entry.file_ref))
                .ok()
                .and_then(|entry_idx| entries.get(entry_idx).map(|entry| entry.idx)),
        }
    }

    /// Insert or replace a lookup entry.
    pub fn insert(&mut self, file_ref: u64, idx: u32) {
        let mut pairs = self.pairs();
        if let Some((_, existing_idx)) = pairs.iter_mut().find(|(existing_ref, _)| {
            mft_record_number(*existing_ref) == mft_record_number(file_ref)
        }) {
            *existing_idx = idx;
        } else {
            pairs.push((file_ref, idx));
        }
        *self = Self::from_pairs(&pairs);
    }

    /// Remove a lookup entry.
    pub fn remove(&mut self, file_ref: u64) {
        let record_number = mft_record_number(file_ref);
        let pairs: Vec<(u64, u32)> = self
            .pairs()
            .into_iter()
            .filter(|(existing_ref, _)| mft_record_number(*existing_ref) != record_number)
            .collect();
        *self = Self::from_pairs(&pairs);
    }

    /// Return all `(file_ref, idx)` pairs in this map.
    #[must_use]
    pub fn pairs(&self) -> Vec<(u64, u32)> {
        match self {
            Self::Empty => Vec::new(),
            Self::Dense {
                base_record_number,
                slots,
            } => slots
                .iter()
                .enumerate()
                .filter_map(|(offset, idx)| {
                    if *idx == MISSING_INDEX {
                        None
                    } else {
                        let record_number =
                            base_record_number.saturating_add(u64::try_from(offset).ok()?);
                        Some((record_number, *idx))
                    }
                })
                .collect(),
            Self::Sorted(entries) => entries
                .iter()
                .map(|entry| (entry.file_ref, entry.idx))
                .collect(),
        }
    }
}

/// EasySearch index over one volume.
#[derive(Debug, Clone, Default)]
pub struct EsIndex {
    /// Fixed-width record column.
    pub records: Vec<EsRecord>,
    /// Concatenated UTF-8 basenames.
    pub names: Vec<u8>,
    /// CSR offsets into [`EsIndex::children_indices`].
    pub children_offsets: Vec<u32>,
    /// CSR child record indices.
    pub children_indices: Vec<u32>,
    /// File-reference lookup used by USN updates.
    pub file_ref_map: FileRefMap,
    /// Derived filename search structures.
    pub search: EsSearchIndex,
    /// Pending changes above the base snapshot.
    pub delta: EsDeltaOverlay,
    /// Runtime status for this index.
    pub status: EsIndexStatus,
}

impl EsIndex {
    /// Construct an index from pre-built columns.
    #[must_use]
    pub fn from_parts(
        records: Vec<EsRecord>,
        names: Vec<u8>,
        children_offsets: Vec<u32>,
        children_indices: Vec<u32>,
        file_ref_map: FileRefMap,
        search: EsSearchIndex,
        status: EsIndexStatus,
    ) -> Self {
        Self {
            records,
            names,
            children_offsets,
            children_indices,
            file_ref_map,
            search,
            delta: EsDeltaOverlay::default(),
            status,
        }
    }

    /// Return the number of base records.
    #[must_use]
    pub fn records_len(&self) -> usize {
        self.records.len()
    }

    /// Return a record by index.
    pub fn record(&self, index: u32) -> Result<EsRecord> {
        let idx = usize::try_from(index).map_err(|_| EsError::RecordIndexOutOfRange {
            index,
            len: self.records.len(),
        })?;
        self.records
            .get(idx)
            .copied()
            .ok_or(EsError::RecordIndexOutOfRange {
                index,
                len: self.records.len(),
            })
    }

    /// Return a record basename.
    pub fn name(&self, index: u32) -> Result<&str> {
        let record = self.record(index)?;
        let start = usize::try_from(record.name_offset).map_err(|_| EsError::InvalidNameRange {
            index,
            offset: record.name_offset,
            len: record.name_len,
        })?;
        let len = usize::from(record.name_len);
        let end = start.checked_add(len).ok_or(EsError::InvalidNameRange {
            index,
            offset: record.name_offset,
            len: record.name_len,
        })?;
        let bytes = self
            .names
            .get(start..end)
            .ok_or(EsError::InvalidNameRange {
                index,
                offset: record.name_offset,
                len: record.name_len,
            })?;
        core::str::from_utf8(bytes).map_err(EsError::from)
    }

    /// Reconstruct a full path by walking parent indices.
    pub fn path_from_idx(&self, index: u32) -> Result<String> {
        let mut parts = Vec::new();
        let mut current = index;
        for _ in 0..=self.records.len() {
            let record = self.record(current)?;
            parts.push(self.name(current)?.to_string());
            if record.parent_idx == PARENT_NONE {
                parts.reverse();
                return Ok(join_path_parts(&parts));
            }
            current = record.parent_idx;
        }
        Err(EsError::ParentCycle { index })
    }

    /// Return direct children for `index`.
    pub fn children(&self, index: u32) -> Result<&[u32]> {
        let idx = usize::try_from(index).map_err(|_| EsError::RecordIndexOutOfRange {
            index,
            len: self.records.len(),
        })?;
        if idx >= self.records.len() {
            return Err(EsError::RecordIndexOutOfRange {
                index,
                len: self.records.len(),
            });
        }
        let start =
            self.children_offsets
                .get(idx)
                .copied()
                .ok_or(EsError::RecordIndexOutOfRange {
                    index,
                    len: self.records.len(),
                })?;
        let end = self
            .children_offsets
            .get(idx.saturating_add(1))
            .copied()
            .ok_or(EsError::RecordIndexOutOfRange {
                index,
                len: self.records.len(),
            })?;
        let start_idx = usize::try_from(start).map_err(|_| EsError::RecordIndexOutOfRange {
            index,
            len: self.records.len(),
        })?;
        let end_idx = usize::try_from(end).map_err(|_| EsError::RecordIndexOutOfRange {
            index,
            len: self.records.len(),
        })?;
        self.children_indices
            .get(start_idx..end_idx)
            .ok_or(EsError::RecordIndexOutOfRange {
                index,
                len: self.records.len(),
            })
    }

    /// Number of base records as a `u32` (saturating).
    #[must_use]
    fn base_len(&self) -> u32 {
        u32::try_from(self.records.len()).unwrap_or(u32::MAX)
    }

    /// Total logical record count (base + overlay-inserted).
    #[must_use]
    fn logical_len(&self) -> u32 {
        self.base_len()
            .saturating_add(u32::try_from(self.delta.inserted.len()).unwrap_or(0))
    }

    /// Borrow an inserted overlay record by logical index.
    fn inserted_at(&self, index: u32) -> Option<&InsertedRecord> {
        let pos = usize::try_from(index.checked_sub(self.base_len())?).ok()?;
        self.delta.inserted.get(pos)
    }

    /// Resolve a file reference to its logical index, honoring the delta
    /// overlay's O(1) overrides above the immutable base `FileRefMap`.
    ///
    /// The overlay wins: `Some(idx)` shadows the base mapping, `None` is a
    /// tombstone hiding a deleted base entry. Falls through to the base map
    /// when no override exists.
    fn ref_lookup(&self, file_ref: u64) -> Option<u32> {
        // Key by MFT record number (low 48 bits) so the overlay aligns with the
        // base `FileRefMap`, which also strips the sequence number. Otherwise a
        // record-number reuse with a bumped sequence number would key a
        // different slot than the base map and miss the override.
        let key = mft_record_number(file_ref);
        match self.delta.ref_overrides.get(&key) {
            Some(&Some(idx)) => Some(idx),
            Some(&None) => None,
            None => self.file_ref_map.get(file_ref),
        }
    }

    /// Return `true` when a logical index is tombstoned (base flag or overlay).
    #[must_use]
    fn logical_is_deleted(&self, index: u32) -> bool {
        if self.delta.is_deleted(index) {
            return true;
        }
        if index < self.base_len() {
            return self
                .record(index)
                .map(EsRecord::is_tombstone)
                .unwrap_or(true);
        }
        self.inserted_at(index).is_none()
    }

    /// Effective basename for a logical index, honouring overlay renames.
    fn logical_name(&self, index: u32) -> Option<String> {
        self.logical_name_ref(index).map(str::to_owned)
    }

    /// Borrow the effective basename for a logical index without allocating.
    fn logical_name_ref(&self, index: u32) -> Option<&str> {
        if let Some(name) = self.delta.renamed.get(&index) {
            return Some(name);
        }
        if index < self.base_len() {
            return self.name(index).ok();
        }
        self.inserted_at(index).map(|rec| rec.name.as_str())
    }

    /// Effective parent logical index, or `None` for a root.
    fn logical_parent(&self, index: u32) -> Option<u32> {
        if let Some(&moved) = self.delta.moved.get(&index) {
            return if moved == PARENT_NONE {
                None
            } else {
                Some(moved)
            };
        }
        if index < self.base_len() {
            let parent = self.record(index).ok()?.parent_idx;
            return if parent == PARENT_NONE {
                None
            } else {
                Some(parent)
            };
        }
        let parent_ref = self.inserted_at(index)?.parent_ref;
        self.ref_lookup(parent_ref)
    }

    /// Effective directory flag for a logical index.
    #[must_use]
    fn logical_is_dir(&self, index: u32) -> bool {
        if index < self.base_len() {
            self.record(index)
                .map(EsRecord::is_directory)
                .unwrap_or(false)
        } else {
            self.inserted_at(index)
                .map(|rec| rec.flags & es_flags::DIRECTORY != 0)
                .unwrap_or(false)
        }
    }

    /// Reconstruct a full path for a logical index across base + overlay.
    fn logical_path(&self, index: u32) -> Result<String> {
        let mut parts = Vec::new();
        let mut current = index;
        for _ in 0..=self.logical_len() {
            let name = self
                .logical_name(current)
                .ok_or(EsError::RecordIndexOutOfRange {
                    index: current,
                    len: self.records.len(),
                })?;
            parts.push(name);
            match self.logical_parent(current) {
                None => {
                    parts.reverse();
                    return Ok(join_path_parts(&parts));
                }
                Some(parent) => current = parent,
            }
        }
        Err(EsError::ParentCycle { index })
    }

    /// Effective children of a logical directory across base + overlay.
    ///
    /// `None` means the traversal was cancelled before a complete child set
    /// could be assembled.
    fn logical_children_inner(&self, index: u32, cancel: Option<&AtomicBool>) -> Option<Vec<u32>> {
        if is_cancelled(cancel) {
            return None;
        }
        let mut kids = Vec::new();

        if index < self.base_len() {
            if let Ok(base_children) = self.children(index) {
                for (position, &child) in base_children.iter().enumerate() {
                    if position % 1024 == 0 && is_cancelled(cancel) {
                        return None;
                    }
                    if self.delta.deleted.contains(&child) {
                        continue;
                    }
                    if let Some(&new_parent) = self.delta.moved.get(&child) {
                        if new_parent != index {
                            continue;
                        }
                    }
                    kids.push(child);
                }
            }
        }

        for (position, (&moved_idx, &new_parent)) in self.delta.moved.iter().enumerate() {
            if position % 1024 == 0 && is_cancelled(cancel) {
                return None;
            }
            if new_parent != index || self.delta.deleted.contains(&moved_idx) {
                continue;
            }
            if moved_idx < self.base_len() {
                if let Ok(rec) = self.record(moved_idx) {
                    if rec.parent_idx == index {
                        continue;
                    }
                }
            }
            kids.push(moved_idx);
        }

        for pos in 0..self.delta.inserted.len() {
            if pos % 1024 == 0 && is_cancelled(cancel) {
                return None;
            }
            let logical = self
                .base_len()
                .saturating_add(u32::try_from(pos).unwrap_or(u32::MAX));
            if self.delta.deleted.contains(&logical) {
                continue;
            }
            if self.logical_parent(logical) == Some(index) {
                kids.push(logical);
            }
        }

        Some(kids)
    }

    /// Number of pending delta entries waiting to be folded into the base.
    #[must_use]
    pub fn delta_event_count(&self) -> usize {
        self.delta.event_count()
    }

    /// Return whether the delta reached five percent of the base record count.
    #[must_use]
    pub fn should_compact(&self) -> bool {
        if self.delta.is_empty() {
            return false;
        }
        let threshold = self.records.len().saturating_add(19) / 20;
        self.delta.event_count() >= threshold.max(1)
    }

    /// Capture the current effective logical records for lock-free rebuilding.
    pub fn compact_snapshot(&self) -> Result<EsCompactSnapshot> {
        let logical_len = self.logical_len();
        let mut records = Vec::with_capacity(
            usize::try_from(logical_len)
                .unwrap_or(usize::MAX)
                .min(self.records.len()),
        );
        for index in 0..logical_len {
            if self.logical_is_deleted(index) {
                continue;
            }
            let (file_ref, flags, rank) = if index < self.base_len() {
                let record = self.record(index)?;
                (record.file_ref, record.flags, record.rank)
            } else {
                let record = self
                    .inserted_at(index)
                    .ok_or(EsError::RecordIndexOutOfRange {
                        index,
                        len: self.records.len(),
                    })?;
                (record.file_ref, record.flags, 0)
            };
            let name = self
                .logical_name(index)
                .ok_or(EsError::RecordIndexOutOfRange {
                    index,
                    len: self.records.len(),
                })?;
            records.push(EsCompactRecord {
                logical_index: index,
                file_ref,
                parent_logical_index: self.logical_parent(index),
                name,
                flags,
                rank,
            });
        }
        Ok(EsCompactSnapshot {
            records,
            status: self.status,
        })
    }

    /// Search basenames and return slim results.
    pub fn search(&self, query: &str, limit: usize) -> Vec<EsSearchResult> {
        self.search_inner(query, limit, None)
    }

    /// Search basenames while allowing an obsolete query to be cancelled.
    pub fn search_with_cancel(
        &self,
        query: &str,
        limit: usize,
        cancel: &AtomicBool,
    ) -> Vec<EsSearchResult> {
        self.search_inner(query, limit, Some(cancel))
    }

    /// Scan all logical records, returning top results and the complete set of
    /// matching IDs when it fits within `max_candidates`.
    pub fn search_collect_candidates_with_cancel(
        &self,
        query: &str,
        limit: usize,
        max_candidates: usize,
        cancel: &AtomicBool,
    ) -> EsCandidateSearch {
        self.search_candidates_inner(query, None, limit, max_candidates, cancel)
    }

    /// Refine a complete candidate set for a stricter query.
    ///
    /// Callers must only pass IDs from the same index generation.
    pub fn search_candidate_ids_with_cancel(
        &self,
        query: &str,
        source_ids: &[u32],
        limit: usize,
        max_candidates: usize,
        cancel: &AtomicBool,
    ) -> EsCandidateSearch {
        self.search_candidates_inner(query, Some(source_ids), limit, max_candidates, cancel)
    }

    fn search_candidates_inner(
        &self,
        query: &str,
        source_ids: Option<&[u32]>,
        limit: usize,
        max_candidates: usize,
        cancel: &AtomicBool,
    ) -> EsCandidateSearch {
        if limit == 0 || cancel.load(Ordering::Relaxed) {
            return EsCandidateSearch::default();
        }

        let query_folded = crate::search::fold::fold_text(query);
        if query_folded.is_empty() {
            return EsCandidateSearch::default();
        }

        let source_len = source_ids.map_or_else(
            || usize::try_from(self.logical_len()).unwrap_or(usize::MAX),
            <[u32]>::len,
        );
        let mut candidate_ids = Some(Vec::with_capacity(
            source_len.min(max_candidates).min(65_536),
        ));
        let mut scored: Vec<(u32, Vec<[u32; 2]>, u32)> = Vec::new();
        let top_cap = limit.saturating_mul(2);

        let mut visit = |index: u32, name: &str, is_dir: bool| {
            let Some((score, highlight)) = score_name(&query_folded, name, is_dir) else {
                return;
            };

            if let Some(ids) = &mut candidate_ids {
                if ids.len() < max_candidates {
                    ids.push(index);
                } else {
                    candidate_ids = None;
                }
            }
            insert_top_n(&mut scored, (score, highlight, index), top_cap);
        };

        match source_ids {
            Some(ids) => {
                for (position, &index) in ids.iter().enumerate() {
                    if position % 1024 == 0 && cancel.load(Ordering::Relaxed) {
                        return EsCandidateSearch::default();
                    }
                    if self.logical_is_deleted(index) {
                        continue;
                    }
                    let Some(name) = self.logical_name_ref(index) else {
                        continue;
                    };
                    visit(index, name, self.logical_is_dir(index));
                }
            }
            None => {
                let has_renames = !self.delta.renamed.is_empty();
                for (position, record) in self.records.iter().enumerate() {
                    if position % 1024 == 0 && cancel.load(Ordering::Relaxed) {
                        return EsCandidateSearch::default();
                    }
                    let index = u32::try_from(position).unwrap_or(u32::MAX);
                    if record.is_tombstone() || self.delta.is_deleted(index) {
                        continue;
                    }
                    let name = if has_renames {
                        self.delta
                            .renamed
                            .get(&index)
                            .map(String::as_str)
                            .or_else(|| self.name(index).ok())
                    } else {
                        let start = record.name_offset as usize;
                        let end = start.saturating_add(record.name_len as usize);
                        self.names
                            .get(start..end)
                            .and_then(|bytes| core::str::from_utf8(bytes).ok())
                    };
                    let Some(name) = name else {
                        continue;
                    };
                    visit(index, name, record.is_directory());
                }

                for position in 0..self.delta.inserted.len() {
                    if position % 1024 == 0 && cancel.load(Ordering::Relaxed) {
                        return EsCandidateSearch::default();
                    }
                    let index = self
                        .base_len()
                        .saturating_add(u32::try_from(position).unwrap_or(u32::MAX));
                    if self.logical_is_deleted(index) {
                        continue;
                    }
                    let Some(record) = self.inserted_at(index) else {
                        continue;
                    };
                    visit(index, &record.name, record.flags & es_flags::DIRECTORY != 0);
                }
            }
        }

        if cancel.load(Ordering::Relaxed) {
            return EsCandidateSearch::default();
        }

        EsCandidateSearch {
            results: self.materialize_scored(scored, limit, Some(cancel)),
            candidate_ids,
        }
    }

    fn search_inner(
        &self,
        query: &str,
        limit: usize,
        cancel: Option<&AtomicBool>,
    ) -> Vec<EsSearchResult> {
        if limit == 0 {
            return Vec::new();
        }
        if is_cancelled(cancel) {
            return Vec::new();
        }
        let query_folded = crate::search::fold::fold_text(query);

        let mut scored: Vec<(u32, Vec<[u32; 2]>, u32)> = Vec::new();

        if query_folded.is_empty() {
            for index in 0..self.logical_len() {
                if index % 1024 == 0 && is_cancelled(cancel) {
                    return Vec::new();
                }
                if self.logical_is_deleted(index) {
                    continue;
                }
                let is_dir = self.logical_is_dir(index);
                scored.push((
                    score_name("", "", is_dir).unwrap_or((0, Vec::new())).0,
                    Vec::new(),
                    index,
                ));
                if scored.len() >= limit {
                    break;
                }
            }
        } else if query_folded.chars().count() <= 2 {
            let candidates = self.search.prefix.candidates(&query_folded);
            for (candidate_pos, index) in candidates.into_iter().enumerate() {
                if candidate_pos % 1024 == 0 && is_cancelled(cancel) {
                    return Vec::new();
                }
                if index >= self.base_len() || self.logical_is_deleted(index) {
                    continue;
                }
                if let Some(name) = self.logical_name_ref(index) {
                    let is_dir = self.logical_is_dir(index);
                    if let Some((score, highlight)) = score_name(&query_folded, name, is_dir) {
                        scored.push((score, highlight, index));
                    }
                }
            }
            self.scan_delta_inserted(&query_folded, &mut scored);
        } else {
            let has_renames = !self.delta.renamed.is_empty();
            for (record_idx, record) in self.records.iter().enumerate() {
                if record_idx % 1024 == 0 && is_cancelled(cancel) {
                    return Vec::new();
                }
                let index = record_idx as u32;
                if record.is_tombstone() || self.delta.is_deleted(index) {
                    continue;
                }
                // Honour overlay renames: a base record whose name was changed
                // via USN must match against its new name, not the stale bytes
                // stored in `self.names`.
                let name = if has_renames {
                    match self
                        .delta
                        .renamed
                        .get(&index)
                        .map(String::as_str)
                        .or_else(|| {
                            let start = record.name_offset as usize;
                            let end = start.saturating_add(record.name_len as usize);
                            self.names
                                .get(start..end)
                                .and_then(|bytes| core::str::from_utf8(bytes).ok())
                        }) {
                        Some(name) => name,
                        None => continue,
                    }
                } else {
                    let start = record.name_offset as usize;
                    let end = start + record.name_len as usize;
                    let name_bytes = match self.names.get(start..end) {
                        Some(b) => b,
                        None => continue,
                    };
                    match core::str::from_utf8(name_bytes) {
                        Ok(s) => s,
                        Err(_) => continue,
                    }
                };
                let is_dir = record.is_directory();
                if let Some((score, highlight)) = score_name(&query_folded, name, is_dir) {
                    insert_top_n(
                        &mut scored,
                        (score, highlight, index),
                        limit.saturating_mul(2),
                    );
                }
            }
            self.scan_delta_inserted(&query_folded, &mut scored);
        }

        self.materialize_scored(scored, limit, cancel)
    }

    /// Scan delta-inserted records for matches.
    fn scan_delta_inserted(&self, query_folded: &str, scored: &mut Vec<(u32, Vec<[u32; 2]>, u32)>) {
        for pos in 0..self.delta.inserted.len() {
            let logical = self
                .base_len()
                .saturating_add(u32::try_from(pos).unwrap_or(u32::MAX));
            if self.logical_is_deleted(logical) {
                continue;
            }
            if let Some(name) = self.logical_name_ref(logical) {
                let is_dir = self.logical_is_dir(logical);
                if let Some((score, highlight)) = score_name(query_folded, name, is_dir) {
                    scored.push((score, highlight, logical));
                }
            }
        }
    }

    fn materialize_scored(
        &self,
        mut scored: Vec<(u32, Vec<[u32; 2]>, u32)>,
        limit: usize,
        cancel: Option<&AtomicBool>,
    ) -> Vec<EsSearchResult> {
        scored.sort_unstable_by(|a, b| b.0.cmp(&a.0));
        scored.truncate(limit);

        let mut results = Vec::with_capacity(scored.len());
        for (score, highlight, index) in scored {
            if is_cancelled(cancel) {
                return Vec::new();
            }
            let name = match self.logical_name(index) {
                Some(name) => name,
                None => continue,
            };
            let path = match self.logical_path(index) {
                Ok(path) => path,
                Err(_) => continue,
            };
            results.push(EsSearchResult {
                path,
                name,
                is_directory: self.logical_is_dir(index),
                score,
                highlight,
            });
        }
        results
    }

    /// Enumerate a directory path.
    pub fn enumerate(
        &self,
        path: &str,
        query: &str,
        recursive: bool,
        limit: usize,
    ) -> Result<Vec<EsSearchResult>> {
        self.enumerate_inner(path, query, recursive, limit, None)
    }

    /// Enumerate a directory path while observing cancellation.
    pub fn enumerate_with_cancel(
        &self,
        path: &str,
        query: &str,
        recursive: bool,
        limit: usize,
        cancel: &AtomicBool,
    ) -> Result<Vec<EsSearchResult>> {
        self.enumerate_inner(path, query, recursive, limit, Some(cancel))
    }

    fn enumerate_inner(
        &self,
        path: &str,
        query: &str,
        recursive: bool,
        limit: usize,
        cancel: Option<&AtomicBool>,
    ) -> Result<Vec<EsSearchResult>> {
        if limit == 0 || is_cancelled(cancel) {
            return Ok(Vec::new());
        }
        let Some(root) = self.path_to_idx_inner(path, cancel)? else {
            return Ok(Vec::new());
        };
        let candidates = if recursive {
            let mut collected = Vec::new();
            let Some(mut stack) = self.logical_children_inner(root, cancel) else {
                return Ok(Vec::new());
            };
            while let Some(child) = stack.pop() {
                if collected.len() % 1024 == 0 && is_cancelled(cancel) {
                    return Ok(Vec::new());
                }
                collected.push(child);
                if self.logical_is_dir(child) {
                    let Some(children) = self.logical_children_inner(child, cancel) else {
                        return Ok(Vec::new());
                    };
                    stack.extend(children);
                }
            }
            collected
        } else {
            let Some(children) = self.logical_children_inner(root, cancel) else {
                return Ok(Vec::new());
            };
            children
        };

        let query_folded = crate::search::fold::fold_text(query);
        let mut results = Vec::new();
        for (position, index) in candidates.into_iter().enumerate() {
            if position % 1024 == 0 && is_cancelled(cancel) {
                return Ok(Vec::new());
            }
            if self.logical_is_deleted(index) {
                continue;
            }
            let Some(name) = self.logical_name(index) else {
                continue;
            };
            let is_dir = self.logical_is_dir(index);
            let Some((score, highlight)) = score_name(&query_folded, &name, is_dir) else {
                continue;
            };
            results.push(EsSearchResult {
                path: self.logical_path(index)?,
                name,
                is_directory: is_dir,
                score,
                highlight,
            });
        }
        if is_cancelled(cancel) {
            return Ok(Vec::new());
        }
        sort_and_limit(&mut results, limit);
        Ok(results)
    }

    /// Resolve a normalized path to a logical record index.
    pub fn path_to_idx(&self, path: &str) -> Result<u32> {
        let normalized = normalize_path_for_lookup(path);
        self.path_to_idx_inner(&normalized, None)?
            .ok_or(EsError::PathNotFound { path: normalized })
    }

    /// Resolve a path by walking directory children one segment at a time.
    ///
    /// Returning `Ok(None)` means the traversal was cancelled.
    fn path_to_idx_inner(&self, path: &str, cancel: Option<&AtomicBool>) -> Result<Option<u32>> {
        let normalized = normalize_path_for_lookup(path);
        let mut parts = normalized.split('\\').filter(|part| !part.is_empty());
        let Some(root_name) = parts.next() else {
            return Err(EsError::PathNotFound { path: normalized });
        };

        if is_cancelled(cancel) {
            return Ok(None);
        }

        let root_matches = |index: u32| {
            !self.logical_is_deleted(index)
                && self.logical_parent(index).is_none()
                && self
                    .logical_name_ref(index)
                    .is_some_and(|name| names_equal_case_insensitive(name, root_name))
        };

        // NTFS uses file reference 5 for the volume root. Keep a fallback for
        // synthetic indexes and malformed/legacy caches.
        let mut current = self.ref_lookup(5).filter(|&index| root_matches(index));
        if current.is_none() {
            for index in 0..self.logical_len() {
                if index % 1024 == 0 && is_cancelled(cancel) {
                    return Ok(None);
                }
                if root_matches(index) {
                    current = Some(index);
                    break;
                }
            }
        }
        let Some(mut current) = current else {
            return Err(EsError::PathNotFound { path: normalized });
        };

        for part in parts {
            if is_cancelled(cancel) {
                return Ok(None);
            }
            let Some(children) = self.logical_children_inner(current, cancel) else {
                return Ok(None);
            };
            let mut next = None;
            for (position, child) in children.into_iter().enumerate() {
                if position % 1024 == 0 && is_cancelled(cancel) {
                    return Ok(None);
                }
                if self.logical_is_deleted(child) {
                    continue;
                }
                if self
                    .logical_name_ref(child)
                    .is_some_and(|name| names_equal_case_insensitive(name, part))
                {
                    next = Some(child);
                    break;
                }
            }
            let Some(child) = next else {
                return Err(EsError::PathNotFound { path: normalized });
            };
            current = child;
        }

        Ok(Some(current))
    }

    /// Apply a batch of USN-derived events to the delta overlay.
    pub fn apply_events(&mut self, events: &[EsUsnEvent]) {
        for event in events {
            match event.kind {
                EsUsnEventKind::Delete => self.apply_delete(event.file_ref),
                EsUsnEventKind::Create => self.apply_create(event),
                EsUsnEventKind::Rename | EsUsnEventKind::Move => self.apply_rename_move(event),
                EsUsnEventKind::Metadata => {}
            }
        }
    }

    fn apply_delete(&mut self, file_ref: u64) {
        if let Some(index) = self.ref_lookup(file_ref) {
            self.delta.deleted.insert(index);
            // O(1) tombstone in the overlay; the base map stays immutable.
            self.delta
                .ref_overrides
                .insert(mft_record_number(file_ref), None);
        }
    }

    fn apply_create(&mut self, event: &EsUsnEvent) {
        let Some(name) = event.name.clone() else {
            return;
        };
        if name.is_empty() {
            return;
        }
        let parent_ref = event.parent_ref.unwrap_or(0);
        let flags = event.flags.unwrap_or(0);
        let logical = self.logical_len();
        self.delta.inserted.push(InsertedRecord {
            file_ref: event.file_ref,
            parent_ref,
            name,
            flags,
        });
        self.delta.deleted.remove(&logical);
        // O(1) overlay override pointing the (possibly reused) file reference
        // at the new logical index.
        self.delta
            .ref_overrides
            .insert(mft_record_number(event.file_ref), Some(logical));
    }

    fn apply_rename_move(&mut self, event: &EsUsnEvent) {
        let Some(index) = self.ref_lookup(event.file_ref) else {
            self.apply_create(event);
            return;
        };
        if let Some(name) = &event.name {
            if !name.is_empty() {
                self.set_logical_name(index, name.clone());
            }
        }
        if let Some(parent_ref) = event.parent_ref {
            if let Some(new_parent) = self.ref_lookup(parent_ref) {
                self.set_logical_parent(index, new_parent);
            }
        }
    }

    fn set_logical_name(&mut self, index: u32, name: String) {
        if index < self.base_len() {
            self.delta.renamed.insert(index, name);
        } else if let Some(pos) = index.checked_sub(self.base_len()) {
            if let Some(rec) = self
                .delta
                .inserted
                .get_mut(usize::try_from(pos).unwrap_or(usize::MAX))
            {
                rec.name = name;
            }
        }
    }

    fn set_logical_parent(&mut self, index: u32, parent: u32) {
        self.delta.moved.insert(index, parent);
    }
}

/// Bounded insert: keep at most `cap` entries, dropping the lowest-scored.
fn is_cancelled(cancel: Option<&AtomicBool>) -> bool {
    cancel.is_some_and(|token| token.load(Ordering::Relaxed))
}

fn names_equal_case_insensitive(left: &str, right: &str) -> bool {
    if left.is_ascii() && right.is_ascii() {
        left.eq_ignore_ascii_case(right)
    } else {
        crate::search::fold::fold_text(left) == crate::search::fold::fold_text(right)
    }
}

fn insert_top_n(
    heap: &mut Vec<(u32, Vec<[u32; 2]>, u32)>,
    entry: (u32, Vec<[u32; 2]>, u32),
    cap: usize,
) {
    if heap.len() < cap {
        heap.push(entry);
    } else if let Some(min) = heap.iter().map(|(s, _, _)| *s).min() {
        if entry.0 > min {
            if let Some(pos) = heap.iter().position(|(s, _, _)| *s == min) {
                heap[pos] = entry;
            }
        }
    }
}

fn sort_and_limit(results: &mut Vec<EsSearchResult>, limit: usize) {
    results.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.path.to_lowercase().cmp(&right.path.to_lowercase()))
    });
    results.truncate(limit);
}

fn join_path_parts(parts: &[String]) -> String {
    if let Some(root) = parts.first() {
        if crate::path::is_drive_prefix(root) {
            let mut path = root.to_ascii_uppercase();
            path.push('\\');
            for part in parts.iter().skip(1) {
                if !path.ends_with('\\') {
                    path.push('\\');
                }
                path.push_str(part);
            }
            return path;
        }
    }
    parts.join(r"\")
}

/// Build a lookup map from child index to parent index.
#[must_use]
pub fn parent_map(records: &[EsRecord]) -> BTreeMap<u32, u32> {
    records
        .iter()
        .enumerate()
        .filter_map(|(idx, record)| {
            if record.parent_idx == PARENT_NONE {
                None
            } else {
                u32::try_from(idx)
                    .ok()
                    .map(|record_idx| (record_idx, record.parent_idx))
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::builder::EsIndexBuilder;
    use crate::record::flags;
    use crate::usn::{EsUsnEvent, EsUsnEventKind};
    use std::sync::atomic::AtomicBool;

    #[test]
    fn path_from_idx_deep() {
        let mut builder = EsIndexBuilder::new();
        let root = builder
            .add_record(5, u32::MAX, "C:", flags::DIRECTORY, 0)
            .unwrap();
        let users = builder
            .add_record(6, root, "Users", flags::DIRECTORY, 1)
            .unwrap();
        let file = builder.add_record(7, users, "note.txt", 0, 2).unwrap();
        let index = builder.finish().unwrap();
        assert_eq!(index.path_from_idx(file).unwrap(), r"C:\Users\note.txt");
    }

    #[test]
    fn children_enumeration() {
        let mut builder = EsIndexBuilder::new();
        let root = builder
            .add_record(5, u32::MAX, "C:", flags::DIRECTORY, 0)
            .unwrap();
        let child = builder
            .add_record(6, root, "Users", flags::DIRECTORY, 1)
            .unwrap();
        let index = builder.finish().unwrap();
        assert_eq!(index.children(root).unwrap(), &[child]);
    }

    #[test]
    fn cancelled_search_returns_without_results() {
        let mut builder = EsIndexBuilder::new();
        let root = builder
            .add_record(5, u32::MAX, "C:", flags::DIRECTORY, 0)
            .unwrap();
        builder.add_record(6, root, "needle.txt", 0, 1).unwrap();
        let index = builder.finish().unwrap();
        let cancel = AtomicBool::new(true);

        assert!(index.search_with_cancel("needle", 10, &cancel).is_empty());
    }

    #[test]
    fn candidate_collection_keeps_all_matches_beyond_top_n() {
        let mut builder = EsIndexBuilder::new();
        let root = builder
            .add_record(5, u32::MAX, "C:", flags::DIRECTORY, 0)
            .unwrap();
        builder.add_record(6, root, "abc.txt", 0, 1).unwrap();
        builder.add_record(7, root, "abcdef.txt", 0, 1).unwrap();
        builder.add_record(8, root, "prefix-abc.log", 0, 1).unwrap();
        builder.add_record(9, root, "unrelated.txt", 0, 1).unwrap();
        let index = builder.finish().unwrap();
        let cancel = AtomicBool::new(false);

        let initial = index.search_collect_candidates_with_cancel("abc", 1, 100, &cancel);
        let candidates = initial.candidate_ids.unwrap();
        assert_eq!(initial.results.len(), 1);
        assert_eq!(candidates.len(), 3);

        let refined = index.search_candidate_ids_with_cancel("abcd", &candidates, 10, 100, &cancel);
        assert_eq!(refined.candidate_ids.as_ref().unwrap().len(), 1);
        assert_eq!(refined.results, index.search("abcd", 10));
    }

    #[test]
    fn overflowing_candidate_budget_never_returns_a_partial_cache() {
        let mut builder = EsIndexBuilder::new();
        let root = builder
            .add_record(5, u32::MAX, "C:", flags::DIRECTORY, 0)
            .unwrap();
        builder.add_record(6, root, "abc-one", 0, 1).unwrap();
        builder.add_record(7, root, "abc-two", 0, 1).unwrap();
        let index = builder.finish().unwrap();
        let cancel = AtomicBool::new(false);

        let search = index.search_collect_candidates_with_cancel("abc", 10, 1, &cancel);
        assert_eq!(search.results.len(), 2);
        assert!(search.candidate_ids.is_none());
    }

    #[test]
    fn cancelled_candidate_search_is_not_cacheable() {
        let mut builder = EsIndexBuilder::new();
        let root = builder
            .add_record(5, u32::MAX, "C:", flags::DIRECTORY, 0)
            .unwrap();
        builder.add_record(6, root, "needle.txt", 0, 1).unwrap();
        let index = builder.finish().unwrap();
        let cancel = AtomicBool::new(true);

        let search = index.search_collect_candidates_with_cancel("needle", 10, 100, &cancel);
        assert!(search.results.is_empty());
        assert!(search.candidate_ids.is_none());
    }

    #[test]
    fn candidate_scan_observes_overlay_renames() {
        let mut builder = EsIndexBuilder::new();
        let root = builder
            .add_record(5, u32::MAX, "C:", flags::DIRECTORY, 0)
            .unwrap();
        builder.add_record(6, root, "before.txt", 0, 1).unwrap();
        let mut index = builder.finish().unwrap();
        index.apply_events(&[EsUsnEvent {
            kind: EsUsnEventKind::Rename,
            file_ref: 6,
            parent_ref: None,
            name: Some("after-needle.txt".to_string()),
            flags: None,
        }]);
        let cancel = AtomicBool::new(false);

        let search = index.search_collect_candidates_with_cancel("needle", 10, 100, &cancel);
        assert_eq!(search.results.len(), 1);
        assert_eq!(search.results[0].name, "after-needle.txt");
        assert_eq!(search.candidate_ids.unwrap().len(), 1);
    }

    #[test]
    fn segmented_path_lookup_reaches_directory_beyond_early_search_candidates() {
        let mut builder = EsIndexBuilder::new();
        let root = builder
            .add_record(5, u32::MAX, "C:", flags::DIRECTORY, 0)
            .unwrap();
        for index in 0..150_u64 {
            builder
                .add_record(100 + index, root, &format!("filler-{index}.txt"), 0, 1)
                .unwrap();
        }
        let target = builder
            .add_record(1_000, root, "Target", flags::DIRECTORY, 1)
            .unwrap();
        builder
            .add_record(1_001, target, "inside.txt", 0, 2)
            .unwrap();
        let index = builder.finish().unwrap();

        let results = index.enumerate(r"C:\Target", "", false, 50).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, r"C:\Target\inside.txt");
    }

    #[test]
    fn segmented_path_lookup_observes_overlay_directory_rename() {
        let mut builder = EsIndexBuilder::new();
        let root = builder
            .add_record(5, u32::MAX, "C:", flags::DIRECTORY, 0)
            .unwrap();
        let directory = builder
            .add_record(6, root, "Before", flags::DIRECTORY, 1)
            .unwrap();
        builder
            .add_record(7, directory, "inside.txt", 0, 2)
            .unwrap();
        let mut index = builder.finish().unwrap();
        index.apply_events(&[EsUsnEvent {
            kind: EsUsnEventKind::Rename,
            file_ref: 6,
            parent_ref: None,
            name: Some("After".to_string()),
            flags: None,
        }]);

        let results = index.enumerate(r"C:\After", "", false, 50).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, r"C:\After\inside.txt");
    }

    #[test]
    fn cancelled_enumeration_returns_no_partial_results() {
        let mut builder = EsIndexBuilder::new();
        let root = builder
            .add_record(5, u32::MAX, "C:", flags::DIRECTORY, 0)
            .unwrap();
        builder.add_record(6, root, "inside.txt", 0, 1).unwrap();
        let index = builder.finish().unwrap();
        let cancel = AtomicBool::new(true);

        let results = index
            .enumerate_with_cancel(r"C:\", "", false, 50, &cancel)
            .unwrap();

        assert!(results.is_empty());
    }
}
