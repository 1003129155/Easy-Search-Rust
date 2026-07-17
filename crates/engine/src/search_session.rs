// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Per-input-session candidate caching for incremental filename search.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};

use easysearch_core::EsSearchResult;

use crate::drive_manager::DriveManager;

const MIN_CACHED_QUERY_CHARS: usize = 3;
const MAX_SNAPSHOTS: usize = 4;
const MAX_CACHED_CANDIDATE_BYTES: usize = 32 * 1024 * 1024;
const CANDIDATE_ID_BYTES: usize = size_of::<u32>();
const MAX_CACHED_CANDIDATES: usize = MAX_CACHED_CANDIDATE_BYTES / CANDIDATE_ID_BYTES;

/// Search path used by the most recent session query.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SearchSessionMode {
    /// The normal index path was used for a one- or two-character query.
    #[default]
    PrefixIndex,
    /// No reusable snapshot existed, so all logical records were scanned.
    FullScan,
    /// A stricter query filtered a broader cached candidate set.
    CandidateFilter,
    /// An exact cached snapshot was reused, typically after Backspace.
    Snapshot,
}

/// Candidate IDs for one drive.
#[derive(Debug)]
pub(crate) struct DriveCandidateSet {
    pub(crate) drive: char,
    pub(crate) ids: Vec<u32>,
}

/// Output from a cache-aware multi-drive scan.
pub(crate) struct CandidateSearchOutput {
    pub(crate) results: Vec<EsSearchResult>,
    pub(crate) drives: Option<Vec<DriveCandidateSet>>,
}

#[derive(Debug)]
struct CandidateSnapshot {
    query: String,
    generation: u64,
    drives: Vec<DriveCandidateSet>,
    candidate_count: usize,
}

/// Stateful candidate cache owned by one logical input/search session.
///
/// A session is memory-only. It stores complete candidate sets for up to four
/// recent query prefixes and discards them on index generation changes, short
/// queries, explicit reset, or an unrelated edit.
#[derive(Debug, Default)]
pub struct SearchSession {
    snapshots: VecDeque<CandidateSnapshot>,
    candidate_count: usize,
    last_mode: SearchSessionMode,
}

impl SearchSession {
    /// Create an empty search session.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Drop all cached candidates.
    pub fn reset(&mut self) {
        self.snapshots.clear();
        self.candidate_count = 0;
        self.last_mode = SearchSessionMode::PrefixIndex;
    }

    /// Return the path used by the most recent query.
    #[must_use]
    pub const fn last_mode(&self) -> SearchSessionMode {
        self.last_mode
    }

    /// Total number of cached record IDs across all retained snapshots.
    #[must_use]
    pub const fn cached_candidate_count(&self) -> usize {
        self.candidate_count
    }

    pub(crate) fn search(
        &mut self,
        manager: &DriveManager,
        query: &str,
        limit: usize,
        cancel: &AtomicBool,
    ) -> Vec<EsSearchResult> {
        if query.chars().count() < MIN_CACHED_QUERY_CHARS {
            self.reset();
            return manager.search_with_cancel(query, limit, cancel);
        }

        let generation = manager.generation();
        if self
            .snapshots
            .front()
            .is_some_and(|snapshot| snapshot.generation != generation)
        {
            self.reset();
        }

        let exact = self
            .snapshots
            .iter()
            .rposition(|snapshot| snapshot.generation == generation && snapshot.query == query);
        let broader = self
            .snapshots
            .iter()
            .enumerate()
            .filter(|(_, snapshot)| {
                snapshot.generation == generation
                    && query.starts_with(&snapshot.query)
                    && query != snapshot.query
            })
            .max_by_key(|(_, snapshot)| snapshot.query.len())
            .map(|(index, _)| index);

        let (mode, output) = if let Some(index) = exact {
            let source = &self.snapshots[index].drives;
            (
                SearchSessionMode::Snapshot,
                manager.search_candidates(
                    query,
                    Some(source),
                    limit,
                    MAX_CACHED_CANDIDATES,
                    cancel,
                ),
            )
        } else if let Some(index) = broader {
            let source = &self.snapshots[index].drives;
            (
                SearchSessionMode::CandidateFilter,
                manager.search_candidates(
                    query,
                    Some(source),
                    limit,
                    MAX_CACHED_CANDIDATES,
                    cancel,
                ),
            )
        } else {
            // A middle edit or unrelated query cannot safely reuse substring
            // candidates from the previous branch.
            self.reset();
            (
                SearchSessionMode::FullScan,
                manager.search_candidates(query, None, limit, MAX_CACHED_CANDIDATES, cancel),
            )
        };

        if cancel.load(Ordering::Relaxed) {
            return Vec::new();
        }

        self.last_mode = mode;
        if exact.is_none()
            && let Some(drives) = output.drives
        {
            self.push_snapshot(query, generation, drives);
        }
        output.results
    }

