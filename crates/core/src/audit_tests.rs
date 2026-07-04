// Copyright (c) 2025-2026 LIJIALU. MIT License.
// Comprehensive audit test suite for easysearch-core

#![cfg(test)]

use core::mem::size_of;

use crate::builder::EsIndexBuilder;
use crate::index::{EsIndex, FileRefMap};
use crate::path::{is_drive_prefix, is_drive_root, normalize_path_for_lookup};
use crate::record::{ES_RECORD_BYTES, EsRecord, flags, mft_record_number};
use crate::search::{fold::fold_text, score_name};

// ─── Record & flags ──────────────────────────────────────────────────────────

#[test]
fn tombstone_flag_detected() {
    let r = EsRecord { flags: flags::TOMBSTONE, ..Default::default() };
    assert!(r.is_tombstone());
    assert!(!r.is_directory());
}

#[test]
fn hidden_system_flags_dont_affect_dir_or_tombstone() {
    let r = EsRecord { flags: flags::HIDDEN | flags::SYSTEM, ..Default::default() };
    assert!(!r.is_directory());
    assert!(!r.is_tombstone());
}

#[test]
fn dir_and_tombstone_flags_can_coexist() {
    let r = EsRecord { flags: flags::DIRECTORY | flags::TOMBSTONE, ..Default::default() };
    assert!(r.is_directory());
    assert!(r.is_tombstone());
}

#[test]
fn mft_record_number_strips_sequence() {
    let file_ref = (3_u64 << 48) | 0x1234;
    assert_eq!(mft_record_number(file_ref), 0x1234);
}

#[test]
fn record_size_const_matches_struct() {
    assert_eq!(size_of::<EsRecord>(), ES_RECORD_BYTES);
}

// ─── Path helpers ────────────────────────────────────────────────────────────

#[test]
fn drive_prefix_detection() {
    assert!(is_drive_prefix("C:"));
    assert!(is_drive_prefix("d:"));
    assert!(!is_drive_prefix("C:\\"));
    assert!(!is_drive_prefix("C"));
    assert!(!is_drive_prefix(""));
}

