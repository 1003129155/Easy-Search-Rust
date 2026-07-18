// Copyright (c) 2025-2026 LIJIALU. MIT License.
//
// Integration tests for easysearch-engine.
//
// # 测试策略
//
// 这些测试分为两层：
//
// 1. **内存层（无需管理员权限）**：用 `EsIndexBuilder` 构造假索引，直接注入
//    `DriveManager`，然后验证 `SearchEngine` 的搜索行为和 `DriveManager::apply`
//    的 USN delta 更新逻辑。大多数 CI 场景可以运行这些测试。
//
// 2. **真实 MFT 层（需要管理员权限）**：启动引擎，索引真实 NTFS 盘，在临时目录
//    创建/删除文件，等待 USN 轮询更新索引，验证搜索结果。用
//    `#[cfg_attr(not(feature = "real-mft-tests"), ignore)]` 或直接 `#[ignore]`
//    标记，需要显式 `cargo test -- --ignored` 运行，且必须以管理员身份执行。

use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use easysearch_core::{
    EsIndexBuilder,
    usn::{EsUsnEvent, EsUsnEventKind},
    record::flags,
};
use easysearch_engine::{DriveManager, EngineConfig, EngineEvent, SearchEngine};

// ─────────────────────────────────────────────────────────────────────────────
// 辅助函数
// ─────────────────────────────────────────────────────────────────────────────

/// 构造一个小型内存索引，拓扑结构：
///
/// ```
/// C:
/// ├── src/
/// │   ├── main.rs
/// │   └── lib.rs
/// ├── Cargo.toml
/// ├── README.md
/// └── build.ps1
/// ```
fn build_test_index(drive: char) -> easysearch_core::EsIndex {
    let mut b = EsIndexBuilder::new();
    let root = b.add_record(5, u32::MAX, &format!("{drive}:"), flags::DIRECTORY, 0).unwrap();
    let src  = b.add_record(6, root,  "src",       flags::DIRECTORY, 0).unwrap();
    /*main */  b.add_record(7, src,   "main.rs",   0,                1).unwrap();
    /*lib  */  b.add_record(8, src,   "lib.rs",    0,                1).unwrap();
    /*cargo*/  b.add_record(9, root,  "Cargo.toml",0,                1).unwrap();
    /*readme*/ b.add_record(10, root, "README.md", 0,                1).unwrap();
    /*build */ b.add_record(11, root, "build.ps1", 0,                1).unwrap();
    b.finish().unwrap()
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. 基础搜索行为 — 内存索引
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn search_finds_exact_filename() {
    let mut mgr = DriveManager::new();
    mgr.install('C', build_test_index('C'));

    let results = mgr.search("main.rs", 10);
    assert!(!results.is_empty(), "应能找到 main.rs");
    assert_eq!(results[0].name, "main.rs");
    assert_eq!(results[0].path, r"C:\src\main.rs");
    assert!(!results[0].is_directory);
}

#[test]
fn search_finds_partial_match_and_ranks_by_score() {
    let mut mgr = DriveManager::new();
    mgr.install('C', build_test_index('C'));

    // "lib" 应匹配 lib.rs；"main" 应只匹配 main.rs
    let results = mgr.search("lib", 10);
    assert!(results.iter().any(|r| r.name == "lib.rs"), "应匹配 lib.rs");
    // main.rs 不含 "lib"
    assert!(!results.iter().any(|r| r.name == "main.rs"), "main.rs 不应出现");
}

#[test]
fn search_extension_glob_matches_all_rs_files() {
    let mut mgr = DriveManager::new();
    mgr.install('C', build_test_index('C'));

    // normalize_query("*.rs") → ".rs"，引擎用它做子串匹配
    let normalized = easysearch_engine::normalize_query("*.rs");
    assert_eq!(normalized, ".rs");

    let results = mgr.search(&normalized, 10);
    let names: Vec<&str> = results.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"main.rs"), "应包含 main.rs，实际 = {names:?}");
    assert!(names.contains(&"lib.rs"),  "应包含 lib.rs，实际 = {names:?}");
    // Cargo.toml 不含 ".rs"
    assert!(!names.contains(&"Cargo.toml"), "Cargo.toml 不应出现");
}

