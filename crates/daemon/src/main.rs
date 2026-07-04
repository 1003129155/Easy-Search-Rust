// Copyright (c) 2025-2026 LIJIALU. MIT License.

// Windowsサブシステム指定: コンソールウィンドウを表示しない
// (daemon モードで黒い CMD ウィンドウが出ないようにする)
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! EasySearch backend daemon entrypoint.
//!
//! On Windows the daemon runs as a named-pipe server so clients can
//! connect without spawning a subprocess with redirected stdio. On all
//! other platforms (CI, cross-compilation dev loop) it falls back to the
//! original stdin/stdout NDJSON mode.
//!
//! In both modes a background worker builds the per-drive indexes
//! asynchronously (so the pipe/loop is responsive and `status` can report
//! `indexing`), then polls the USN journal to apply incremental changes.

mod cli;
mod config;
mod drive_manager;
mod error;
mod ipc;
mod process_lifetime;
mod protocol;
mod service;
mod usn_source;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use config::EsConfig;
use error::EsError;
use service::EsService;

/// Interval between USN journal polls.
const POLL_INTERVAL: Duration = Duration::from_millis(1000);

fn main() -> Result<(), EsError> {
    // CLI モード判定: コマンドライン引数があれば CLI クライアントとして動作する
    if let Some(args) = cli::CliArgs::parse() {
        if let Err(e) = cli::run(args) {
            eprintln!("[easysearch] {e}");
            std::process::exit(1);
        }
        return Ok(());
    }

    // Daemon モード
    let config = EsConfig::from_env();

    // 重複起動防止: pipe が既に存在する場合は別の daemon が動いているので即座に終了
    if is_pipe_already_taken(&config.pipe_name) {
        eprintln!("[easysearch] daemon already running (pipe exists), exiting.");
        return Ok(());
    }

    let service = Arc::new(Mutex::new(EsService::new(config.clone())));

    spawn_index_worker(Arc::clone(&service), config.clone());

    #[cfg(windows)]
    return run_pipe_server(&service, &config.pipe_name);

    #[cfg(not(windows))]
    return run_stdio(&service);
}

/// Background worker: build every configured drive index, then poll the USN
/// journal to keep them fresh.
fn spawn_index_worker(service: Arc<Mutex<EsService>>, config: EsConfig) {
    std::thread::spawn(move || {
        let cache_dir = config.cache_dir.clone();
        let drives = config.auto_index_drives.clone();

        // ── Build phase ──────────────────────────────────────────────────────
        for letter in &drives {
            match drive_manager::build_index(*letter, cache_dir.as_deref()) {
                Ok(index) => {
                    if let Ok(mut svc) = service.lock() {
                        svc.install_index(*letter, index);
                    }
                }
                Err(err) => eprintln!("[easysearch] index build failed for {letter}: {err}"),
            }
        }
        if let Ok(mut svc) = service.lock() {
            svc.set_indexing(false);
        }

        // ── Poll phase ───────────────────────────────────────────────────────
        loop {
            if service.lock().map(|s| s.should_shutdown()).unwrap_or(true) {
                break;
            }
            std::thread::sleep(POLL_INTERVAL);

            for letter in &drives {
                let cursor = service.lock().ok().and_then(|s| s.cursor(*letter));
                let Some((_journal_id, last_usn)) = cursor else {
                    continue;
                };

                if let Some(result) = usn_source::poll_drive(*letter, last_usn) {
                    let advanced = result.new_last_usn != last_usn;
                    if !result.events.is_empty() || advanced {
                        if let Ok(mut svc) = service.lock() {
                            svc.apply_events(
                                *letter,
                                &result.events,
                                result.new_last_usn,
                                result.journal_id,
                            );
                        }
                    }
                }
            }
        }
    });
}

/// Windows production mode: named-pipe server.
#[cfg(windows)]
fn run_pipe_server(service: &Arc<Mutex<EsService>>, pipe_name: &str) -> Result<(), EsError> {
    ipc::server::run(Arc::clone(service), pipe_name).map_err(EsError::Io)
}

/// Non-Windows / development mode: stdin → NDJSON → stdout.
#[cfg(not(windows))]
fn run_stdio(service: &Arc<Mutex<EsService>>) -> Result<(), EsError> {
    use std::io::{BufRead as _, Write as _};

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let response = {
            let mut svc = service
                .lock()
                .map_err(|_| EsError::from(std::io::Error::other("service lock poisoned")))?;
            svc.handle_json_line(&line)
        };
        serde_json::to_writer(&mut stdout, &response)?;
        stdout.write_all(b"\n")?;

        let stop = service.lock().map(|s| s.should_shutdown()).unwrap_or(true);
        if stop {
            break;
        }
    }

    Ok(())
}

/// 指定された pipe name が既に別プロセスに占有されているかチェックする。
/// 接続できれば既存 daemon が動いている → true を返す。
fn is_pipe_already_taken(pipe_name: &str) -> bool {
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(pipe_name)
        .is_ok()
}
