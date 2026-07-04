// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Header for the `.flowcache` format.

use core::mem::size_of;

use crate::error::{EsError, Result};
use crate::record::ES_RECORD_BYTES;

/// Magic bytes at the start of every `.flowcache`.
pub const ES_CACHE_MAGIC: [u8; 8] = *b"EZSEARCH";

/// Current `.flowcache` format version.
pub const ES_CACHE_VERSION: u32 = 1;

/// Fixed byte size of [`EsCacheHeader`].
pub const ES_CACHE_HEADER_BYTES: usize = 112;

/// Fixed header at the front of a `.flowcache` file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct EsCacheHeader {
    /// Magic bytes, always [`ES_CACHE_MAGIC`].
    pub magic: [u8; 8],
    /// Cache format version.
    pub version: u32,
    /// Header size in bytes.
    pub header_bytes: u32,
    /// Size of each record row.
    pub record_bytes: u32,
    /// Explicit padding before aligned u64 fields.
    #[expect(
        clippy::pub_underscore_fields,
        reason = "bytemuck Pod requires all fields same visibility"
    )]
    pub _pad0: u32,
    /// NTFS volume serial number.
    pub volume_serial: u64,
    /// USN journal id captured by this cache.
    pub usn_journal_id: u64,
    /// Last applied USN.
    pub last_usn: i64,
    /// Number of records in the cache.
    pub record_count: u32,
    /// Explicit padding before aligned u64 fields.
    #[expect(
        clippy::pub_underscore_fields,
        reason = "bytemuck Pod requires all fields same visibility"
    )]
    pub _pad1: u32,
    /// Number of bytes in the names blob.
    pub name_bytes: u64,
    /// File offset of the records column.
    pub records_offset: u64,
    /// File offset of the names blob.
    pub names_offset: u64,
    /// File offset of child CSR offsets.
    pub children_offsets_offset: u64,
    /// File offset of child CSR indices.
    pub children_indices_offset: u64,
    /// File offset of file-reference lookup data.
    pub file_ref_map_offset: u64,
    /// File offset of search index data.
    pub search_index_offset: u64,
}

const _: () = assert!(size_of::<EsCacheHeader>() == ES_CACHE_HEADER_BYTES);

impl EsCacheHeader {
    /// Create a header with invariant fields initialized.
    #[must_use]
    pub const fn new(volume_serial: u64, usn_journal_id: u64, last_usn: i64) -> Self {
        Self {
            magic: ES_CACHE_MAGIC,
            version: ES_CACHE_VERSION,
            header_bytes: ES_CACHE_HEADER_BYTES as u32,
            record_bytes: ES_RECORD_BYTES as u32,
            _pad0: 0,
            volume_serial,
            usn_journal_id,
            last_usn,
            record_count: 0,
            _pad1: 0,
            name_bytes: 0,
            records_offset: 0,
            names_offset: 0,
            children_offsets_offset: 0,
            children_indices_offset: 0,
            file_ref_map_offset: 0,
            search_index_offset: 0,
        }
    }

    /// Validate invariant fields.
    pub fn validate(self) -> Result<()> {
        if self.magic != ES_CACHE_MAGIC {
            return Err(EsError::CacheHeader {
                reason: "magic mismatch",
            });
        }
        if self.version != ES_CACHE_VERSION {
            return Err(EsError::CacheHeader {
                reason: "version mismatch",
            });
        }
        if self.header_bytes != ES_CACHE_HEADER_BYTES as u32 {
            return Err(EsError::CacheHeader {
                reason: "header size mismatch",
            });
        }
        if self.record_bytes != ES_RECORD_BYTES as u32 {
            return Err(EsError::CacheHeader {
                reason: "record size mismatch",
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use core::mem::size_of;

    use super::{ES_CACHE_HEADER_BYTES, EsCacheHeader};

    #[test]
    fn header_size_is_stable() {
        assert_eq!(size_of::<EsCacheHeader>(), ES_CACHE_HEADER_BYTES);
    }

    #[test]
    fn new_header_validates() {
        EsCacheHeader::new(1, 2, 3).validate().unwrap();
    }
}
