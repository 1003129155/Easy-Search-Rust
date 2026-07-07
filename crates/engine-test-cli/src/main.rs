// engine-test-cli: 端到端测试程序
// 1. 启动 SearchEngine，索引盘
// 2. 创建测试文件
// 3. 等待 USN 轮询捕获
// 4. 搜索文件名验证是否能在搜索结果中出现
// 5. 删除测试文件
// 6. 等待 USN 轮询捕获
// 7. 再次搜索验证文件已从结果中消失

use std::sync::Arc;
use std::time::{Duration, Instant};

use easysearch_engine::{EngineConfig, EngineEvent, SearchEngine};

/// 唯一的测试文件名，不太可能和系统中已有文件冲突
const TEST_FILENAME: &str = "EASYSEARCH_USN_LIVE_TEST_7f3a9b2c.txt";

fn main() {
    println!("═══════════════════════════════════════════════════════════");
    println!("  EasySearch Engine 端到端测试");
    println!("  验证: 创建文件 → USN捕获 → 搜索能找到");
    println!("         删除文件 → USN捕获 → 搜索找不到");
    println!("═══════════════════════════════════════════════════════════");
    println!();

    let config = EngineConfig::from_env();
    println!("[配置] 索引盘符: {:?}", config.auto_index_drives);
    println!();

    let (engine, event_rx) = SearchEngine::with_events(config);
    let engine = Arc::new(engine);
    engine.start_background();

    // ── Phase 1: 等索引就绪 ──────────────────────────────────────────
    println!("[Phase 1] 等待索引就绪...");
    let start = Instant::now();
    loop {
        match event_rx.recv_timeout(Duration::from_secs(30)) {
            Ok(EngineEvent::AllReady) => {
                println!(
                    "[Phase 1] ✓ 索引就绪 ({} 条记录, {:.2}s)",
                    engine.record_count(),
                    start.elapsed().as_secs_f64()
                );
                break;
            }
            Ok(EngineEvent::DriveReady { drive, records, elapsed }) => {
                println!(
                    "  盘 {drive}: {records} 条记录, {:.2}s",
                    elapsed.as_secs_f64()
                );
            }
            Ok(EngineEvent::DriveError { drive, error }) => {
                println!("  ✗ 盘 {drive} 索引失败: {error}");
            }
            Ok(_) => {}
            Err(_) => {
                println!("[Phase 1] ✗ 超时 30s，索引未完成");
                std::process::exit(1);
            }
        }
    }
    println!();

    // ── Phase 2: 搜索测试文件（应该不存在） ─────────────────────────
    println!("[Phase 2] 搜索 \"{TEST_FILENAME}\" (应该不存在)...");
    let results = engine.search(TEST_FILENAME, 10);
    if results.is_empty() {
        println!("[Phase 2] ✓ 搜索结果为空 (正确，文件尚未创建)");
    } else {
        println!(
            "[Phase 2] ⚠ 搜索返回 {} 条结果 (可能已存在同名文件?)",
            results.len()
        );
        for r in &results {
            println!("          - {}", r.path);
        }
    }
    println!();

    // ── Phase 3: 创建测试文件 ────────────────────────────────────────
    let test_dir = std::env::current_dir().unwrap_or_default();
    let test_path = test_dir.join(TEST_FILENAME);
    println!("[Phase 3] 创建测试文件: {}", test_path.display());
    std::fs::write(&test_path, "USN live test content").expect("无法创建测试文件");
    println!("[Phase 3] ✓ 文件已创建");
    println!();

    // ── Phase 4: 等待 USN 捕获 + 搜索验证 ───────────────────────────
    println!("[Phase 4] 等待 USN 轮询捕获文件创建事件...");
    let poll_start = Instant::now();
    let mut found = false;
    let max_wait = Duration::from_secs(15);

    // 先消耗事件通道让 USN 应用生效
    while poll_start.elapsed() < max_wait {
        // 等至少 2 秒让 USN 轮询有时间跑
        std::thread::sleep(Duration::from_secs(2));

        // 消耗所有待处理事件
        while event_rx.try_recv().is_ok() {}

        // 搜索测试文件
        let results = engine.search(TEST_FILENAME, 10);
        if !results.is_empty() {
            println!(
                "[Phase 4] ✓ 搜索找到文件！(等待 {:.1}s)",
                poll_start.elapsed().as_secs_f64()
            );
            for r in &results {
                println!("          路径: {}", r.path);
                println!("          分数: {}", r.score);
            }
            found = true;
            break;
        }
        println!(
            "  ... 等待中 ({:.0}s / {:.0}s)",
            poll_start.elapsed().as_secs_f64(),
            max_wait.as_secs_f64()
        );
    }

    if !found {
        println!("[Phase 4] ✗ 超时 {}s，搜索仍无结果", max_wait.as_secs());
        println!("          USN 事件可能已捕获但 apply_events 未正确工作");
        println!("          或 delta overlay 的搜索路径有问题");
    }
    println!();

    // ── Phase 5: 删除测试文件 ────────────────────────────────────────
    println!("[Phase 5] 删除测试文件...");
    std::fs::remove_file(&test_path).expect("无法删除测试文件");
    println!("[Phase 5] ✓ 文件已删除");
    println!();

    // ── Phase 6: 等待 USN 捕获删除 + 搜索验证 ───────────────────────
    println!("[Phase 6] 等待 USN 轮询捕获文件删除事件...");
    let poll_start = Instant::now();
    let mut gone = false;

    while poll_start.elapsed() < max_wait {
        std::thread::sleep(Duration::from_secs(2));
        while event_rx.try_recv().is_ok() {}

        let results = engine.search(TEST_FILENAME, 10);
        if results.is_empty() {
            println!(
                "[Phase 6] ✓ 搜索结果为空（文件已从索引移除, 等待 {:.1}s）",
                poll_start.elapsed().as_secs_f64()
            );
            gone = true;
            break;
        }
        println!(
            "  ... 仍找到 {} 条结果, 等待中 ({:.0}s)",
            results.len(),
            poll_start.elapsed().as_secs_f64()
        );
    }

    if !gone {
        println!("[Phase 6] ✗ 超时，文件仍在搜索结果中");
    }
    println!();

    // ── 总结 ─────────────────────────────────────────────────────────
    println!("═══════════════════════════════════════════════════════════");
    if found && gone {
        println!("  ✓ 全部测试通过！USN 实时文件变化检测正常工作。");
        println!("    创建的文件能被搜到，删除后能从搜索结果中移除。");
    } else if found && !gone {
        println!("  △ 部分通过：文件创建能被搜到，但删除后仍在结果中。");
    } else if !found && gone {
        println!("  ✗ 创建文件后搜索不到！delta overlay 搜索有问题。");
    } else {
        println!("  ✗ 创建和删除都有问题，USN→搜索链路不通。");
    }
    println!("═══════════════════════════════════════════════════════════");

    // 清理：确保测试文件被删除
    let _ = std::fs::remove_file(&test_path);
}
