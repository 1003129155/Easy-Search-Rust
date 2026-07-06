// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Per-drive index manager.
//!
//! Indexes are built by the background worker and installed here once ready.
//! The manager holds the USN cursor for each drive and applies incremental
//! events produced by the poll loop.

use std::path::Path;

use easysearch_core::index::EsIndex;
use easysearch_core::usn::EsUsnEvent;

/// Owns the loaded indexes, one per drive.
#[derive(Debug, Default)]
pub struct DriveManager {
    indexes: Vec<(char, EsIndex)>,
}

impl DriveManager {
    /// Create an empty manager.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Return an iterator over all loaded indexes.
    pub fn indexes(&self) -> impl Iterator<Item = &EsIndex> {
        self.indexes.iter().map(|(_, idx)| idx)
    }

    /// Total number of loaded records across all drives.
    #[must_use]
    pub fn record_count(&self) -> u64 {
        self.indexes()
            .map(|idx| u64::try_from(idx.records_len()).unwrap_or(u64::MAX))
            .sum()
    }

    /// Return loaded drive labels (e.g. `"C:"`, `"D:"`).
    #[must_use]
    pub fn drive_labels(&self) -> Vec<String> {
        self.indexes
            .iter()
            .map(|(letter, _)| format!("{letter}:"))
            .collect()
    }

    /// Return loaded drive letters (uppercase).
    #[must_use]
    pub fn drive_letters(&self) -> Vec<char> {
        self.indexes.iter().map(|(letter, _)| *letter).collect()
    }

    /// Get a reference to the index for a specific drive.
    #[must_use]
    pub fn index_for(&self, drive_letter: char) -> Option<&EsIndex> {
        let letter = drive_letter.to_ascii_uppercase();
        self.indexes
            .iter()
            .find(|(l, _)| *l == letter)
            .map(|(_, idx)| idx)
    }

    /// Remove a drive's index from the manager.
    pub fn remove(&mut self, drive_letter: char) {
        let letter = drive_letter.to_ascii_uppercase();
        self.indexes.retain(|(l, _)| *l != letter);
    }

    /// Install (or replace) the index for `drive_letter`.
    pub fn install(&mut self, drive_letter: char, index: EsIndex) {
        let letter = drive_letter.to_ascii_uppercase();
        self.indexes.retain(|(l, _)| *l != letter);
        self.indexes.push((letter, index));
    }

    /// Return the USN cursor `(journal_id, last_usn)` for a drive, if loaded.
    #[must_use]
    pub fn cursor(&self, drive_letter: char) -> Option<(u64, i64)> {
        let letter = drive_letter.to_ascii_uppercase();
        self.indexes
            .iter()
            .find(|(l, _)| *l == letter)
            .map(|(_, idx)| (idx.status.journal_id, idx.status.last_usn))
    }

    /// Apply incremental USN events to a drive's index and advance its cursor.
    pub fn apply(
        &mut self,
        drive_letter: char,
        events: &[EsUsnEvent],
        new_last_usn: i64,
        journal_id: u64,
    ) {
        let letter = drive_letter.to_ascii_uppercase();
        if let Some((_, index)) = self.indexes.iter_mut().find(|(l, _)| *l == letter) {
            index.apply_events(events);
            index.status.last_usn = new_last_usn;
            index.status.journal_id = journal_id;
        }
    }

    /// Search across all loaded indexes.
    pub fn search(&self, query: &str, limit: usize) -> Vec<easysearch_core::EsSearchResult> {
        let mut all_results = Vec::new();
        for (_, index) in &self.indexes {
            let results = index.search(query, limit);
            all_results.extend(results);
        }
        // Prefer stronger matches first, then shallower/shorter paths so
        // broad queries keep predictable, user-near results near the top.
        all_results.sort_unstable_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| path_depth(&left.path).cmp(&path_depth(&right.path)))
                .then_with(|| left.path.len().cmp(&right.path.len()))
                .then_with(|| left.path.to_lowercase().cmp(&right.path.to_lowercase()))
        });
        all_results.truncate(limit);
        all_results
    }

    /// Enumerate a directory path across all loaded indexes.
    pub fn enumerate(
        &self,
        path: &str,
        query: &str,
        recursive: bool,
        limit: usize,
    ) -> Result<Vec<easysearch_core::EsSearchResult>, String> {
        for (_, index) in &self.indexes {
            match index.enumerate(path, query, recursive, limit) {
                Ok(results) if !results.is_empty() => return Ok(results),
                Ok(_) => continue,
                Err(_) => continue,
            }
        }
        Ok(Vec::new())
    }
}

fn path_depth(path: &str) -> usize {
    path.chars().filter(|&ch| ch == '\\' || ch == '/').count()
}

