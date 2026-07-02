// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Filename search primitives for EasySearch.
//!
//! Strategy:
//! - 1-2 character queries: lightweight [`PrefixIndex`] bucket lookup.
//! - 3+ character queries: parallel linear scan over the contiguous names
//!   blob (Everything-style). CPU-cache-friendly and uses negligible extra
//!   memory beyond the base record/names data.
//! - Path reconstruction: only for top-N results after scoring, not for
//!   every candidate.

pub mod fold;
pub mod postings; // kept as empty placeholder
pub mod prefix;
pub mod trigram; // trigram generation kept for potential future use

use serde::{Deserialize, Serialize};

use self::fold::fold_text;
use self::prefix::PrefixIndex;

/// Search-side indexes derived from the base record/name columns.
///
/// After the memory optimization, only the lightweight [`PrefixIndex`] for
/// 1-2 character queries is retained. Longer queries use direct linear scan.
#[derive(Debug, Clone, Default)]
pub struct EsSearchIndex {
    /// Buckets for one- and two-character queries.
    pub prefix: PrefixIndex,
}

impl EsSearchIndex {
    /// Register a folded basename in the prefix index.
    pub fn add_name(&mut self, record_idx: u32, basename: &str) {
        let folded = fold_text(basename);
        self.prefix.add(&folded, record_idx);
        // No trigram postings — 3+ char queries use linear scan.
    }
}

/// Slim result shape returned by search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EsSearchResult {
    /// Full path to the file or directory.
    pub path: String,
    /// Basename without parent path.
    pub name: String,
    /// Whether this result is a directory.
    pub is_directory: bool,
    /// Higher scores should rank earlier.
    pub score: u32,
    /// Highlight ranges as byte offsets `[start, len]` in `name`.
    pub highlight: Vec<[u32; 2]>,
}

/// Score a basename against a folded query.
#[must_use]
pub fn score_name(
    query_folded: &str,
    name: &str,
    is_directory: bool,
) -> Option<(u32, Vec<[u32; 2]>)> {
    if query_folded.is_empty() {
        return Some((directory_bonus(is_directory), Vec::new()));
    }

    let name_folded = fold_text(name);
    let base_score = if name_folded == query_folded {
        1_000
    } else if name_folded.starts_with(query_folded) {
        900
    } else if name_folded.contains(query_folded) {
        800
    } else {
        return None;
    };

    let highlight = name_folded
        .find(query_folded)
        .map_or_else(Vec::new, |start| {
            vec![[bounded_u32(start), bounded_u32(query_folded.len())]]
        });

    Some((base_score + directory_bonus(is_directory), highlight))
}

/// Return the directory score bonus.
#[must_use]
pub const fn directory_bonus(is_directory: bool) -> u32 {
    if is_directory { 100 } else { 0 }
}

fn bounded_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::score_name;

    #[test]
    fn exact_beats_prefix_by_score_shape() {
        let exact = score_name("abc", "abc", false).unwrap().0;
        let prefix = score_name("abc", "abcdef", false).unwrap().0;
        assert!(exact > prefix);
    }

    #[test]
    fn directory_gets_bonus() {
        let file = score_name("abc", "abc", false).unwrap().0;
        let directory = score_name("abc", "abc", true).unwrap().0;
        assert_eq!(directory - file, 100);
    }
}
