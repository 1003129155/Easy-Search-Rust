// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! USN journal polling: read raw NTFS journal records and convert them into
//! platform-agnostic [`EsUsnEvent`]s that `easysearch-core` can apply.

use easysearch_core::usn::EsUsnEvent;

/// Result of one poll: the events to apply, the advanced USN cursor, and the
/// live journal id (which may differ from the caller's if the journal was
/// recreated).
pub struct PollResult {
    /// Events to apply to the drive's index.
    pub events: Vec<EsUsnEvent>,
    /// New `last_usn` cursor to persist.
    pub new_last_usn: i64,
    /// Live journal id.
    pub journal_id: u64,
}

/// Poll the USN journal for `drive_letter` since `last_usn`.
///
/// Returns `Ok(PollResult)` on success, or `Err(reason)` when the
/// drive/journal can't be read (non-fatal; the caller retries on next tick).
///
/// When the saved cursor (`last_usn`) points to a journal entry that has
/// already been reclaimed (`ERROR_JOURNAL_ENTRY_DELETED`, OS error 1181),
/// the cursor is automatically advanced to the journal's current
/// `first_usn`. This means any changes between the old cursor and the new
/// `first_usn` are lost — but polling resumes correctly instead of being
/// stuck forever.
#[cfg(windows)]
pub fn poll_drive(drive_letter: char, last_usn: i64) -> Result<PollResult, String> {
    use uffs_mft::platform::DriveLetter;
    use uffs_mft::usn::{Usn, query_usn_journal, read_usn_journal};

    let drive = DriveLetter::parse(drive_letter)
        .map_err(|e| format!("DriveLetter::parse failed: {e}"))?;

    let info = query_usn_journal(drive)
        .map_err(|e| format!("query_usn_journal failed: {e}"))?;

    let start = Usn::new(last_usn);
    match read_usn_journal(drive, info.journal_id, start) {
        Ok((records, next_usn)) => {
            let events = convert_records(&records);
            Ok(PollResult {
                events,
                new_last_usn: next_usn.raw(),
                journal_id: info.journal_id,
            })
        }
        Err(e) => {
            // ERROR_JOURNAL_ENTRY_DELETED (1181): the saved cursor is too old,
            // the journal has already reclaimed entries past our last_usn.
            // Recovery: advance cursor to the journal's current first_usn so
            // subsequent polls succeed. Some intermediate changes are lost but
            // the alternative is polling being stuck forever.
            const ERROR_JOURNAL_ENTRY_DELETED: i32 = 1181;
            if e.raw_os_error() == Some(ERROR_JOURNAL_ENTRY_DELETED) {
                eprintln!(
                    "[easysearch-engine] {drive_letter}: journal cursor too old \
                     (last_usn={last_usn}, journal first_usn={}). Advancing cursor.",
                    info.first_usn.raw()
                );
                // Try reading from the journal's current first valid USN
                let recovery_start = info.first_usn;
                match read_usn_journal(drive, info.journal_id, recovery_start) {
                    Ok((records, next_usn)) => {
                        let events = convert_records(&records);
                        Ok(PollResult {
                            events,
                            new_last_usn: next_usn.raw(),
                            journal_id: info.journal_id,
                        })
                    }
                    Err(e2) => {
                        // Even recovery read failed — advance to next_usn
                        // so at least we stop retrying the dead range.
                        eprintln!(
                            "[easysearch-engine] {drive_letter}: recovery read also failed: {e2}. \
                             Advancing cursor to next_usn={}.",
                            info.next_usn.raw()
                        );
                        Ok(PollResult {
                            events: Vec::new(),
                            new_last_usn: info.next_usn.raw(),
                            journal_id: info.journal_id,
                        })
                    }
                }
            } else {
                Err(format!("read_usn_journal failed: {e}"))
            }
        }
    }
}

/// Non-Windows stub: journals are unavailable, so nothing to poll.
#[cfg(not(windows))]
pub fn poll_drive(_drive_letter: char, _last_usn: i64) -> Result<PollResult, String> {
    Err("USN journal polling not available on non-Windows".to_string())
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
