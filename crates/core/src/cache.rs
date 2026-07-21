// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! `.flowcache` file read / write.
//!
//! # File layout
//!
//! ```text
//! [UFFSENC v2 envelope (AES-256-GCM)]
//! [EsCacheHeader    112 bytes]
//! [records          record_count × 24 bytes]
//! [names            name_bytes bytes]
//! [children_offsets (record_count + 1) × 4 bytes]
//! [children_indices N × 4 bytes]
//! [file_ref_map     varint-encoded (frs, idx) pairs]
//! [search_index     (reserved, written as 0 bytes for now)]
//! ```
//!
//! # Atomic write
//!
//! The plaintext payload is encrypted with AES-256-GCM before it is written.
//! Writes go to `<name>.tmp` then rename to `<name>` so a crash during write
//! never leaves a partial cache visible to a reader.

use std::fs;
use std::io::Write;
use std::mem::size_of;
use std::path::{Path, PathBuf};

use bytemuck::{bytes_of, cast_slice, pod_read_unaligned};

use crate::cache_header::{ES_CACHE_HEADER_BYTES, EsCacheHeader};
use crate::error::{EsError, Result};
use crate::index::{EsIndex, FileRefMap};
use crate::record::{ES_RECORD_BYTES, EsRecord};
use crate::search::EsSearchIndex;
use crate::status::EsIndexStatus;

#[cfg(not(test))]
fn cache_key() -> std::io::Result<[u8; 32]> {
    uffs_security::keystore::get_cache_key()
}

// Cache unit tests should be hermetic and must not create or mutate the user's
// platform keystore. The crypto module separately tests real key management.
#[cfg(test)]
fn cache_key() -> std::io::Result<[u8; 32]> {
    Ok([0xA5; 32])
}

/// Copy-and-cast `bytes` into a `Vec<T>` without requiring alignment.
fn pod_collect_unaligned<T: bytemuck::Pod>(bytes: &[u8]) -> Vec<T> {
    let size = size_of::<T>();
    debug_assert_eq!(bytes.len() % size, 0, "bytes not a multiple of T");
    let count = bytes.len() / size;
    let mut out: Vec<T> = Vec::with_capacity(count);
    for chunk in bytes.chunks_exact(size) {
        out.push(pod_read_unaligned(chunk));
    }
    out
}

/// Return the cache filename for an NTFS volume serial.
#[must_use]
pub fn cache_file_name(volume_serial: u64) -> String {
    format!("{volume_serial:016x}.flowcache")
}

/// Validate a header read from disk.
pub fn validate_cache_header(header: EsCacheHeader) -> Result<()> {
    header.validate()
}

// ── Write ─────────────────────────────────────────────────────────────────────

/// Persist `index` to `cache_dir / {volume_serial}.flowcache` atomically.
///
/// Writes to a `.tmp` file first, then renames over the final path.
///
/// # Errors
///
/// Returns [`EsError::CacheIo`] on any I/O failure.
pub fn write_flow_cache(
    index: &EsIndex,
    cache_dir: &Path,
    volume_serial: u64,
    usn_journal_id: u64,
) -> Result<()> {
    fs::create_dir_all(cache_dir).map_err(|e| EsError::CacheIo {
        detail: format!("create cache dir: {e}"),
    })?;

    let final_path = cache_dir.join(cache_file_name(volume_serial));
    let tmp_path = tmp_path(&final_path);

    let mut plaintext = Vec::new();
    write_to(&mut plaintext, index, volume_serial, usn_journal_id)?;
    let key = cache_key().map_err(|e| EsError::CacheIo {
        detail: format!("get cache encryption key: {e}"),
    })?;
    let encrypted =
        uffs_security::crypto::encrypt_cache(&plaintext, &key).map_err(|e| EsError::CacheIo {
            detail: format!("encrypt cache: {e}"),
        })?;

    {
        let mut file = fs::File::create(&tmp_path).map_err(|e| EsError::CacheIo {
            detail: format!("create tmp: {e}"),
        })?;
        write_all(&mut file, &encrypted)?;
        file.flush().map_err(|e| EsError::CacheIo {
            detail: format!("flush tmp: {e}"),
        })?;
    }

    fs::rename(&tmp_path, &final_path).map_err(|e| EsError::CacheIo {
        detail: format!("rename tmp: {e}"),
    })?;

    Ok(())
}

fn tmp_path(final_path: &Path) -> PathBuf {
    let mut p = final_path.to_owned();
    let mut name = p.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    p.set_file_name(name);
    p
}