    fn push_snapshot(&mut self, query: &str, generation: u64, drives: Vec<DriveCandidateSet>) {
        let candidate_count = drives.iter().map(|drive| drive.ids.len()).sum::<usize>();
        if candidate_count > MAX_CACHED_CANDIDATES {
            return;
        }

        while self.snapshots.len() >= MAX_SNAPSHOTS
            || self.candidate_count.saturating_add(candidate_count) > MAX_CACHED_CANDIDATES
        {
            let Some(evicted) = self.snapshots.pop_front() else {
                break;
            };
            self.candidate_count = self.candidate_count.saturating_sub(evicted.candidate_count);
        }

        self.candidate_count = self.candidate_count.saturating_add(candidate_count);
        self.snapshots.push_back(CandidateSnapshot {
            query: query.to_owned(),
            generation,
            drives,
            candidate_count,
        });
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;

    use easysearch_core::builder::EsIndexBuilder;
    use easysearch_core::record::flags;

    use super::{DriveCandidateSet, MAX_SNAPSHOTS, SearchSession, SearchSessionMode};
    use crate::DriveManager;

    fn test_index(drive: char) -> easysearch_core::index::EsIndex {
        let mut builder = EsIndexBuilder::new();
        let root = builder
            .add_record(5, u32::MAX, &format!("{drive}:"), flags::DIRECTORY, 0)
            .unwrap();
        builder.add_record(6, root, "abc.txt", 0, 1).unwrap();
        builder.add_record(7, root, "abcdef.txt", 0, 1).unwrap();
        builder.add_record(8, root, "prefix-abc.log", 0, 1).unwrap();
        builder.finish().unwrap()
    }

    #[test]
    fn extension_filters_candidates_and_backspace_restores_snapshot() {
        let mut manager = DriveManager::new();
        manager.install('C', test_index('C'));
        let mut session = SearchSession::new();
        let cancel = AtomicBool::new(false);

        let initial = session.search(&manager, "abc", 10, &cancel);
        assert_eq!(initial.len(), 3);
        assert_eq!(session.last_mode(), SearchSessionMode::FullScan);

        let extended = session.search(&manager, "abcd", 10, &cancel);
        assert_eq!(extended.len(), 1);
        assert_eq!(session.last_mode(), SearchSessionMode::CandidateFilter);

        let restored = session.search(&manager, "abc", 10, &cancel);
        assert_eq!(restored, initial);
        assert_eq!(session.last_mode(), SearchSessionMode::Snapshot);
    }

    #[test]
    fn index_generation_change_invalidates_snapshots() {
        let mut manager = DriveManager::new();
        manager.install('C', test_index('C'));
        let mut session = SearchSession::new();
        let cancel = AtomicBool::new(false);

        session.search(&manager, "abc", 10, &cancel);
        manager.install('C', test_index('C'));
        session.search(&manager, "abc", 10, &cancel);

        assert_eq!(session.last_mode(), SearchSessionMode::FullScan);
    }

    #[test]
    fn short_query_releases_candidate_memory() {
        let mut manager = DriveManager::new();
        manager.install('C', test_index('C'));
        let mut session = SearchSession::new();
        let cancel = AtomicBool::new(false);

        session.search(&manager, "abc", 10, &cancel);
        assert!(session.cached_candidate_count() > 0);
        session.search(&manager, "ab", 10, &cancel);

        assert_eq!(session.cached_candidate_count(), 0);
        assert_eq!(session.last_mode(), SearchSessionMode::PrefixIndex);
    }

    #[test]
    fn snapshot_count_is_bounded() {
        let mut session = SearchSession::new();
        for index in 0..=MAX_SNAPSHOTS {
            session.push_snapshot(
                &format!("query-{index}"),
                1,
                vec![DriveCandidateSet {
                    drive: 'C',
                    ids: vec![u32::try_from(index).unwrap()],
                }],
            );
        }

        assert_eq!(session.snapshots.len(), MAX_SNAPSHOTS);
        assert_eq!(session.cached_candidate_count(), MAX_SNAPSHOTS);
    }

    #[test]
    fn cancelled_scan_does_not_commit_snapshot() {
        let mut manager = DriveManager::new();
        manager.install('C', test_index('C'));
        let mut session = SearchSession::new();
        let cancel = AtomicBool::new(true);

        assert!(session.search(&manager, "abc", 10, &cancel).is_empty());
        assert_eq!(session.cached_candidate_count(), 0);
    }
}