/// Build (or hot-load) the [`EsIndex`] for `drive_letter`.
///
/// - Hot-load from a valid `.flowcache` when its journal id still matches the
///   live journal.
/// - Otherwise full MFT rebuild, capturing the current journal head as the
///   starting cursor, then persist the cache.
///
/// On non-Windows this returns an empty index (dev stub).
///
/// # Errors
///
/// Returns a human-readable error string on failure.
pub fn build_index(drive_letter: char, cache_dir: Option<&Path>) -> Result<EsIndex, String> {
    let letter = drive_letter.to_ascii_uppercase();

    #[cfg(windows)]
    {
        build_index_windows(letter, cache_dir)
    }
    #[cfg(not(windows))]
    {
        let _ = (letter, cache_dir);
        Ok(EsIndex::default())
    }
}

#[cfg(windows)]
fn build_index_windows(letter: char, cache_dir: Option<&Path>) -> Result<EsIndex, String> {
    use easysearch_core::cache::{read_flow_cache, write_flow_cache};
    use uffs_mft::platform::DriveLetter;
    use uffs_mft::usn::query_usn_journal;

    let drive = DriveLetter::parse(letter).map_err(|e| e.to_string())?;

    // ── Try hot load from cache (only if the journal id still matches) ────────
    if let Some(dir) = cache_dir {
        if let Some(volume_serial) = probe_volume_serial(letter) {
            match read_flow_cache(dir, volume_serial) {
                Ok(Some(cached)) => match query_usn_journal(drive) {
                    Ok(info) => {
                        if cached.status.journal_id != 0
                            && cached.status.journal_id == info.journal_id
                        {
                            return Ok(cached);
                        }
                        eprintln!(
                            "[easysearch-engine] {letter}: journal id changed (cache {} vs live {}), rebuilding",
                            cached.status.journal_id, info.journal_id
                        );
                    }
                    Err(_) => return Ok(cached),
                },
                Ok(None) => {}
                Err(err) => {
                    eprintln!(
                        "[easysearch-engine] cache invalid for {letter}: ({err}) — rebuilding"
                    );
                }
            }
        }
    }

    // ── Full MFT rebuild ─────────────────────────────────────────────────────
    let mut index = build_index_from_live_mft(letter)?;

    // Capture the current journal head as the starting cursor.
    match query_usn_journal(drive) {
        Ok(info) => {
            index.status.journal_id = info.journal_id;
            index.status.last_usn = info.next_usn.raw();
        }
        Err(err) => {
            eprintln!("[easysearch-engine] {letter}: query_usn_journal failed: {err}");
        }
    }

    // ── Persist to cache (best-effort, non-fatal) ─────────────────────────────
    if let Some(dir) = cache_dir {
        if let Some(volume_serial) = probe_volume_serial(letter) {
            if let Err(err) = write_flow_cache(&index, dir, volume_serial, index.status.journal_id)
            {
                eprintln!("[easysearch-engine] cache write failed for {letter}: {err}");
            }
        }
    }

    Ok(index)
}

/// Build an [`EsIndex`] by reading the live NTFS MFT for `drive_letter`.
#[cfg(windows)]
fn build_index_from_live_mft(drive_letter: char) -> Result<EsIndex, String> {
    use uffs_mft::platform::DriveLetter;

    let letter = DriveLetter::parse(drive_letter).map_err(|err| err.to_string())?;
    let reader = uffs_mft::MftReader::open(letter).map_err(|err| err.to_string())?;
    let mft_index = reader
        .read_all_index_sync()
        .map_err(|err| err.to_string())?;

    build_index_from_mft_index(&mft_index, drive_letter)
}