fn write_to(
    out: &mut impl Write,
    index: &EsIndex,
    volume_serial: u64,
    usn_journal_id: u64,
) -> Result<()> {
    let record_count = u32::try_from(index.records.len()).map_err(|_| EsError::CacheHeader {
        reason: "record count overflows u32",
    })?;
    let name_bytes = u64::try_from(index.names.len()).map_err(|_| EsError::CacheHeader {
        reason: "names blob overflows u64",
    })?;

    // ── Compute section offsets ───────────────────────────────────────────────
    let records_offset = ES_CACHE_HEADER_BYTES as u64;
    let names_offset = records_offset + (u64::from(record_count) * ES_RECORD_BYTES as u64);
    let children_offsets_offset = names_offset + name_bytes;
    let co_count = u64::from(record_count).saturating_add(1);
    let children_offsets_bytes = co_count * 4;
    let children_indices_offset = children_offsets_offset + children_offsets_bytes;
    let ci_count = u64::try_from(index.children_indices.len()).unwrap_or(u64::MAX);
    let children_indices_bytes = ci_count * 4;
    let file_ref_map_offset = children_indices_offset + children_indices_bytes;

    let fref_bytes = encode_file_ref_map(&index.file_ref_map);
    let search_index_offset =
        file_ref_map_offset + u64::try_from(fref_bytes.len()).unwrap_or(u64::MAX);

    // ── Header ───────────────────────────────────────────────────────────────
    let mut header = EsCacheHeader::new(volume_serial, usn_journal_id, index.status.last_usn);
    header.record_count = record_count;
    header.name_bytes = name_bytes;
    header.records_offset = records_offset;
    header.names_offset = names_offset;
    header.children_offsets_offset = children_offsets_offset;
    header.children_indices_offset = children_indices_offset;
    header.file_ref_map_offset = file_ref_map_offset;
    header.search_index_offset = search_index_offset;

    write_all(out, bytes_of(&header))?;
    write_all(out, cast_slice::<EsRecord, u8>(&index.records))?;
    write_all(out, &index.names)?;
    write_all(out, cast_slice::<u32, u8>(&index.children_offsets))?;
    write_all(out, cast_slice::<u32, u8>(&index.children_indices))?;
    write_all(out, &fref_bytes)?;
    Ok(())
}

fn write_all(out: &mut impl Write, data: &[u8]) -> Result<()> {
    out.write_all(data).map_err(|e| EsError::CacheIo {
        detail: format!("write: {e}"),
    })
}

// ── Read ──────────────────────────────────────────────────────────────────────

/// Load an [`EsIndex`] from `cache_dir / {volume_serial}.flowcache`.
///
/// Returns `None` if the file does not exist (trigger a full rebuild).
/// Returns `Err` if the file exists but is corrupt or version-mismatched.
///
/// # Errors
///
/// Returns [`EsError`] on I/O or validation failure.
pub fn read_flow_cache(cache_dir: &Path, volume_serial: u64) -> Result<Option<EsIndex>> {
    let path = cache_dir.join(cache_file_name(volume_serial));
    if !path.exists() {
        return Ok(None);
    }
    let stored = fs::read(&path).map_err(|e| EsError::CacheIo {
        detail: format!("read {}: {e}", path.display()),
    })?;

    let legacy_plaintext = stored.starts_with(&crate::cache_header::ES_CACHE_MAGIC);
    let plaintext = if legacy_plaintext {
        stored
    } else {
        let key = cache_key().map_err(|e| EsError::CacheIo {
            detail: format!("get cache decryption key: {e}"),
        })?;
        match uffs_security::crypto::decrypt_cache(&stored, &key) {
            Ok(data) => data,
            Err(e) => {
                let _ = fs::remove_file(&path);
                return Err(EsError::CacheIo {
                    detail: format!("decrypt {}: {e}", path.display()),
                });
            }
        }
    };

    let index = parse_cache_bytes(&plaintext, volume_serial)?;
    if legacy_plaintext {
        if let Err(error) =
            write_flow_cache(&index, cache_dir, volume_serial, index.status.journal_id)
        {
            tracing::warn!(
                path = %path.display(),
                error = %error,
                "failed to migrate plaintext flow cache to encrypted format"
            );
        }
    }
    Ok(Some(index))
}