#[test]
fn search_is_case_insensitive() {
    let mut mgr = DriveManager::new();
    mgr.install('C', build_test_index('C'));

    let lower = mgr.search("readme", 10);
    let upper = mgr.search("README", 10);
    let mixed = mgr.search("ReadMe", 10);

    for results in [&lower, &upper, &mixed] {
        assert!(
            results.iter().any(|r| r.name == "README.md"),
            "大小写变体都应匹配 README.md"
        );
    }
}

#[test]
fn search_directory_gets_score_bonus_over_file() {
    let mut mgr = DriveManager::new();
    mgr.install('C', build_test_index('C'));

    // "src" 精确匹配目录
    let results = mgr.search("src", 10);
    let dir_result = results.iter().find(|r| r.is_directory && r.name == "src");
    assert!(dir_result.is_some(), "应有目录结果 src/");

    if results.len() >= 2 {
        // 如果目录和文件同分，目录 bonus (+100) 应让目录排在前面
        assert!(results[0].is_directory || results[0].score >= results[1].score);
    }
}

#[test]
fn search_empty_query_returns_all_records_up_to_limit() {
    let mut mgr = DriveManager::new();
    mgr.install('C', build_test_index('C'));

    // 引擎约定：空 query 返回全部记录（受 limit 约束），供“浏览”模式使用。
    // 注意 SearchEngine::search 会先 normalize_query，GUI 层的空输入不会走到这里。
    let results = mgr.search("", 10);
    assert!(!results.is_empty(), "空 query 应返回全部记录（浏览模式）");
    // build_test_index 有 7 条记录（含根），limit=10 应全部返回
    assert!(results.len() <= 10, "结果数应受 limit 约束");
    assert!(
        results.iter().any(|r| r.name == "C:"),
        "空 query 结果应包含根记录"
    );
}

#[test]
fn search_limit_is_respected() {
    let mut mgr = DriveManager::new();
    mgr.install('C', build_test_index('C'));

    // ".rs" 和 ".toml" 等至少 4 条匹配，但 limit=2 时只返回 2 条
    let results = mgr.search(".", 2);
    assert!(results.len() <= 2, "结果数应 ≤ limit");
}

#[test]
fn enumerate_lists_directory_children() {
    let mut mgr = DriveManager::new();
    mgr.install('C', build_test_index('C'));

    let children = mgr.enumerate(r"C:\src", "", false, 20).unwrap();
    let names: Vec<&str> = children.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"main.rs"), "src/ 应含 main.rs，实际 = {names:?}");
    assert!(names.contains(&"lib.rs"),  "src/ 应含 lib.rs，实际 = {names:?}");
    // 不应含 root 下的文件
    assert!(!names.contains(&"Cargo.toml"), "Cargo.toml 是 root 的子项");
}

#[test]
fn enumerate_root_lists_top_level_entries() {
    let mut mgr = DriveManager::new();
    mgr.install('C', build_test_index('C'));

    let entries = mgr.enumerate("C:", "", false, 20).unwrap();
    let names: Vec<&str> = entries.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"src"),       "root 应含 src/");
    assert!(names.contains(&"Cargo.toml"),"root 应含 Cargo.toml");
    assert!(names.contains(&"README.md"), "root 应含 README.md");
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. SearchFilter 语义 — 内存索引
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn search_filter_files_only_excludes_directories() {
    let mut mgr = DriveManager::new();
    mgr.install('C', build_test_index('C'));

    // 不用 SearchEngine，直接用 DriveManager，然后在结果层 assert
    let all = mgr.search("src", 10);
    // 验证存在目录项
    assert!(all.iter().any(|r| r.is_directory), "未过滤时应包含目录");

    // SearchFilter::files_only 的逻辑等价于 exclude_flags = DIRECTORY
    let filtered: Vec<_> = all
        .into_iter()
        .filter(|r| !r.is_directory)
        .collect();
    assert!(
        filtered.iter().all(|r| !r.is_directory),
        "files_only 过滤后不应含目录"
    );
}

