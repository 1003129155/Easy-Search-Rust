// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! File preview panel — shows file details when a result is selected.
//!
//! Reference: Flow.Launcher's Explorer plugin PreviewPanel which displays:
//! - File icon (large, 96x96)
//! - Filename
//! - File path
//! - File size
//! - Created date
//! - Last modified date
//!
//! This is rendered in the right side of the search window (like Flow.Launcher's
//! Preview column with 0.85* width).

use std::path::Path;
use std::time::SystemTime;

/// Preview information for the currently selected result.
#[derive(Debug, Clone, Default)]
pub struct PreviewInfo {
    /// Full file path being previewed.
    pub path: String,
    /// Filename (basename).
    pub filename: String,
    /// Whether it's a directory.
    pub is_directory: bool,
    /// Human-readable file size (e.g. "1.23 MB").
    pub file_size: String,
    /// Created date formatted string.
    pub created_at: String,
    /// Last modified date formatted string.
    pub modified_at: String,
}

impl PreviewInfo {
    /// Build preview info for a file/directory path.
    /// Returns None if the path doesn't exist or can't be read.
    pub fn from_path(path: &str) -> Option<Self> {
        let p = Path::new(path);
        let metadata = std::fs::metadata(p).ok()?;

        let filename = p
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string());

        let is_directory = metadata.is_dir();

        let file_size = if is_directory {
            // Don't compute folder size (can be slow)
            String::from("—")
        } else {
            format_size(metadata.len())
        };

        let created_at = metadata
            .created()
            .ok()
            .map(format_time)
            .unwrap_or_default();

        let modified_at = metadata
            .modified()
            .ok()
            .map(format_time)
            .unwrap_or_default();

        Some(Self {
            path: path.to_string(),
            filename,
            is_directory,
            file_size,
            created_at,
            modified_at,
        })
    }
}

/// Format bytes into human-readable size.
/// Flow.Launcher uses ResultManager.ToReadableSize with 2 decimals.
fn format_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    const TB: f64 = GB * 1024.0;

    let size = bytes as f64;
    if size < KB {
        format!("{} B", bytes)
    } else if size < MB {
        format!("{:.2} KB", size / KB)
    } else if size < GB {
        format!("{:.2} MB", size / MB)
    } else if size < TB {
        format!("{:.2} GB", size / GB)
    } else {
        format!("{:.2} TB", size / TB)
    }
}

/// Format a SystemTime into a human-readable date/time string.
fn format_time(time: SystemTime) -> String {
    // Convert to seconds since UNIX epoch then format
    let duration = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs() as i64;

    // Simple date formatting without chrono dependency
    // Use Windows API for proper locale-aware formatting
    #[cfg(windows)]
    {
        format_time_windows(secs)
    }
    #[cfg(not(windows))]
    {
        let _ = secs;
        String::from("N/A")
    }
}

/// Use Win32 `FileTimeToSystemTime` for locale-aware formatting.
#[cfg(windows)]
fn format_time_windows(unix_secs: i64) -> String {
    use windows::Win32::Foundation::FILETIME;
    use windows::Win32::System::Time::FileTimeToSystemTime;

    // Convert UNIX seconds to FILETIME (100-nanosecond intervals since 1601-01-01)
    const EPOCH_DIFF: i64 = 116_444_736_000_000_000; // 100ns intervals between 1601 and 1970
    let ft_val = (unix_secs * 10_000_000) + EPOCH_DIFF;
    let filetime = FILETIME {
        dwLowDateTime: ft_val as u32,
        dwHighDateTime: (ft_val >> 32) as u32,
    };

    let mut systime = windows::Win32::Foundation::SYSTEMTIME::default();
    let ok = unsafe { FileTimeToSystemTime(&filetime, &mut systime) };
    if !ok.is_ok() {
        return String::new();
    }

    // Simple formatting: YYYY-MM-DD HH:MM
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        systime.wYear, systime.wMonth, systime.wDay, systime.wHour, systime.wMinute
    )
}
