// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Prefix buckets for short filename queries.

use std::collections::BTreeMap;

/// Lightweight prefix bucket index used for one- and two-character queries.
#[derive(Debug, Clone, Default)]
pub struct PrefixIndex {
    buckets: BTreeMap<String, Vec<u32>>,
}

impl PrefixIndex {
    /// Add `record_idx` under the first two folded characters of `name`.
    pub fn add(&mut self, name: &str, record_idx: u32) {
        let key: String = name.chars().take(2).collect();
        if !key.is_empty() {
            self.buckets.entry(key).or_default().push(record_idx);
        }
    }

    /// Return candidates whose bucket starts with `query`.
    #[must_use]
    pub fn candidates(&self, query: &str) -> Vec<u32> {
        self.buckets
            .iter()
            .filter(|(key, _)| key.starts_with(query))
            .flat_map(|(_, values)| values.iter().copied())
            .collect()
    }
}
