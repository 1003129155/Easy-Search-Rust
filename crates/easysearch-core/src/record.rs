// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Fixed-width row stored in the EasySearch cache.

use core::mem::size_of;

/// Fixed byte size of [`EsRecord`].
pub const ES_RECORD_BYTES: usize = 24;

/// Sentinel parent index used for volume roots and orphaned records.
pub const PARENT_NONE: u32 = u32::MAX;

/// Bit flags stored in [`EsRecord::flags`].
pub mod flags {
    /// Record is a directory.
    pub const DIRECTORY: u16 = 0x0001;
    /// Record was deleted in the delta overlay.
    pub const TOMBSTONE: u16 = 0x0002;
    /// Record has the Windows hidden attribute.
    pub const HIDDEN: u16 = 0x0004;
    /// Record has the Windows system attribute.
    pub const SYSTEM: u16 = 0x0008;
    /// Record type could not be classified.
    pub const UNKNOWN_TYPE: u16 = 0x0010;
}

/// Minimal per-file row for EasySearch filename search.
///
/// The row stores only a basename reference and parent pointer. Full paths are
/// reconstructed by walking `parent_idx` immediately before returning results.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct EsRecord {
    /// NTFS file reference number used to match USN events.
    pub file_ref: u64,
    /// Byte offset into the names blob.
    pub name_offset: u32,
    /// Parent record index, or [`PARENT_NONE`] for a root.
    pub parent_idx: u32,
    /// UTF-8 byte length of the basename.
    pub name_len: u16,
    /// Record flags; see [`flags`].
    pub flags: u16,
    /// Interned extension id; zero means no extension.
    pub ext_id: u16,
    /// Low-cost tie-breaker rank, lower is better.
    pub rank: u16,
}

const _: () = assert!(size_of::<EsRecord>() == ES_RECORD_BYTES);

impl EsRecord {
    /// Returns `true` when this row represents a directory.
    #[inline]
    #[must_use]
    pub const fn is_directory(self) -> bool {
        self.flags & flags::DIRECTORY != 0
    }

    /// Returns `true` when this row is logically deleted.
    #[inline]
    #[must_use]
    pub const fn is_tombstone(self) -> bool {
        self.flags & flags::TOMBSTONE != 0
    }

    /// Returns the MFT record number portion of [`EsRecord::file_ref`].
    #[inline]
    #[must_use]
    pub const fn mft_record_number(self) -> u64 {
        mft_record_number(self.file_ref)
    }
}

/// Extract the low 48-bit MFT record number from an NTFS file reference.
#[inline]
#[must_use]
pub const fn mft_record_number(file_ref: u64) -> u64 {
    file_ref & 0x0000_FFFF_FFFF_FFFF
}

#[cfg(test)]
mod tests {
    use core::mem::size_of;

    use super::{ES_RECORD_BYTES, EsRecord, flags, mft_record_number};

    #[test]
    fn record_size_is_24() {
        assert_eq!(size_of::<EsRecord>(), ES_RECORD_BYTES);
    }

    #[test]
    fn directory_flag_is_detected() {
        let record = EsRecord {
            flags: flags::DIRECTORY,
            ..EsRecord::default()
        };
        assert!(record.is_directory());
    }

    #[test]
    fn file_reference_masks_sequence_number() {
        let file_ref = (7_u64 << 48) | 0x3039;
        assert_eq!(mft_record_number(file_ref), 12_345);
    }
}