#[cfg(windows)]
fn build_index_from_mft_index(
    mft_index: &uffs_mft::index::MftIndex,
    drive_letter: char,
) -> Result<EsIndex, String> {
    use easysearch_core::builder::EsIndexBuilder;
    use easysearch_core::record::flags as es_flags;
    use uffs_mft::index::{NO_ENTRY, ROOT_FRS};

    let drive_upper = drive_letter.to_ascii_uppercase();

    struct RowDraft {
        frs: u64,
        parent_frs: u64,
        name: String,
        flags: u16,
    }

    let capacity = mft_index.records.len();
    let mut drafts: Vec<RowDraft> = Vec::with_capacity(capacity);

    for record in &mft_index.records {
        if record.is_extension() || record.is_deleted() || !record.has_name() {
            continue;
        }
        let name_ref = record.first_name.name;
        if !name_ref.is_valid() {
            continue;
        }
        let name_bytes = mft_index.get_name_bytes(name_ref);
        if name_bytes.is_empty() {
            continue;
        }

        let name = String::from_utf8_lossy(name_bytes).into_owned();
        let parent_frs = u64::from(record.first_name.parent_frs);
        let frs = u64::from(record.frs);

        let mut ff: u16 = 0;
        if record.is_directory() {
            ff |= es_flags::DIRECTORY;
        }
        if record.stdinfo.is_hidden() {
            ff |= es_flags::HIDDEN;
        }
        if record.stdinfo.is_system() {
            ff |= es_flags::SYSTEM;
        }

        drafts.push(RowDraft {
            frs,
            parent_frs,
            name,
            flags: ff,
        });
    }

    let drive_root_name = format!("{drive_upper}:");
    let root_frs = ROOT_FRS;
    let no_parent = NO_ENTRY as u64;

    if let Some(root) = drafts.iter_mut().find(|d| d.frs == root_frs) {
        root.name = drive_root_name.clone();
        root.parent_frs = no_parent;
        root.flags |= es_flags::DIRECTORY;
    } else {
        drafts.insert(
            0,
            RowDraft {
                frs: root_frs,
                parent_frs: no_parent,
                name: drive_root_name,
                flags: es_flags::DIRECTORY,
            },
        );
    }

    let mut frs_to_draft: Vec<(u64, usize)> = drafts
        .iter()
        .enumerate()
        .map(|(di, d)| (d.frs, di))
        .collect();
    frs_to_draft.sort_unstable_by_key(|&(frs, _)| frs);

    let lookup_draft = |frs: u64| -> Option<usize> {
        frs_to_draft
            .binary_search_by_key(&frs, |&(f, _)| f)
            .ok()
            .map(|pos| frs_to_draft[pos].1)
    };

    let mut builder = EsIndexBuilder::with_capacity(drafts.len());
    let mut draft_to_flow: Vec<u32> = vec![u32::MAX; drafts.len()];

    let root_di = lookup_draft(root_frs).unwrap_or(0);
    {
        let d = &drafts[root_di];
        let flow_idx = builder
            .add_record(d.frs, u32::MAX, &d.name, d.flags, 0)
            .map_err(|e| e.to_string())?;
        draft_to_flow[root_di] = flow_idx;
    }

    let mut inserted = 1usize;
    let mut prev_inserted = 0usize;
    while inserted != prev_inserted {
        prev_inserted = inserted;
        for di in 0..drafts.len() {
            if draft_to_flow[di] != u32::MAX {
                continue;
            }
            let d = &drafts[di];
            let parent_flow = resolve_parent(
                d.parent_frs,
                d.frs,
                no_parent,
                &lookup_draft,
                &draft_to_flow,
            );
            if parent_flow != u32::MAX || d.parent_frs == no_parent || d.parent_frs == d.frs {
                let fi = builder
                    .add_record(d.frs, parent_flow, &d.name, d.flags, 0)
                    .map_err(|e| e.to_string())?;
                draft_to_flow[di] = fi;
                inserted += 1;
            }
        }
    }

    let root_flow = draft_to_flow[root_di];
    for di in 0..drafts.len() {
        if draft_to_flow[di] == u32::MAX {
            let d = &drafts[di];
            let fi = builder
                .add_record(d.frs, root_flow, &d.name, d.flags, 0)
                .map_err(|e| e.to_string())?;
            draft_to_flow[di] = fi;
        }
    }

    drop(draft_to_flow);
    builder.finish().map_err(|e| e.to_string())
}

#[cfg(windows)]
fn resolve_parent(
    parent_frs: u64,
    self_frs: u64,
    no_parent_sentinel: u64,
    lookup_draft: &impl Fn(u64) -> Option<usize>,
    draft_to_flow: &[u32],
) -> u32 {
    if parent_frs == no_parent_sentinel || parent_frs == self_frs {
        return u32::MAX;
    }
    match lookup_draft(parent_frs) {
        Some(parent_di) => draft_to_flow[parent_di],
        None => u32::MAX,
    }
}

/// Read the NTFS volume serial number for the given drive letter.
#[cfg(windows)]
fn probe_volume_serial(letter: char) -> Option<u64> {
    use windows::Win32::Storage::FileSystem::GetVolumeInformationW;
    use windows::core::PCWSTR;

    let root: Vec<u16> = format!("{letter}:\\\0").encode_utf16().collect();
    let mut serial: u32 = 0;

    #[expect(unsafe_code, reason = "GetVolumeInformationW Win32 FFI")]
    let ok = unsafe {
        GetVolumeInformationW(
            PCWSTR(root.as_ptr()),
            None,
            Some(&mut serial),
            None,
            None,
            None,
        )
    };

    if ok.is_ok() {
        Some(u64::from(serial))
    } else {
        None
    }
}