fn parse_cache_bytes(data: &[u8], volume_serial: u64) -> Result<EsIndex> {
    if data.len() < ES_CACHE_HEADER_BYTES {
        return Err(EsError::CacheHeader {
            reason: "file too small for header",
        });
    }

    let header: EsCacheHeader = pod_read_unaligned(&data[..ES_CACHE_HEADER_BYTES]);
    header.validate()?;

    if header.volume_serial != volume_serial {
        return Err(EsError::CacheHeader {
            reason: "volume serial mismatch",
        });
    }

    let record_count = header.record_count as usize;
    let name_bytes = header.name_bytes as usize;

    // ── Records ───────────────────────────────────────────────────────────────
    let rec_start = header.records_offset as usize;
    let rec_end = rec_start + record_count * ES_RECORD_BYTES;
    let rec_bytes = data.get(rec_start..rec_end).ok_or(EsError::CacheHeader {
        reason: "records section out of bounds",
    })?;
    let records: Vec<EsRecord> = pod_collect_unaligned(rec_bytes);

    // ── Names ─────────────────────────────────────────────────────────────────
    let names_start = header.names_offset as usize;
    let names_end = names_start + name_bytes;
    let names = data
        .get(names_start..names_end)
        .ok_or(EsError::CacheHeader {
            reason: "names section out of bounds",
        })?
        .to_vec();

    // ── Children offsets (CSR, record_count + 1 entries) ─────────────────────
    let co_start = header.children_offsets_offset as usize;
    let co_count = record_count.saturating_add(1);
    let co_end = co_start + co_count * 4;
    let co_bytes = data.get(co_start..co_end).ok_or(EsError::CacheHeader {
        reason: "children_offsets out of bounds",
    })?;
    let children_offsets: Vec<u32> = pod_collect_unaligned(co_bytes);

    // ── Children indices ──────────────────────────────────────────────────────
    let ci_start = header.children_indices_offset as usize;
    let ci_end = header.file_ref_map_offset as usize;
    if ci_end < ci_start {
        return Err(EsError::CacheHeader {
            reason: "children_indices bounds inverted",
        });
    }
    let ci_bytes = data.get(ci_start..ci_end).ok_or(EsError::CacheHeader {
        reason: "children_indices out of bounds",
    })?;
    let children_indices: Vec<u32> = pod_collect_unaligned(ci_bytes);

    // ── FileRefMap ────────────────────────────────────────────────────────────
    let fm_start = header.file_ref_map_offset as usize;
    let fm_end = header.search_index_offset as usize;
    if fm_end < fm_start {
        return Err(EsError::CacheHeader {
            reason: "file_ref_map bounds inverted",
        });
    }
    let fm_bytes = data.get(fm_start..fm_end).ok_or(EsError::CacheHeader {
        reason: "file_ref_map out of bounds",
    })?;
    let file_ref_map = decode_file_ref_map(fm_bytes)?;

    // ── Search index (rebuild in-memory from records + names) ─────────────────
    let mut search = EsSearchIndex::default();
    for (idx, record) in records.iter().enumerate() {
        let Ok(record_idx) = u32::try_from(idx) else {
            continue;
        };
        let start = record.name_offset as usize;
        let end = start + record.name_len as usize;
        if let Some(bytes) = names.get(start..end) {
            if let Ok(name) = core::str::from_utf8(bytes) {
                search.add_name(record_idx, name);
            }
        }
    }

    let records_count = u64::try_from(records.len()).unwrap_or(u64::MAX);
    let mut status = EsIndexStatus::ready(records_count, header.last_usn);
    status.journal_id = header.usn_journal_id;

    Ok(EsIndex::from_parts(
        records,
        names,
        children_offsets,
        children_indices,
        file_ref_map,
        search,
        status,
    ))
}

// ── FileRefMap encoding ───────────────────────────────────────────────────────

fn encode_file_ref_map(map: &FileRefMap) -> Vec<u8> {
    let pairs = map.pairs();
    let count = u32::try_from(pairs.len()).unwrap_or(u32::MAX);
    let mut out = Vec::with_capacity(4 + pairs.len() * 12);
    out.extend_from_slice(&count.to_le_bytes());
    for (frs, idx) in &pairs {
        out.extend_from_slice(&frs.to_le_bytes());
        out.extend_from_slice(&idx.to_le_bytes());
    }
    out
}

