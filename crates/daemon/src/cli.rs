// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! CLI モード: コマンドライン引数を解析し、daemon に named-pipe 経由でクエリを送信する。
//! C# フロントエンド (UffsApi.cs) が期待する NDJSON 形式で標準出力に結果を返す。

use std::io::{BufRead as _, BufReader, Write};
use std::time::Duration;

use crate::ipc::pipe_name_for_current_user;

/// CLI の解析結果。
#[derive(Debug)]
pub(crate) struct CliArgs {
    /// 検索パターン（第 1 位置引数）。
    pub(crate) pattern: String,
    /// 結果の上限数。
    pub(crate) limit: usize,
    /// フルパス検索モード。
    #[allow(dead_code)]
    pub(crate) name_only: bool,
    /// 親パスフィルタ。
    pub(crate) in_path: Option<String>,
    /// 出力フォーマット（"json" 固定）。
    pub(crate) _format: String,
}

impl CliArgs {
    /// コマンドライン引数を解析する。
    /// `--daemon` サブコマンドがある場合は `None` を返す（daemon モードとして処理）。
    pub(crate) fn parse() -> Option<Self> {
        let args: Vec<String> = std::env::args().skip(1).collect();

        if args.is_empty() {
            // 引数なし → daemon モード
            return None;
        }

        // `--daemon` サブコマンドチェック
        if args.iter().any(|a| a == "--daemon") {
            return None;
        }

        let mut pattern = String::new();
        let mut limit: usize = 100;
        let mut name_only = false;
        let mut in_path: Option<String> = None;
        let mut format = String::from("json");

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--limit" => {
                    i += 1;
                    if i < args.len() {
                        limit = args[i].parse().unwrap_or(100);
                    }
                }
                "--format" => {
                    i += 1;
                    if i < args.len() {
                        format = args[i].clone();
                    }
                }
                "--name-only" => {
                    name_only = true;
                }
                "--in-path" => {
                    i += 1;
                    if i < args.len() {
                        in_path = Some(args[i].clone());
                    }
                }
                other => {
                    if pattern.is_empty() {
                        pattern = other.to_string();
                    }
                }
            }
            i += 1;
        }

        if pattern.is_empty() {
            pattern = String::from("*");
        }

        Some(Self {
            pattern,
            limit,
            name_only,
            in_path,
            _format: format,
        })
    }
}

/// CLI モードのエントリポイント。daemon に接続してクエリを送信し、
/// NDJSON を標準出力に書き出す。
pub(crate) fn run(args: CliArgs) -> Result<(), Box<dyn std::error::Error>> {
    let pipe_name = std::env::var("EASYSEARCH_PIPE")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(pipe_name_for_current_user);

    // daemon に接続を試みる（最大 5 回リトライ）
    let pipe = connect_with_retry(&pipe_name, 5)?;

    let mut writer = pipe;
    let reader_handle = writer.try_clone()?;
    let mut reader = BufReader::new(reader_handle);

    // インデックス構築完了を待つ（最大 90 秒）
    wait_for_ready(&mut writer, &mut reader)?;

    // リクエスト構築
    let request = build_request(&args);
    let request_json = serde_json::to_string(&request)?;

    // リクエスト送信
    writer.write_all(request_json.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()?;

    // レスポンス読み取り
    let mut response_line = String::new();
    reader.read_line(&mut response_line)?;

    // レスポンスを解析して NDJSON 形式で出力
    output_results(&response_line, &mut std::io::stdout().lock())?;

    Ok(())
}

/// daemon のインデックス構築完了を待つ。
/// `status` リクエストを送信し、`ready=true` になるまでポーリングする。
fn wait_for_ready(
    writer: &mut std::fs::File,
    reader: &mut BufReader<std::fs::File>,
) -> Result<(), Box<dyn std::error::Error>> {
    let max_wait = Duration::from_secs(90);
    let poll_interval = Duration::from_millis(500);
    let start = std::time::Instant::now();

    loop {
        let status_req = serde_json::json!({
            "id": 0,
            "method": "status"
        });
        let json = serde_json::to_string(&status_req)?;
        writer.write_all(json.as_bytes())?;
        writer.write_all(b"\n")?;
        writer.flush()?;

        let mut line = String::new();
        reader.read_line(&mut line)?;

        if let Ok(resp) = serde_json::from_str::<serde_json::Value>(line.trim()) {
            let ready = resp.get("ready").and_then(|v| v.as_bool()).unwrap_or(false);
            if ready {
                return Ok(());
            }
        }

        if start.elapsed() > max_wait {
            // タイムアウトしてもクエリは試みる（部分的な結果が返る可能性あり）
            eprintln!("[easysearch] warning: index not ready after {:?}, proceeding anyway", max_wait);
            return Ok(());
        }

        std::thread::sleep(poll_interval);
    }
}

/// daemon の named pipe に接続する。失敗時はリトライする。
fn connect_with_retry(
    pipe_name: &str,
    max_retries: u32,
) -> Result<std::fs::File, Box<dyn std::error::Error>> {
    // まず既存の daemon への接続を試みる
    if let Ok(file) = open_pipe(pipe_name) {
        return Ok(file);
    }

    // 接続失敗 → daemon が起動していない可能性あり
    // 既に easysearch プロセスが存在するかチェック（重複起動防止）
    if !is_daemon_process_running() {
        spawn_daemon();
    }

    // daemon の起動を待ってリトライ
    for attempt in 0..=max_retries {
        std::thread::sleep(Duration::from_millis(800 * u64::from(attempt + 1)));
        match open_pipe(pipe_name) {
            Ok(file) => return Ok(file),
            Err(_) if attempt < max_retries => continue,
            Err(e) => return Err(e),
        }
    }
    Err("failed to connect to daemon after retries".into())
}

/// daemon プロセスが既に実行中かどうかを確認する（重複起動防止）。
fn is_daemon_process_running() -> bool {
    #[cfg(windows)]
    {
        use std::process::Command;
        // tasklist で easysearch.exe のプロセスを探す
        let output = Command::new("tasklist")
            .args(["/fi", "imagename eq easysearch.exe", "/fo", "csv", "/nh"])
            .output();
        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                stdout.contains("easysearch.exe")
            }
            Err(_) => false,
        }
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// named pipe をファイルとして開く。
fn open_pipe(pipe_name: &str) -> Result<std::fs::File, Box<dyn std::error::Error>> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(pipe_name)?;
    Ok(file)
}

