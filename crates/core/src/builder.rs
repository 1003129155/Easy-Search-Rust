// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Builder for in-memory indexes.

use crate::error::{EsError, Result};
use crate::index::{EsIndex, FileRefMap};
use crate::record::EsRecord;
use crate::search::EsSearchIndex;
use crate::status::EsIndexStatus;

/// Incremental builder for [`EsIndex`].
#[derive(Debug, Default)]
pub struct EsIndexBuilder {
    records: Vec<EsRecord>,
    names: Vec<u8>,
    file_ref_pairs: Vec<(u64, u32)>,
}

impl EsIndexBuilder {
    /// Create an empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a builder with pre-allocated capacity.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            records: Vec::with_capacity(capacity),
            names: Vec::with_capacity(capacity.saturating_mul(20)),
            file_ref_pairs: Vec::with_capacity(capacity),
        }
    }

    /// Add one record and return its record index.
    pub fn add_record(
        &mut self,
        file_ref: u64,
        parent_idx: u32,
        name: &str,
        flags: u16,
        rank: u16,
    ) -> Result<u32> {
        let name_offset =
            u32::try_from(self.names.len()).map_err(|_| EsError::NamesBlobTooLarge {
                len: self.names.len(),
            })?;
        let name_len =
            u16::try_from(name.len()).map_err(|_| EsError::NameTooLong { len: name.len() })?;
        let idx = u32::try_from(self.records.len()).map_err(|_| EsError::RecordCountTooLarge {
            len: self.records.len(),
        })?;

        self.names.extend_from_slice(name.as_bytes());
        self.records.push(EsRecord {
            file_ref,
            name_offset,
            parent_idx,
            name_len,
            flags,
            ext_id: 0,
            rank,
        });
        self.file_ref_pairs.push((file_ref, idx));
        Ok(idx)
    }

    /// Finish all derived columns and return a searchable index.
    pub fn finish(self) -> Result<EsIndex> {
        let mut children: Vec<Vec<u32>> = vec![Vec::new(); self.records.len()];
        for (idx, record) in self.records.iter().enumerate() {
            let Ok(record_idx) = u32::try_from(idx) else {
                return Err(EsError::RecordCountTooLarge {
                    len: self.records.len(),
                });
            };
            if record.parent_idx == u32::MAX {
                continue;
            }
            let parent_idx =
                usize::try_from(record.parent_idx).map_err(|_| EsError::RecordIndexOutOfRange {
                    index: record.parent_idx,
                    len: self.records.len(),
                })?;
            let Some(bucket) = children.get_mut(parent_idx) else {
                return Err(EsError::RecordIndexOutOfRange {
                    index: record.parent_idx,
                    len: self.records.len(),
                });
            };
            bucket.push(record_idx);
        }

        let mut children_offsets = Vec::with_capacity(self.records.len().saturating_add(1));
        let mut children_indices = Vec::new();
        children_offsets.push(0);
        for bucket in children {
            children_indices.extend(bucket);
            let offset = u32::try_from(children_indices.len()).map_err(|_| {
                EsError::RecordCountTooLarge {
                    len: children_indices.len(),
                }
            })?;
            children_offsets.push(offset);
        }

        let mut search = EsSearchIndex::default();
        for idx in 0..self.records.len() {
            let record_idx = u32::try_from(idx).map_err(|_| EsError::RecordCountTooLarge {
                len: self.records.len(),
            })?;
            let record = self.records[idx];
            let start =
                usize::try_from(record.name_offset).map_err(|_| EsError::InvalidNameRange {
                    index: record_idx,
                    offset: record.name_offset,
                    len: record.name_len,
                })?;
            let end = start.checked_add(usize::from(record.name_len)).ok_or(
                EsError::InvalidNameRange {
                    index: record_idx,
                    offset: record.name_offset,
                    len: record.name_len,
                },
            )?;
            let bytes = self
                .names
                .get(start..end)
                .ok_or(EsError::InvalidNameRange {
                    index: record_idx,
                    offset: record.name_offset,
                    len: record.name_len,
                })?;
            let name = core::str::from_utf8(bytes)?;
            search.add_name(record_idx, name);
        }

        let records_count = u64::try_from(self.records.len()).unwrap_or(u64::MAX);
        Ok(EsIndex::from_parts(
            self.records,
            self.names,
            children_offsets,
            children_indices,
            FileRefMap::from_pairs(&self.file_ref_pairs),
            search,
            EsIndexStatus::ready(records_count, 0),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::EsIndexBuilder;
    use crate::record::flags;

    #[test]
    fn search_returns_full_path() {
        let mut builder = EsIndexBuilder::new();
        let root = builder
            .add_record(5, u32::MAX, "C:", flags::DIRECTORY, 0)
            .unwrap();
        builder.add_record(6, root, "abc.txt", 0, 1).unwrap();
        let index = builder.finish().unwrap();
        let results = index.search("abc", 10);
        assert_eq!(results[0].path, r"C:\abc.txt");
    }

    #[test]
    fn file_ref_lookup() {
        let mut builder = EsIndexBuilder::new();
        let root = builder
            .add_record(5, u32::MAX, "C:", flags::DIRECTORY, 0)
            .unwrap();
        let index = builder.finish().unwrap();
        assert_eq!(index.file_ref_map.get(5), Some(root));
    }
}
