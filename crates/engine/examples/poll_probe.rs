// USN 轮询 CPU 占用测量探针（非生产代码，仅用于优化课题基准测试）。
//
// 流程：
//   1. 启动 SearchEngine 并索引 EASYSEARCH_DRIVES 指定的盘。
//   2. 等待 AllReady（全盘 MFT 索引完成）。
//   3. 打印 "POLL_PHASE_START <pid>" 标记，随后仅让后台 USN 轮询循环空转
//      EASYSEARCH_PROBE_SECS 秒（默认 60s）。此阶段进程不做搜索/重绘，
//      CPU 几乎全部来自每秒一次的 USN 轮询。
//   4. 打印 "POLL_PHASE_END" 并退出。
//
// 外部采样器（PowerShell）在 POLL_PHASE_START 与 POLL_PHASE_END 之间读取本
// 进程的 TotalProcessorTime 差值，即为轮询窗口消耗的 CPU 秒数，用于对比
// reason_mask 收窄前后的开销。

use std::sync::Arc;
use std::time::{Duration, Instant};

use easysearch_engine::{EngineConfig, EngineEvent, SearchEngine};

fn main() {
    let probe_secs: u64 = std::env::var("EASYSEARCH_PROBE_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(60);

    let config = EngineConfig::from_env();
    eprintln!("[probe] drives={:?} window={}s", config.auto_index_drives, probe_secs);

    let (engine, event_rx) = SearchEngine::with_events(config);
    let engine = Arc::new(engine);
    engine.start_background();

    // 等索引就绪（全盘 MFT 扫描是 CPU 密集的，必须排除在测量窗口之外）。
    let start = Instant::now();
    loop {
        match event_rx.recv_timeout(Duration::from_secs(120)) {
            Ok(EngineEvent::AllReady) => {
                eprintln!(
                    "[probe] index ready: {} records, {:.2}s",
                    engine.record_count(),
                    start.elapsed().as_secs_f64()
                );
                break;
            }
            Ok(EngineEvent::DriveReady { drive, records, elapsed }) => {
                eprintln!("[probe]   drive {drive}: {records} records, {:.2}s", elapsed.as_secs_f64());
            }
            Ok(EngineEvent::DriveError { drive, error }) => {
                eprintln!("[probe]   drive {drive} ERROR: {error}");
            }
            Ok(_) => {}
            Err(_) => {
                eprintln!("[probe] timeout waiting for index");
                std::process::exit(1);
            }
        }
    }

    // 排空索引阶段积压的事件，避免它们影响测量窗口。
    while event_rx.try_recv().is_ok() {}

    let pid = std::process::id();
    // 采样器靠这行 stdout 标记对齐测量窗口起点。
    println!("POLL_PHASE_START {pid}");
    // 持续排空事件通道，防止发送端阻塞（GUI 里由消息循环消费，此处代劳）。
    let drain = {
        let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let done_clone = Arc::clone(&done);
        let handle = std::thread::spawn(move || {
            while !done_clone.load(std::sync::atomic::Ordering::Relaxed) {
                while event_rx.try_recv().is_ok() {}
                std::thread::sleep(Duration::from_millis(100));
            }
        });
        (done, handle)
    };

    std::thread::sleep(Duration::from_secs(probe_secs));

    // Mark the measurement window end, then stay alive a few extra seconds so
    // the external sampler can read this process's TotalProcessorTime while it
    // still exists (avoids a race where the process exits before the sampler
    // captures the end CPU value, which showed up as a negative delta).
    println!("POLL_PHASE_END");
    std::thread::sleep(Duration::from_secs(6));
    drain.0.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = drain.1.join();
    engine.shutdown();
}
