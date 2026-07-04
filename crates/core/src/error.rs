// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Error types for EasySearch core.

/// Convenient result alias used by `easysearch-core`.
pub type Result<T> = core::result::Result<T, EsError>;

/// Errors produced while building, loading, or querying an index.
#[derive(Debug)]
pub enum EsError {
    /// Record index was outside the records column.
    RecordIndexOutOfRange {
        /// Requested record index.
        index: u32,
        /// Number of records present in the index.
        len: usize,
    },

    /// A record points outside the names blob.
    InvalidNameRange {
        /// Record whose name range failed validation.
        index: u32,
        /// Byte offset into the names blob.
        offset: u32,
        /// Byte length inside the names blob.
        len: u16,
    },

    /// A name slice was not valid UTF-8.
    InvalidNameUtf8(core::str::Utf8Error),

    /// A path could not be resolved to a record.
    PathNotFound {
        /// Normalized lookup path.
        path: String,
    },

    /// Parent-chain traversal detected a cycle.
    ParentCycle {
        /// Record where traversal became cyclic or exceeded index length.
        index: u32,
    },

    /// A filename exceeded the fixed record field width.
    NameTooLong {
        /// Byte length that did not fit in `u16`.
        len: usize,
    },

    /// The names blob exceeded the fixed record field width.
    NamesBlobTooLarge {
        /// Byte length that did not fit in `u32`.
        len: usize,
    },

    /// The record column exceeded the fixed child-index field width.
    RecordCountTooLarge {
        /// Record count that did not fit in `u32`.
        len: usize,
    },

    /// Cache header validation failed.
    CacheHeader {
        /// Human-readable validation failure.
        reason: &'static str,
    },

    /// MFT read or parse error.
    MftRead {
        /// Human-readable detail from the underlying error.
        detail: String,
    },

    /// Cache file I/O error.
    CacheIo {
        /// Human-readable detail from the underlying error.
        detail: String,
    },
}

impl core::fmt::Display for EsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::RecordIndexOutOfRange { index, len } => {
                write!(f, "record index {index} is outside records length {len}")
            }
            Self::InvalidNameRange { index, offset, len } => {
                write!(f, "record {index} has invalid name range offset={offset} len={len}")
            }
            Self::InvalidNameUtf8(e) => write!(f, "record name is not valid UTF-8: {e}"),
            Self::PathNotFound { path } => write!(f, "path not found in index: {path}"),
            Self::ParentCycle { index } => {
                write!(f, "parent-chain cycle while resolving record {index}")
            }
            Self::NameTooLong { len } => {
                write!(f, "filename is too long for EsRecord name_len: {len} bytes")
            }
            Self::NamesBlobTooLarge { len } => {
                write!(f, "names blob is too large for EsRecord name_offset: {len} bytes")
            }
            Self::RecordCountTooLarge { len } => {
                write!(f, "record count is too large for u32 indices: {len}")
            }
            Self::CacheHeader { reason } => write!(f, "invalid cache header: {reason}"),
            Self::MftRead { detail } => write!(f, "MFT read failed: {detail}"),
            Self::CacheIo { detail } => write!(f, "cache I/O error: {detail}"),
        }
    }
}

impl std::error::Error for EsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidNameUtf8(e) => Some(e),
            _ => None,
        }
    }
}

impl From<core::str::Utf8Error> for EsError {
    fn from(e: core::str::Utf8Error) -> Self {
        Self::InvalidNameUtf8(e)
    }
}