/// 自分自身を daemon モード（引数なし）でバックグラウンド起動する。
fn spawn_daemon() {
    let exe = std::env::current_exe().unwrap_or_default();
    if !exe.exists() {
        return;
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        let _ = std::process::Command::new(&exe)
            .arg("--daemon")
            .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
            .spawn();
    }

    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new(&exe)
            .arg("--daemon")
            .spawn();
    }
}

/// CLI 引数からプロトコルリクエストを構築する。
fn build_request(args: &CliArgs) -> serde_json::Value {
    // クエリはそのまま daemon に渡す（変換は daemon 側で実施）
    let query = &args.pattern;

    // `in_path` がある場合は `enumerate`、なければ `search`
    if let Some(ref path) = args.in_path {
        let base_path = path.trim_end_matches('*').trim_end_matches('\\');
        serde_json::json!({
            "id": 1,
            "method": "enumerate",
            "query": query,
            "path": base_path,
            "recursive": true,
            "limit": args.limit
        })
    } else {
        serde_json::json!({
            "id": 1,
            "method": "search",
            "query": query,
            "limit": args.limit
        })
    }
}

/// Everything 互換の glob/正規表現パターンを Engine の子文字列検索形式に変換する。
/// ※ 現在は daemon 側で変換するため、この関数は将来の拡張用に残す。
#[allow(dead_code)]
fn translate_pattern(pattern: &str) -> String {
    pattern.to_string()
}

/// daemon のレスポンスを解析し、各結果を 1 行 1 JSON で出力する（NDJSON）。
/// C# フロントエンドが期待するフィールド: `path`, `is_directory`
fn output_results(
    response_line: &str,
    out: &mut impl Write,
) -> Result<(), Box<dyn std::error::Error>> {
    let response: serde_json::Value = serde_json::from_str(response_line.trim())?;

    if let Some(items) = response.get("items").and_then(|v| v.as_array()) {
        for item in items {
            // C# 側が期待する最小フィールドセット
            let entry = serde_json::json!({
                "Path": item.get("path").and_then(|v| v.as_str()).unwrap_or(""),
                "is_directory": item.get("is_directory").and_then(|v| v.as_bool()).unwrap_or(false),
            });
            serde_json::to_writer(&mut *out, &entry)?;
            out.write_all(b"\n")?;
        }
        out.flush()?;
    } else if let Some(error) = response.get("error").and_then(|v| v.as_str()) {
        eprintln!("[easysearch] error: {error}");
    }

    Ok(())
}