#[test]
fn drive_root_detection() {
    assert!(is_drive_root(r"C:\"));
    assert!(is_drive_root("C:/"));
    assert!(!is_drive_root("C:"));
    assert!(!is_drive_root(r"C:\Users"));
}

#[test]
fn normalize_drive_root_various_inputs() {
    assert_eq!(normalize_path_for_lookup("c:/"), r"C:\");
    assert_eq!(normalize_path_for_lookup(r"C:\"), r"C:\");
    assert_eq!(normalize_path_for_lookup("c:"), r"C:\");
}

#[test]
fn normalize_strips_trailing_slash() {
    assert_eq!(normalize_path_for_lookup(r"C:\Users\"), r"C:\Users");
}

#[test]
fn normalize_converts_forward_slashes() {
    assert_eq!(normalize_path_for_lookup("C:/Users/foo"), r"C:\Users\foo");
}

// ─── Builder ─────────────────────────────────────────────────────────────────

#[test]
fn builder_produces_correct_children_csr() {
    let mut b = EsIndexBuilder::new();
    let root = b.add_record(1, u32::MAX, "C:", flags::DIRECTORY, 0).unwrap();
    let users = b.add_record(2, root, "Users", flags::DIRECTORY, 1).unwrap();
    let docs  = b.add_record(3, root, "docs", flags::DIRECTORY, 1).unwrap();
    let f1    = b.add_record(4, users, "a.txt", 0, 2).unwrap();
    let idx   = b.finish().unwrap();
    let root_children = idx.children(root).unwrap();
    assert!(root_children.contains(&users));
    assert!(root_children.contains(&docs));
    assert!(!root_children.contains(&f1));
    let users_children = idx.children(users).unwrap();
    assert_eq!(users_children, &[f1]);
}

#[test]
fn builder_leaf_has_no_children() {
    let mut b = EsIndexBuilder::new();
    let root = b.add_record(1, u32::MAX, "C:", flags::DIRECTORY, 0).unwrap();
    let f = b.add_record(2, root, "x.txt", 0, 1).unwrap();
    let idx = b.finish().unwrap();
    assert_eq!(idx.children(f).unwrap(), &[] as &[u32]);
}

#[test]
fn path_from_idx_single_root() {
    let mut b = EsIndexBuilder::new();
    let root = b.add_record(1, u32::MAX, "C:", flags::DIRECTORY, 0).unwrap();
    let idx = b.finish().unwrap();
    assert_eq!(idx.path_from_idx(root).unwrap(), r"C:\");
}

#[test]
fn path_from_idx_three_levels_deep() {
    let mut b = EsIndexBuilder::new();
    let root = b.add_record(1, u32::MAX, "C:", flags::DIRECTORY, 0).unwrap();
    let users = b.add_record(2, root, "Users", flags::DIRECTORY, 1).unwrap();
    let admin = b.add_record(3, users, "Admin", flags::DIRECTORY, 2).unwrap();
    let file  = b.add_record(4, admin, "readme.txt", 0, 3).unwrap();
    let idx   = b.finish().unwrap();
    assert_eq!(idx.path_from_idx(file).unwrap(), r"C:\Users\Admin\readme.txt");
}

// ─── FileRefMap ───────────────────────────────────────────────────────────────

#[test]
fn file_ref_map_empty_returns_none() {
    let m = FileRefMap::Empty;
    assert!(m.get(42).is_none());
}

#[test]
fn file_ref_map_dense_roundtrip() {
    let pairs: Vec<(u64, u32)> = (0u64..10).map(|i| (i, i as u32 * 2)).collect();
    let m = FileRefMap::from_pairs(&pairs);
    for (fr, expected_idx) in &pairs {
        assert_eq!(m.get(*fr), Some(*expected_idx));
    }
}

#[test]
fn file_ref_map_with_sequence_number_in_file_ref() {
    let file_ref = (7_u64 << 48) | 123;
    let m = FileRefMap::from_pairs(&[(file_ref, 99)]);
    assert_eq!(m.get(file_ref), Some(99));
    let file_ref_v2 = (9_u64 << 48) | 123;
    assert_eq!(m.get(file_ref_v2), Some(99));
}

#[test]
fn file_ref_map_insert_and_remove() {
    let mut m = FileRefMap::from_pairs(&[(1, 0), (2, 1)]);
    m.insert(3, 2);
    assert_eq!(m.get(3), Some(2));
    m.remove(1);
    assert!(m.get(1).is_none());
    assert_eq!(m.get(2), Some(1));
}

#[test]
fn file_ref_map_sparse_falls_back_to_sorted() {
    let pairs: Vec<(u64, u32)> = vec![(1, 0), (1_000_000, 1)];
    let m = FileRefMap::from_pairs(&pairs);
    assert_eq!(m.get(1), Some(0));
    assert_eq!(m.get(1_000_000), Some(1));
    assert!(m.get(500_000).is_none());
}

// ─── Search / scoring ────────────────────────────────────────────────────────

#[test]
fn score_exact_gt_prefix_gt_substring() {
    let exact     = score_name("abc", "abc",    false).unwrap().0;
    let prefix    = score_name("abc", "abcdef", false).unwrap().0;
    let substring = score_name("abc", "xabcx",  false).unwrap().0;
    assert!(exact > prefix, "exact={exact} prefix={prefix}");
    assert!(prefix > substring, "prefix={prefix} substring={substring}");
}

#[test]
fn score_case_insensitive() {
    let lower = score_name("abc", "ABC", false).unwrap().0;
    let exact  = score_name("abc", "abc", false).unwrap().0;
    assert_eq!(lower, exact);
}

#[test]
fn score_no_match_returns_none() {
    assert!(score_name("xyz", "abc", false).is_none());
}

#[test]
fn score_empty_query_always_matches() {
    let r = score_name("", "anything.txt", false);
    assert!(r.is_some());
}

#[test]
fn highlight_covers_match_position() {
    let (_, hl) = score_name("abc", "xyzabc", false).unwrap();
    assert_eq!(hl.len(), 1);
    let [start, len] = hl[0];
    assert_eq!(len, 3);
    assert_eq!(start, 3);
}

#[test]
fn fold_text_lowercases_ascii() {
    assert_eq!(fold_text("ABC"), "abc");
    assert_eq!(fold_text("Hello.TXT"), "hello.txt");
}

// ─── EsIndex search integration ──────────────────────────────────────────────

fn build_test_index() -> EsIndex {
    let mut b = EsIndexBuilder::new();
    let root   = b.add_record(1, u32::MAX, "C:", flags::DIRECTORY, 0).unwrap();
    let users  = b.add_record(2, root,  "Users",    flags::DIRECTORY, 1).unwrap();
    let admin  = b.add_record(3, users, "admin",    flags::DIRECTORY, 2).unwrap();
    let _docs  = b.add_record(4, admin, "Documents",flags::DIRECTORY, 3).unwrap();
    let _notes = b.add_record(5, admin, "notes.txt",0, 4).unwrap();
    let _code  = b.add_record(6, root,  "code",     flags::DIRECTORY, 1).unwrap();
    let _rf    = b.add_record(7, root,  "readme.md",0, 2).unwrap();
    b.finish().unwrap()
}

#[test]
fn search_exact_match_first() {
    let idx = build_test_index();
    let results = idx.search("admin", 10);
    assert!(!results.is_empty());
    assert_eq!(results[0].name, "admin");
}

#[test]
fn search_returns_no_tombstones() {
    let mut b = EsIndexBuilder::new();
    let root = b.add_record(1, u32::MAX, "C:", flags::DIRECTORY, 0).unwrap();
    let file = b.add_record(2, root, "ghost.txt", 0, 1).unwrap();
    let mut idx = b.finish().unwrap();
    idx.delta.mark_deleted(file);
    let results = idx.search("ghost", 10);
    assert!(results.is_empty(), "tombstoned file should not appear");
}

#[test]
fn search_case_insensitive_match() {
    let idx = build_test_index();
    let lower = idx.search("documents", 10);
    let upper = idx.search("DOCUMENTS", 10);
    assert!(!lower.is_empty());
    assert_eq!(lower[0].name, upper[0].name);
}

#[test]
fn search_empty_query_returns_all_within_limit() {
    let idx = build_test_index();
    let results = idx.search("", 3);
    assert_eq!(results.len(), 3);
}

#[test]
fn search_limit_zero_returns_empty() {
    let idx = build_test_index();
    assert!(idx.search("a", 0).is_empty());
}

#[test]
fn search_directories_score_higher_than_files_on_same_name() {
    let mut b = EsIndexBuilder::new();
    let root = b.add_record(1, u32::MAX, "C:", flags::DIRECTORY, 0).unwrap();
    b.add_record(2, root, "src", flags::DIRECTORY, 0).unwrap();
    b.add_record(3, root, "src", 0, 0).unwrap();
    let idx = b.finish().unwrap();
    let results = idx.search("src", 10);
    assert!(results[0].is_directory, "directory should rank above file with same name");
}

// ─── Enumerate ───────────────────────────────────────────────────────────────

#[test]
fn enumerate_flat_lists_direct_children_only() {
    let idx = build_test_index();
    let results = idx.enumerate(r"C:\", "", false, 100).unwrap();
    assert_eq!(results.len(), 3);
    let names: Vec<&str> = results.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"Users"));
    assert!(names.contains(&"code"));
    assert!(names.contains(&"readme.md"));
}

#[test]
fn enumerate_recursive_finds_all_descendants() {
    let idx = build_test_index();
    let results = idx.enumerate(r"C:\", "", true, 100).unwrap();
    assert_eq!(results.len(), 6, "expected 6 descendants, got {}: {:?}",
        results.len(), results.iter().map(|r| &r.name).collect::<Vec<_>>());
}

#[test]
fn enumerate_with_query_filters_results() {
    let idx = build_test_index();
    let results = idx.enumerate(r"C:\", "code", false, 100).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "code");
}

#[test]
fn enumerate_nonexistent_path_returns_error() {
    let idx = build_test_index();
    let result = idx.enumerate(r"C:\NonExistent", "", false, 100);
    assert!(result.is_err());
}

#[test]
fn enumerate_limit_zero_returns_empty() {
    let idx = build_test_index();
    let results = idx.enumerate(r"C:\", "", false, 0).unwrap();
    assert!(results.is_empty());
}

#[test]
fn enumerate_path_normalizes_trailing_slash() {
    let idx = build_test_index();
    let with_slash    = idx.enumerate(r"C:\", "", false, 100).unwrap();
    let without_slash = idx.enumerate("C:", "", false, 100).unwrap();
    let names_with:    Vec<&str> = with_slash.iter().map(|r| r.name.as_str()).collect();
    let names_without: Vec<&str> = without_slash.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names_with, names_without);
}

// ─── Delta overlay ────────────────────────────────────────────────────────────

#[test]
fn delta_is_empty_on_new_index() {
    let idx = build_test_index();
    assert!(idx.delta.is_empty());
}

#[test]
fn delta_mark_deleted_hides_from_search() {
    let mut b = EsIndexBuilder::new();
    let root = b.add_record(1, u32::MAX, "C:", flags::DIRECTORY, 0).unwrap();
    let f    = b.add_record(2, root, "target.txt", 0, 1).unwrap();
    let mut idx = b.finish().unwrap();
    assert!(!idx.search("target", 10).is_empty());
    idx.delta.mark_deleted(f);
    assert!(idx.search("target", 10).is_empty(), "deleted record should be hidden");
}

#[test]
fn delta_mark_deleted_does_not_affect_other_records() {
    let mut b = EsIndexBuilder::new();
    let root = b.add_record(1, u32::MAX, "C:", flags::DIRECTORY, 0).unwrap();
    let f1   = b.add_record(2, root, "keep.txt", 0, 1).unwrap();
    let _f2  = b.add_record(3, root, "delete.txt", 0, 1).unwrap();
    let mut idx = b.finish().unwrap();
    idx.delta.mark_deleted(_f2);
    let results = idx.search("keep", 10);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "keep.txt");
    let _ = f1;
}

// ─── Edge cases & boundary conditions ────────────────────────────────────────

#[test]
fn path_from_idx_root_has_trailing_backslash() {
    let mut b = EsIndexBuilder::new();
    let root = b.add_record(1, u32::MAX, "C:", flags::DIRECTORY, 0).unwrap();
    let idx  = b.finish().unwrap();
    assert_eq!(idx.path_from_idx(root).unwrap(), r"C:\");
}

#[test]
fn index_with_only_root_record() {
    let mut b = EsIndexBuilder::new();
    let root = b.add_record(5, u32::MAX, "D:", flags::DIRECTORY, 0).unwrap();
    let idx  = b.finish().unwrap();
    assert_eq!(idx.records_len(), 1);
    assert_eq!(idx.children(root).unwrap(), &[] as &[u32]);
}

#[test]
fn name_retrieval_correctness() {
    let mut b = EsIndexBuilder::new();
    let root = b.add_record(1, u32::MAX, "C:", flags::DIRECTORY, 0).unwrap();
    let f    = b.add_record(2, root, "hello world.txt", 0, 1).unwrap();
    let idx  = b.finish().unwrap();
    assert_eq!(idx.name(f).unwrap(), "hello world.txt");
}

#[test]
fn out_of_range_record_returns_error() {
    let mut b = EsIndexBuilder::new();
    b.add_record(1, u32::MAX, "C:", flags::DIRECTORY, 0).unwrap();
    let idx = b.finish().unwrap();
    assert!(idx.record(999).is_err());
    assert!(idx.name(999).is_err());
    assert!(idx.children(999).is_err());
    assert!(idx.path_from_idx(999).is_err());
}

#[test]
fn search_no_match_returns_empty() {
    let idx = build_test_index();
    let results = idx.search("zzznomatch", 10);
    assert!(results.is_empty());
}

#[test]
fn search_results_sorted_by_score_descending() {
    let idx = build_test_index();
    let results = idx.search("a", 20);
    for pair in results.windows(2) {
        assert!(pair[0].score >= pair[1].score,
            "results not sorted: {} > {} violated",
            pair[0].score, pair[1].score);
    }
}

// ─── path_to_idx ─────────────────────────────────────────────────────────────

#[test]
fn path_to_idx_finds_root() {
    let idx = build_test_index();
    let root_idx = idx.path_to_idx(r"C:\").unwrap();
    assert_eq!(idx.name(root_idx).unwrap(), "C:");
}

#[test]
fn path_to_idx_finds_nested_dir() {
    let idx = build_test_index();
    let idx_result = idx.path_to_idx(r"C:\Users\admin").unwrap();
    assert_eq!(idx.name(idx_result).unwrap(), "admin");
}

#[test]
fn path_to_idx_case_insensitive() {
    let idx = build_test_index();
    let lower = idx.path_to_idx(r"C:\users\admin").unwrap();
    let upper = idx.path_to_idx(r"C:\USERS\ADMIN").unwrap();
    assert_eq!(lower, upper);
}

// ─── USN delta overlay (apply_events) ────────────────────────────────────────

use crate::usn::{EsUsnEvent, EsUsnEventKind};

fn build_overlay_fixture() -> EsIndex {
    let mut b = EsIndexBuilder::new();
    let root = b.add_record(5, u32::MAX, "C:", flags::DIRECTORY, 0).unwrap();
    b.add_record(6, root, "Users", flags::DIRECTORY, 1).unwrap();
    b.finish().unwrap()
}

#[test]
fn delta_create_makes_file_searchable_with_full_path() {
    let mut idx = build_overlay_fixture();
    idx.apply_events(&[EsUsnEvent {
        kind: EsUsnEventKind::Create,
        file_ref: 100,
        parent_ref: Some(6),
        name: Some("new.txt".to_owned()),
        flags: Some(0),
    }]);
    let results = idx.search("new.txt", 10);
    assert_eq!(results.len(), 1, "created file must be searchable");
    assert_eq!(results[0].path, r"C:\Users\new.txt");
}

#[test]
fn delta_create_child_before_parent_in_batch_still_links() {
    let mut idx = build_overlay_fixture();
    idx.apply_events(&[
        EsUsnEvent {
            kind: EsUsnEventKind::Create,
            file_ref: 101,
            parent_ref: Some(100),
            name: Some("child.txt".to_owned()),
            flags: Some(0),
        },
        EsUsnEvent {
            kind: EsUsnEventKind::Create,
            file_ref: 100,
            parent_ref: Some(6),
            name: Some("sub".to_owned()),
            flags: Some(flags::DIRECTORY),
        },
    ]);
    let results = idx.search("child.txt", 10);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].path, r"C:\Users\sub\child.txt");
}

#[test]
fn delta_delete_hides_created_file() {
    let mut idx = build_overlay_fixture();
    idx.apply_events(&[EsUsnEvent {
        kind: EsUsnEventKind::Create,
        file_ref: 100,
        parent_ref: Some(6),
        name: Some("gone.txt".to_owned()),
        flags: Some(0),
    }]);
    assert_eq!(idx.search("gone.txt", 10).len(), 1);
    idx.apply_events(&[EsUsnEvent {
        kind: EsUsnEventKind::Delete,
        file_ref: 100,
        parent_ref: None,
        name: None,
        flags: None,
    }]);
    assert!(idx.search("gone.txt", 10).is_empty(), "deleted file must disappear");
}

#[test]
fn delta_rename_updates_name() {
    let mut idx = build_overlay_fixture();
    idx.apply_events(&[EsUsnEvent {
        kind: EsUsnEventKind::Create,
        file_ref: 100,
        parent_ref: Some(6),
        name: Some("old.txt".to_owned()),
        flags: Some(0),
    }]);
    idx.apply_events(&[EsUsnEvent {
        kind: EsUsnEventKind::Rename,
        file_ref: 100,
        parent_ref: Some(6),
        name: Some("renamed.txt".to_owned()),
        flags: None,
    }]);
    assert!(idx.search("old.txt", 10).is_empty(), "old name gone");
    let results = idx.search("renamed.txt", 10);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].path, r"C:\Users\renamed.txt");
}

#[test]
fn delta_move_updates_child_path() {
    let mut idx = build_overlay_fixture();
    idx.apply_events(&[
        EsUsnEvent {
            kind: EsUsnEventKind::Create,
            file_ref: 100,
            parent_ref: Some(6),
            name: Some("movedir".to_owned()),
            flags: Some(flags::DIRECTORY),
        },
        EsUsnEvent {
            kind: EsUsnEventKind::Create,
            file_ref: 101,
            parent_ref: Some(100),
            name: Some("leaf.txt".to_owned()),
            flags: Some(0),
        },
    ]);
    assert_eq!(idx.search("leaf.txt", 10)[0].path, r"C:\Users\movedir\leaf.txt");

    idx.apply_events(&[EsUsnEvent {
        kind: EsUsnEventKind::Move,
        file_ref: 100,
        parent_ref: Some(5),
        name: None,
        flags: None,
    }]);
    let results = idx.search("leaf.txt", 10);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].path, r"C:\movedir\leaf.txt", "child path follows moved parent");
}