fn decode_file_ref_map(data: &[u8]) -> Result<FileRefMap> {
    if data.len() < 4 {
        return Ok(FileRefMap::Empty);
    }
    let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let expected = 4 + count * 12;
    if data.len() < expected {
        return Err(EsError::CacheHeader {
            reason: "file_ref_map data truncated",
        });
    }
    let mut pairs: Vec<(u64, u32)> = Vec::with_capacity(count);
    for i in 0..count {
        let base = 4 + i * 12;
        let frs = u64::from_le_bytes(data[base..base + 8].try_into().map_err(|_| {
            EsError::CacheHeader {
                reason: "file_ref_map frs slice",
            }
        })?);
        let idx = u32::from_le_bytes(data[base + 8..base + 12].try_into().map_err(|_| {
            EsError::CacheHeader {
                reason: "file_ref_map idx slice",
            }
        })?);
        pairs.push((frs, idx));
    }
    Ok(FileRefMap::from_pairs(&pairs))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::builder::EsIndexBuilder;
    use crate::record::flags;

    fn build_small_index() -> EsIndex {
        let mut b = EsIndexBuilder::new();
        let root = b
            .add_record(5, u32::MAX, "C:", flags::DIRECTORY, 0)
            .unwrap();
        let users = b.add_record(6, root, "Users", flags::DIRECTORY, 1).unwrap();
        b.add_record(7, users, "readme.txt", 0, 2).unwrap();
        b.finish().unwrap()
    }

    #[test]
    fn cache_name_is_hex_serial() {
        assert_eq!(cache_file_name(0xABCD), "000000000000abcd.flowcache");
    }

    #[test]
    fn roundtrip_write_then_read() {
        let dir = TempDir::new().unwrap();
        let idx = build_small_index();
        let volume_serial = 0x1234_5678_u64;

        write_flow_cache(&idx, dir.path(), volume_serial, 0).unwrap();

        let stored = fs::read(dir.path().join(cache_file_name(volume_serial))).unwrap();
        assert!(matches!(
            uffs_security::crypto::detect_format(&stored),
            uffs_security::crypto::CacheFormat::Encrypted
        ));
        assert!(!stored.windows(10).any(|bytes| bytes == b"readme.txt"));

        let loaded = read_flow_cache(dir.path(), volume_serial)
            .unwrap()
            .expect("cache file should exist after write");

        assert_eq!(loaded.records_len(), idx.records_len());
        assert_eq!(loaded.names, idx.names);
        assert_eq!(loaded.children_offsets, idx.children_offsets);
        assert_eq!(loaded.children_indices, idx.children_indices);
    }

    #[test]
    fn missing_cache_returns_none() {
        let dir = TempDir::new().unwrap();
        let result = read_flow_cache(dir.path(), 0x9999).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn legacy_plaintext_is_migrated_after_read() {
        let dir = TempDir::new().unwrap();
        let idx = build_small_index();
        let serial = 0xCAFE_BABE_u64;
        let path = dir.path().join(cache_file_name(serial));
        let mut plaintext = Vec::new();
        write_to(&mut plaintext, &idx, serial, 9).unwrap();
        fs::write(&path, plaintext).unwrap();

        let loaded = read_flow_cache(dir.path(), serial).unwrap().unwrap();
        assert_eq!(loaded.records_len(), idx.records_len());
        let migrated = fs::read(path).unwrap();
        assert!(matches!(
            uffs_security::crypto::detect_format(&migrated),
            uffs_security::crypto::CacheFormat::Encrypted
        ));
    }

    #[test]
    fn wrong_serial_returns_error() {
        let dir = TempDir::new().unwrap();
        let idx = build_small_index();
        write_flow_cache(&idx, dir.path(), 0xAAAA, 0).unwrap();
        let result = read_flow_cache(dir.path(), 0xBBBB);
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn paths_survive_roundtrip() {
        let dir = TempDir::new().unwrap();
        let idx = build_small_index();
        let serial = 0xDEAD_BEEF_u64;

        write_flow_cache(&idx, dir.path(), serial, 0).unwrap();
        let loaded = read_flow_cache(dir.path(), serial).unwrap().unwrap();

        let results = loaded.search("readme", 10);
        assert!(!results.is_empty());
        assert_eq!(results[0].path, r"C:\Users\readme.txt");
    }

    #[test]
    fn file_ref_map_survives_roundtrip() {
        let dir = TempDir::new().unwrap();
        let idx = build_small_index();
        let serial = 0xCAFE_u64;

        write_flow_cache(&idx, dir.path(), serial, 0).unwrap();
        let loaded = read_flow_cache(dir.path(), serial).unwrap().unwrap();

        assert_eq!(loaded.file_ref_map.get(5), idx.file_ref_map.get(5));
        assert_eq!(loaded.file_ref_map.get(6), idx.file_ref_map.get(6));
        assert_eq!(loaded.file_ref_map.get(7), idx.file_ref_map.get(7));
    }
}