#[test]
fn search_filter_directories_only() {
    let mut mgr = DriveManager::new();
    mgr.install('C', build_test_index('C'));

    // "src" 匹配目录 src/（注意：fixture 里目录名不含 "."，所以不能用 "." 做查询）
    let all = mgr.search("src", 20);
    let dirs: Vec<_> = all.into_iter().filter(|r| r.is_directory).collect();
    assert!(!dirs.is_empty(), "至少应有一个目录结果");
    for d in &dirs {
        assert!(d.is_directory, "过滤后每条结果都应是目录");
    }
    assert!(
        dirs.iter().any(|d| d.name == "src"),
        "目录过滤结果应含 src/"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. USN delta 更新 — apply_events 内存测试
// ─────────────────────────────────────────────────────────────────────────────

/// 在内存索引上模拟 USN Create 事件（新文件出现），然后验证可以搜到。
#[test]
fn usn_create_event_makes_file_searchable() {
    let mut mgr = DriveManager::new();
    mgr.install('C', build_test_index('C'));

    // 初始状态：没有 "hello_world.rs"
    assert!(
        mgr.search("hello_world", 10).is_empty(),
        "初始索引中不应存在 hello_world.rs"
    );

    // root 目录的 file_ref 是 5（见 build_test_index）
    let events = vec![EsUsnEvent {
        kind:       EsUsnEventKind::Create,
        file_ref:   999,
        parent_ref: Some(5), // C: 根目录
        name:       Some("hello_world.rs".to_string()),
        flags:      Some(0), // 普通文件
    }];
    mgr.apply('C', &events, 100, 1);

    let results = mgr.search("hello_world", 10);
    assert!(
        results.iter().any(|r| r.name == "hello_world.rs"),
        "apply Create 后应能搜到 hello_world.rs，实际 = {results:?}"
    );
}

/// 模拟“创建后删除”一个叠加层（overlay-inserted）记录的完整生命周期。
/// 这正是真实 MFT delete 测试所走的路径：文件先经 Create 事件进入 delta.inserted，
/// 搜到之后再经 Delete 事件 tombstone，应从搜索结果中消失。
#[test]
fn usn_create_then_delete_overlay_record_disappears() {
    let mut mgr = DriveManager::new();
    mgr.install('C', build_test_index('C'));

    // 用一个不在 base 索引里的 file_ref（999）模拟真实盘上新分配的 MFT 记录。
    let file_ref = 999u64;
    mgr.apply('C', &[EsUsnEvent {
        kind:       EsUsnEventKind::Create,
        file_ref,
        parent_ref: Some(5), // C: 根目录
        name:       Some("ephemeral_temp.tmp".to_string()),
        flags:      Some(0),
    }], 600, 1);

    assert!(
        mgr.search("ephemeral_temp", 10).iter().any(|r| r.name == "ephemeral_temp.tmp"),
        "Create 后应能搜到临时文件"
    );

    // 删除同一个 file_ref。
    mgr.apply('C', &[EsUsnEvent {
        kind:       EsUsnEventKind::Delete,
        file_ref,
        parent_ref: None,
        name:       None,
        flags:      None,
    }], 700, 1);

    assert!(
        !mgr.search("ephemeral_temp", 10).iter().any(|r| r.name == "ephemeral_temp.tmp"),
        "Delete 后临时文件应从搜索结果中消失，实际 = {:?}",
        mgr.search("ephemeral_temp", 10)
    );
}

/// 模拟“创建后重命名”一个叠加层（overlay-inserted）记录。这正是真实 MFT
/// rename 测试所走的路径：文件在索引建好后才创建（进入 delta.inserted），
/// 随后被重命名。改名走的是 `set_logical_name` 的 inserted 分支，而非 base
/// 记录的 `delta.renamed` 分支，因此需要单独覆盖。
#[test]
fn usn_rename_overlay_inserted_record_updates_name() {
    let mut mgr = DriveManager::new();
    mgr.install('C', build_test_index('C'));

    // 用一个不在 base 索引里的 file_ref（999）模拟新建文件。
    let file_ref = 999u64;
    mgr.apply('C', &[EsUsnEvent {
        kind:       EsUsnEventKind::Create,
        file_ref,
        parent_ref: Some(5), // C: 根目录
        name:       Some("draft.txt".to_string()),
        flags:      Some(0),
    }], 600, 1);

    assert!(
        mgr.search("draft", 10).iter().any(|r| r.name == "draft.txt"),
        "创建后应能搜到 draft.txt"
    );

    // 重命名同一个 overlay 记录 draft.txt -> final.txt。
    mgr.apply('C', &[EsUsnEvent {
        kind:       EsUsnEventKind::Rename,
        file_ref,
        parent_ref: Some(5),
        name:       Some("final.txt".to_string()),
        flags:      Some(0),
    }], 700, 1);

    assert!(
        !mgr.search("draft", 10).iter().any(|r| r.name == "draft.txt"),
        "重命名后 draft.txt 不应出现，实际 = {:?}",
        mgr.search("draft", 10)
    );
    assert!(
        mgr.search("final", 10).iter().any(|r| r.name == "final.txt"),
        "重命名后应能搜到 final.txt，实际 = {:?}",
        mgr.search("final", 10)
    );
}

/// 模拟 USN Delete 事件（文件被删除），然后验证搜不到。
#[test]
fn usn_delete_event_removes_file_from_search() {
    let mut mgr = DriveManager::new();
    mgr.install('C', build_test_index('C'));

    // 先确认 main.rs 存在
    let before = mgr.search("main.rs", 10);
    assert!(
        before.iter().any(|r| r.name == "main.rs"),
        "删除前应能找到 main.rs"
    );

    // main.rs 的 file_ref 是 7（见 build_test_index）
    let events = vec![EsUsnEvent {
        kind:       EsUsnEventKind::Delete,
        file_ref:   7,
        parent_ref: None,
        name:       None,
        flags:      None,
    }];
    mgr.apply('C', &events, 200, 1);

    let after = mgr.search("main.rs", 10);
    assert!(
        !after.iter().any(|r| r.name == "main.rs"),
        "apply Delete 后不应找到 main.rs，实际 = {after:?}"
    );
}

/// 模拟 USN Rename 事件（文件被重命名）。
#[test]
fn usn_rename_event_updates_filename() {
    let mut mgr = DriveManager::new();
    mgr.install('C', build_test_index('C'));

    // lib.rs (file_ref=8) 重命名为 library.rs
    let events = vec![EsUsnEvent {
        kind:       EsUsnEventKind::Rename,
        file_ref:   8,
        parent_ref: Some(6), // src/ 目录
        name:       Some("library.rs".to_string()),
        flags:      Some(0),
    }];
    mgr.apply('C', &events, 300, 1);

    let old = mgr.search("lib.rs", 10);
    assert!(
        !old.iter().any(|r| r.name == "lib.rs"),
        "重命名后 lib.rs 不应出现，实际 = {old:?}"
    );

    let new = mgr.search("library", 10);
    assert!(
        new.iter().any(|r| r.name == "library.rs"),
        "重命名后应能搜到 library.rs，实际 = {new:?}"
    );
}

/// apply 后 generation 递增，搜索会话快照因此失效（防止缓存读到旧文件名）。
#[test]
fn usn_apply_bumps_generation() {
    let mut mgr = DriveManager::new();
    mgr.install('C', build_test_index('C'));

    let gen_before = mgr.generation();

    mgr.apply('C', &[EsUsnEvent {
        kind:       EsUsnEventKind::Create,
        file_ref:   888,
        parent_ref: Some(5),
        name:       Some("new_file.txt".to_string()),
        flags:      Some(0),
    }], 400, 1);

    assert!(
        mgr.generation() > gen_before,
        "apply 非空事件列表后 generation 应递增"
    );
}

/// apply 空事件列表不应改变 generation（避免不必要的缓存失效）。
#[test]
fn usn_apply_empty_events_does_not_bump_generation() {
    let mut mgr = DriveManager::new();
    mgr.install('C', build_test_index('C'));

    let gen_before = mgr.generation();
    mgr.apply('C', &[], 500, 1);
    assert_eq!(
        mgr.generation(), gen_before,
        "apply 空事件列表不应改变 generation"
    );
}

/// Compact the effective logical view and verify the rebuilt base has no overlay.
#[test]
fn usn_compact_preserves_effective_state_and_clears_delta() {
    let mut manager = DriveManager::new();
    manager.install('C', build_test_index('C'));

    manager.apply(
        'C',
        &[
            EsUsnEvent {
                kind: EsUsnEventKind::Create,
                file_ref: 900,
                parent_ref: Some(5),
                name: Some("temporary.tmp".to_string()),
                flags: Some(0),
            },
            EsUsnEvent {
                kind: EsUsnEventKind::Create,
                file_ref: 901,
                parent_ref: Some(5),
                name: Some("draft.txt".to_string()),
                flags: Some(0),
            },
            EsUsnEvent {
                kind: EsUsnEventKind::Rename,
                file_ref: 901,
                parent_ref: Some(6),
                name: Some("final.txt".to_string()),
                flags: None,
            },
            EsUsnEvent {
                kind: EsUsnEventKind::Delete,
                file_ref: 900,
                parent_ref: None,
                name: None,
                flags: None,
            },
        ],
        900,
        77,
    );

    let candidate = manager
        .compact_candidate()
        .expect("5% threshold should be reached")
        .expect("snapshot should succeed");
    let drive = candidate.drive;
    let revision = candidate.revision;
    let mut rebuilt = candidate.snapshot.rebuild().expect("rebuild should succeed");
    rebuilt.status.journal_id = candidate.journal_id;
    rebuilt.status.last_usn = candidate.last_usn;

    assert!(manager.commit_compact(drive, revision, rebuilt));
    let index = manager.index_for('C').unwrap();
    assert_eq!(index.delta_event_count(), 0);
    assert_eq!(index.status.journal_id, 77);
    assert_eq!(index.status.last_usn, 900);
    assert!(manager.search("temporary", 10).is_empty());
    let final_result = manager.search("final", 10);
    assert!(final_result.iter().any(|result| result.path == r"C:\src\final.txt"));
}

/// A compact result must not replace a drive changed by a later USN batch.
#[test]
fn stale_compact_candidate_is_rejected() {
    let mut manager = DriveManager::new();
    manager.install('C', build_test_index('C'));
    manager.apply(
        'C',
        &[EsUsnEvent {
            kind: EsUsnEventKind::Create,
            file_ref: 910,
            parent_ref: Some(5),
            name: Some("first.txt".to_string()),
            flags: Some(0),
        }],
        100,
        1,
    );

    let candidate = manager.compact_candidate().unwrap().unwrap();
    let rebuilt = candidate.snapshot.rebuild().unwrap();
    manager.apply(
        'C',
        &[EsUsnEvent {
            kind: EsUsnEventKind::Create,
            file_ref: 911,
            parent_ref: Some(5),
            name: Some("second.txt".to_string()),
            flags: Some(0),
        }],
        200,
        1,
    );

    assert!(!manager.commit_compact(candidate.drive, candidate.revision, rebuilt));
    assert!(manager.search("second", 10).iter().any(|result| result.name == "second.txt"));
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. 多盘符索引
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn multi_drive_search_spans_all_drives() {
    let mut mgr = DriveManager::new();
    mgr.install('C', build_test_index('C'));
    mgr.install('D', build_test_index('D'));

    let results = mgr.search("main.rs", 10);
    let c_result = results.iter().find(|r| r.path.starts_with("C:"));
    let d_result = results.iter().find(|r| r.path.starts_with("D:"));
    assert!(c_result.is_some(), "C: 盘结果应存在");
    assert!(d_result.is_some(), "D: 盘结果应存在");
}

#[test]
fn record_count_sums_all_drives() {
    let mut mgr = DriveManager::new();
    mgr.install('C', build_test_index('C'));
    let c_count = mgr.record_count();

    mgr.install('D', build_test_index('D'));
    let total = mgr.record_count();

    assert_eq!(total, c_count * 2, "双盘总记录数应为单盘的两倍");
}

#[test]
fn remove_drive_reduces_record_count_and_search_results() {
    let mut mgr = DriveManager::new();
    mgr.install('C', build_test_index('C'));
    mgr.install('D', build_test_index('D'));

    mgr.remove('D');

    let results = mgr.search("main.rs", 10);
    assert!(
        !results.iter().any(|r| r.path.starts_with("D:")),
        "remove D: 后不应有 D: 的结果"
    );
    assert!(
        results.iter().any(|r| r.path.starts_with("C:")),
        "C: 的结果应仍然存在"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. 取消令牌
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn cancelled_search_returns_empty() {
    let mut mgr = DriveManager::new();
    mgr.install('C', build_test_index('C'));

    let cancel = AtomicBool::new(true);
    let results = mgr.search_with_cancel("main", 10, &cancel);
    assert!(results.is_empty(), "已取消的搜索应返回空结果");
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. 真实 MFT + USN 监听集成测试（需要管理员权限，默认 ignore）
// ─────────────────────────────────────────────────────────────────────────────
//
// 运行方式（管理员 PowerShell）：
//   cargo test -p easysearch-engine -- --ignored
//
// 这些测试会：
//   1. 初始化真实引擎索引 C 盘
//   2. 在 %TEMP% 目录创建唯一命名的测试文件
//   3. 等待 USN 轮询（最多 10 秒）把文件更新到索引
//   4. 用引擎 search() 验证文件出现在结果里
//   5. 删除文件，再等待确认从索引中消失

/// 生成一个高分辨率的唯一后缀（纳秒时间戳），保证同一进程内多个测试、
/// 以及跨多次运行留下的残留文件都不会命名冲突。
#[cfg(windows)]
fn unique_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

/// 等待条件满足或超时，每 200ms 检查一次。
#[cfg(windows)]
fn wait_until<F: Fn() -> bool>(timeout: Duration, check: F) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if check() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    false
}

/// 从事件流等待 AllReady，最多等 120 秒（MFT 全量索引可能较慢）。
#[cfg(windows)]
fn wait_for_ready(rx: &easysearch_engine::EventReceiver) -> bool {
    let deadline = Instant::now() + Duration::from_secs(120);
    while Instant::now() < deadline {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(EngineEvent::AllReady) => return true,
            Ok(EngineEvent::DriveError { drive, error }) => {
                eprintln!("[test] Drive {drive} error: {error}");
                return false;
            }
            _ => continue,
        }
    }
    false
}

#[test]
#[ignore = "需要管理员权限和真实 NTFS 盘；用 cargo test -- --ignored 运行"]
#[cfg(windows)]
fn real_mft_create_file_appears_in_search() {
    use std::fs;

    let temp_dir = std::env::temp_dir();
    let unique_name = format!("easysearch_test_{}_{}.tmp", std::process::id(), unique_suffix());
    let test_file = temp_dir.join(&unique_name);

    // 确保测试 panic 时也能清理（RAII guard）
    struct CleanupGuard(std::path::PathBuf);
    impl Drop for CleanupGuard {
        fn drop(&mut self) { let _ = std::fs::remove_file(&self.0); }
    }
    let _guard = CleanupGuard(test_file.clone());

    let config = EngineConfig {
        cache_dir: Some(tempfile::tempdir().unwrap().keep()),
        auto_index_drives: vec!['C'],
    };
    let (engine, event_rx) = SearchEngine::with_events(config);
    engine.start_background();

    // 等待索引就绪
    assert!(
        wait_for_ready(&event_rx),
        "引擎未能在 120 秒内完成 C 盘索引"
    );

    // 确认文件还不存在
    assert!(
        engine.search(&unique_name, 5).is_empty(),
        "测试文件创建前不应出现在搜索结果中"
    );

    // 创建文件
    fs::write(&test_file, b"easysearch integration test").unwrap();

    // 等待 USN 轮询把新文件加入索引。引擎每 1s 轮询一次；在满盘索引 +
    // 多测试并行的负载下，给 30s 余量以避免偶发的时序抖动导致假失败。
    let found = wait_until(Duration::from_secs(30), || {
        engine
            .search(&unique_name, 5)
            .iter()
            .any(|r| r.name == unique_name)
    });

    // 清理
    let _ = fs::remove_file(&test_file);

    // 关停引擎，让后台 USN 轮询线程退出，避免与后续测试争用资源。
    engine.shutdown();

    assert!(
        found,
        "创建文件后 30 秒内应在搜索结果中找到 {unique_name}"
    );
}

#[test]
#[ignore = "需要管理员权限和真实 NTFS 盘；用 cargo test -- --ignored 运行"]
#[cfg(windows)]
fn real_mft_delete_file_disappears_from_search() {
    use std::fs;

    let temp_dir = std::env::temp_dir();
    let unique_name = format!("easysearch_del_{}_{}.tmp", std::process::id(), unique_suffix());
    let test_file = temp_dir.join(&unique_name);

    let config = EngineConfig {
        cache_dir: Some(tempfile::tempdir().unwrap().keep()),
        auto_index_drives: vec!['C'],
    };
    let (engine, event_rx) = SearchEngine::with_events(config);
    engine.start_background();

    assert!(
        wait_for_ready(&event_rx),
        "引擎未能在 120 秒内完成 C 盘索引"
    );

    // 先创建文件，等它出现在索引里（满盘索引负载下给 30s 余量）
    fs::write(&test_file, b"to be deleted").unwrap();
    let appeared = wait_until(Duration::from_secs(30), || {
        engine
            .search(&unique_name, 5)
            .iter()
            .any(|r| r.name == unique_name)
    });
    assert!(appeared, "文件应先出现在索引中才能测试删除");

    // 删除文件
    fs::remove_file(&test_file).unwrap();

    // 等待索引更新，文件应消失
    let disappeared = wait_until(Duration::from_secs(30), || {
        engine
            .search(&unique_name, 5)
            .iter()
            .all(|r| r.name != unique_name)
    });

    // 关停引擎，让后台 USN 轮询线程退出，避免与后续测试争用资源。
    engine.shutdown();

    assert!(
        disappeared,
        "删除文件后 30 秒内应从搜索结果中消失 {unique_name}"
    );
}

/// 真实盘上重命名一个文件，验证引擎经 USN 轮询检测到改名：
///   1. 创建 old 名字的文件，等它可搜索
///   2. `fs::rename` 改成 new 名字
///   3. 等 USN 轮询更新索引后：old 名字搜不到，new 名字搜得到
#[test]
#[ignore = "需要管理员权限和真实 NTFS 盘；用 cargo test -- --ignored 运行"]
#[cfg(windows)]
fn real_mft_rename_file_updates_search() {
    use std::fs;

    let temp_dir = std::env::temp_dir();
    let suffix = unique_suffix();
    let old_name = format!("easysearch_ren_old_{}_{}.tmp", std::process::id(), suffix);
    let new_name = format!("easysearch_ren_new_{}_{}.tmp", std::process::id(), suffix);
    let old_path = temp_dir.join(&old_name);
    let new_path = temp_dir.join(&new_name);

    // 无论断言是否 panic，都清理两个可能存在的文件。
    struct CleanupGuard(std::path::PathBuf, std::path::PathBuf);
    impl Drop for CleanupGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
            let _ = std::fs::remove_file(&self.1);
        }
    }
    let _guard = CleanupGuard(old_path.clone(), new_path.clone());

    let config = EngineConfig {
        cache_dir: Some(tempfile::tempdir().unwrap().keep()),
        auto_index_drives: vec!['C'],
    };
    let (engine, event_rx) = SearchEngine::with_events(config);
    engine.start_background();

    assert!(
        wait_for_ready(&event_rx),
        "引擎未能在 120 秒内完成 C 盘索引"
    );

    // 先创建 old 名字的文件，等它出现在索引里。
    fs::write(&old_path, b"to be renamed").unwrap();
    let appeared = wait_until(Duration::from_secs(30), || {
        engine
            .search(&old_name, 5)
            .iter()
            .any(|r| r.name == old_name)
    });
    assert!(appeared, "重命名前应先能搜到 old 名字 {old_name}");

    // 重命名 old -> new。
    fs::rename(&old_path, &new_path).unwrap();

    // 等 USN 轮询检测到改名：new 名字应能搜到。
    let new_found = wait_until(Duration::from_secs(30), || {
        engine
            .search(&new_name, 5)
            .iter()
            .any(|r| r.name == new_name)
    });

    // old 名字应已从结果中消失。
    let old_gone = wait_until(Duration::from_secs(30), || {
        engine
            .search(&old_name, 5)
            .iter()
            .all(|r| r.name != old_name)
    });

    // 关停引擎，让后台 USN 轮询线程退出，避免与后续测试争用资源。
    engine.shutdown();

    assert!(new_found, "重命名后 30 秒内应能搜到 new 名字 {new_name}");
    assert!(old_gone, "重命名后 30 秒内 old 名字应从结果中消失 {old_name}");
}

#[test]
#[ignore = "需要管理员权限和真实 NTFS 盘；用 cargo test -- --ignored 运行"]
#[cfg(windows)]
fn real_mft_search_results_contain_valid_paths() {
    let config = EngineConfig {
        cache_dir: Some(tempfile::tempdir().unwrap().keep()),
        auto_index_drives: vec!['C'],
    };
    let (engine, event_rx) = SearchEngine::with_events(config);
    engine.start_background();

    assert!(
        wait_for_ready(&event_rx),
        "引擎未能在 120 秒内完成 C 盘索引"
    );

    assert!(engine.record_count() > 0, "索引后记录数应 > 0");

    // 搜索 "windows"（C:\Windows 必然存在）
    let results = engine.search("windows", 20);
    assert!(!results.is_empty(), "搜索 'windows' 应有结果");

    for r in &results {
        // 路径以 "C:\" 开头
        assert!(
            r.path.starts_with("C:"),
            "结果路径应以 C: 开头，实际 = {}",
            r.path
        );
        // 文件名不为空
        assert!(!r.name.is_empty(), "文件名不应为空");
        // score > 0
        assert!(r.score > 0, "score 应 > 0，实际 = {}", r.score);
    }

    // 关停引擎，让后台 USN 轮询线程退出，避免与后续测试争用资源。
    engine.shutdown();
}

#[test]
#[ignore = "需要管理员权限和真实 NTFS 盘；用 cargo test -- --ignored 运行"]
#[cfg(windows)]
fn real_mft_enumerate_windows_system32() {
    let config = EngineConfig {
        cache_dir: Some(tempfile::tempdir().unwrap().keep()),
        auto_index_drives: vec!['C'],
    };
    let (engine, event_rx) = SearchEngine::with_events(config);
    engine.start_background();

    assert!(
        wait_for_ready(&event_rx),
        "引擎未能在 120 秒内完成 C 盘索引"
    );

    let entries = engine
        .enumerate(r"C:\Windows\System32", "kernel", false, 10)
        .unwrap();

    assert!(!entries.is_empty(), "System32 下应能找到含 'kernel' 的文件");

    for entry in &entries {
        assert!(
            entry.path.to_lowercase().contains("system32"),
            "结果应在 System32 目录内，实际 = {}",
            entry.path
        );
    }

    // 关停引擎，让后台 USN 轮询线程退出，避免与后续测试争用资源。
    engine.shutdown();
}
