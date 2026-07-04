// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! USN journal polling: read raw NTFS journal records and convert them into
//! platform-agnostic [`EsUsnEvent`]s that `easysearch-core` can apply.

use easysearch_core::usn::EsUsnEvent;

/// Result of one poll: the events to apply, the advanced USN cursor, and the
/// live journal id (which may differ from the caller's if the journal was
/// recreated).
pub(crate) struct PollResult {
    /// Events to apply to the drive's index.
    pub(crate) events: Vec<EsUsnEvent>,
    /// New `last_usn` cursor to persist.
    pub(crate) new_last_usn: i64,
    /// Live journal id.
    pub(crate) journal_id: u64,
}

/// Poll the USN journal for `drive_letter` since `last_usn`.
///
/// Returns `None` when the drive/journal can't be read (non-fatal; the caller
/// simply retries on the next tick).
#[cfg(windows)]
pub(crate) fn poll_drive(drive_letter: char, last_usn: i64) -> Option<PollResult> {
    use uffs_mft::platform::DriveLetter;
    use uffs_mft::usn::{Usn, query_usn_journal, read_usn_journal};

    let drive = DriveLetter::parse(drive_letter).ok()?;
    let info = query_usn_journal(drive).ok()?;

    let start = Usn::new(last_usn);
    let (records, next_usn) = read_usn_journal(drive, info.journal_id, start).ok()?;

    let events = convert_records(&records);
    Some(PollResult {
        events,
        new_last_usn: next_usn.raw(),
        journal_id: info.journal_id,
    })
}

/// Non-Windows stub: journals are unavailable, so nothing to poll.
#[cfg(not(windows))]
pub(crate) fn poll_drive(_drive_letter: char, _last_usn: i64) -> Option<PollResult> {
    None
}

/// Convert aggregated USN records into events.
#[cfg(windows)]
fn convert_records(records: &[uffs_mft::usn::UsnRecord]) -> Vec<EsUsnEvent> {
    use std::collections::HashMap;

    use easysearch_core::usn::EsUsnEventKind;
    use uffs_mft::usn::aggregate_changes;

    let mut attributes: HashMap<u64, u32> = HashMap::new();
    for record in records {
        attributes.insert(record.frs.raw(), record.file_attributes);
    }

    let changes = aggregate_changes(records);
    let mut events = Vec::with_capacity(changes.len());
    for (frs, change) in &changes {
        let file_ref = frs.raw();
        if change.deleted {
            events.push(EsUsnEvent {
                kind: EsUsnEventKind::Delete,
                file_ref,
                parent_ref: None,
                name: None,
                flags: None,
            });
        } else if change.created {
            let attrs = attributes.get(&file_ref).copied().unwrap_or(0);
            events.push(EsUsnEvent {
                kind: EsUsnEventKind::Create,
                file_ref,
                parent_ref: Some(change.parent_frs.raw()),
                name: Some(change.filename.clone()),
                flags: Some(flags_from_attributes(attrs)),
            });
        } else if change.renamed {
            events.push(EsUsnEvent {
                kind: EsUsnEventKind::Rename,
                file_ref,
                parent_ref: Some(change.parent_frs.raw()),
                name: Some(change.filename.clone()),
                flags: None,
            });
        }
    }
    events
}

/// Map raw NTFS `FILE_ATTRIBUTE_*` flags to record flags.
#[cfg(windows)]
fn flags_from_attributes(attributes: u32) -> u16 {
    use easysearch_core::record::flags as es_flags;

    const ATTR_DIRECTORY: u32 = 0x0000_0010;
    const ATTR_HIDDEN: u32 = 0x0000_0002;
    const ATTR_SYSTEM: u32 = 0x0000_0004;

    let mut flags: u16 = 0;
    if attributes & ATTR_DIRECTORY != 0 {
        flags |= es_flags::DIRECTORY;
    }
    if attributes & ATTR_HIDDEN != 0 {
        flags |= es_flags::HIDDEN;
    }
    if attributes & ATTR_SYSTEM != 0 {
        flags |= es_flags::SYSTEM;
    }
    flags
}
