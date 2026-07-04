// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! USN event DTOs consumed by the delta overlay.

/// Cursor identifying a point in one NTFS USN journal.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EsUsnCursor {
    /// USN journal id.
    pub journal_id: u64,
    /// Last applied USN.
    pub last_usn: i64,
}

/// Coarse event kind required by the overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EsUsnEventKind {
    /// New file or directory was created.
    Create,
    /// File or directory was deleted.
    Delete,
    /// Basename changed.
    Rename,
    /// Parent directory changed.
    Move,
    /// Metadata changed but search-visible fields did not.
    Metadata,
}

/// Minimal USN event after aggregation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EsUsnEvent {
    /// Event kind.
    pub kind: EsUsnEventKind,
    /// File reference affected by the event.
    pub file_ref: u64,
    /// New parent file reference for move/create events.
    pub parent_ref: Option<u64>,
    /// New basename for create/rename events.
    pub name: Option<String>,
    /// New flags when known.
    pub flags: Option<u16>,
}
