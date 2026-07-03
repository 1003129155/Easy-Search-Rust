// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Native Win32 process enumeration using ToolHelp32 API.
//!
//! This avoids shelling out to `tasklist.exe` and provides richer data:
//! - Process name, PID, executable path
//! - Window titles via EnumWindows

use super::ProcessEntry;
use std::collections::HashMap;
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, MAX_PATH};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
};

/// Enumerate all running processes using CreateToolhelp32Snapshot.
pub fn enumerate_processes_native() -> Vec<ProcessEntry> {
    let mut entries = Vec::new();

    // Get window titles mapping: PID -> window title
    let window_titles = get_window_titles();

    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return entries;
        };

        let mut pe: PROCESSENTRY32W = std::mem::zeroed();
        pe.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

        if Process32FirstW(snapshot, &mut pe).is_ok() {
            loop {
                let name = wchar_to_string(&pe.szExeFile);
                let pid = pe.th32ProcessID;

                if pid != 0 {
                    let path = get_process_path(pid);
                    let window_title = window_titles.get(&pid).cloned();

                    entries.push(ProcessEntry {
                        pid,
                        name,
                        path,
                        window_title,
                    });
                }

                if Process32NextW(snapshot, &mut pe).is_err() {
                    break;
                }
            }
        }

        let _ = CloseHandle(snapshot);
    }

    entries
}

/// Get the full image path for a process (best-effort).
fn get_process_path(pid: u32) -> Option<String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;

        let mut buf = [0u16; MAX_PATH as usize];
        let mut size = buf.len() as u32;

        let result = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_FORMAT(0),
            windows::core::PWSTR(buf.as_mut_ptr()),
            &mut size,
        );

        let _ = CloseHandle(handle);

        if result.is_ok() && size > 0 {
            Some(String::from_utf16_lossy(&buf[..size as usize]))
        } else {
            None
        }
    }
}

/// Get a mapping of PID -> window title for all top-level windows.
fn get_window_titles() -> HashMap<u32, String> {
    let mut map: HashMap<u32, String> = HashMap::new();

    unsafe {
        // Use EnumWindows to iterate visible top-level windows
        let _ = windows::Win32::UI::WindowsAndMessaging::EnumWindows(
            Some(enum_window_callback),
            LPARAM(&mut map as *mut HashMap<u32, String> as isize),
        );
    }

    map
}

/// Callback for EnumWindows — collects PID -> window title.
unsafe extern "system" fn enum_window_callback(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowTextLengthW, GetWindowTextW, IsWindowVisible,
    };

    unsafe {
        // Only visible windows
        if !IsWindowVisible(hwnd).as_bool() {
            return windows::core::BOOL(1); // continue
        }

        let text_len = GetWindowTextLengthW(hwnd);
        if text_len == 0 {
            return windows::core::BOOL(1); // continue — no title
        }

        // Get window title
        let mut buf = vec![0u16; (text_len + 1) as usize];
        let copied = GetWindowTextW(hwnd, &mut buf);
        if copied == 0 {
            return windows::core::BOOL(1);
        }
        let title = String::from_utf16_lossy(&buf[..copied as usize]);

        // Get owning process ID
        let mut pid: u32 = 0;
        windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(hwnd, Some(&mut pid));

        if pid != 0 && !title.is_empty() {
            let map = &mut *(lparam.0 as *mut HashMap<u32, String>);
            // Only keep the first (topmost) title per PID
            map.entry(pid).or_insert(title);
        }

        windows::core::BOOL(1) // continue enumeration
    }
}

/// Convert a null-terminated u16 buffer to a Rust String.
fn wchar_to_string(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}
