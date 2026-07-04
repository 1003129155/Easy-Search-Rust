// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Action execution — handles all result actions (Open, Copy, RunCommand, SystemCommand).

use easysearch_core::{Action, SystemCmd};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Execute an action from a plugin result.
#[cfg(windows)]
pub fn execute(action: &Action) {
    match action {
        Action::Open(target) => open_target(target),
        Action::Copy(text) => copy_to_clipboard(text),
        Action::RunCommand { cmd, keep_open } => run_command(cmd, *keep_open),
        Action::DaemonSearch(_query) => {
            // TODO: Will be handled via IPC in Phase 3
        }
        Action::SystemCommand(cmd) => execute_system_command(cmd),
        Action::None => {}
    }
}

/// Open a URL, file path, or ms-settings: URI via ShellExecuteW.
#[cfg(windows)]
fn open_target(target: &str) {
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use windows::core::PCWSTR;

    let target_wide: Vec<u16> = target.encode_utf16().chain(std::iter::once(0)).collect();
    let verb_wide: Vec<u16> = "open".encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        ShellExecuteW(
            None,
            PCWSTR(verb_wide.as_ptr()),
            PCWSTR(target_wide.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
    }
}

/// Copy text to the Windows clipboard.
#[cfg(windows)]
fn copy_to_clipboard(text: &str) {
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE,
    };
    use windows::Win32::System::Ole::CF_UNICODETEXT;

    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let size = wide.len() * 2;

    unsafe {
        if OpenClipboard(None).is_ok() {
            let _ = EmptyClipboard();

            if let Ok(hmem) = GlobalAlloc(GMEM_MOVEABLE, size) {
                let ptr = GlobalLock(hmem);
                if !ptr.is_null() {
                    std::ptr::copy_nonoverlapping(wide.as_ptr() as *const u8, ptr as *mut u8, size);
                    let _ = GlobalUnlock(hmem);
                    let _ = SetClipboardData(
                        CF_UNICODETEXT.0 as u32,
                        Some(windows::Win32::Foundation::HANDLE(hmem.0)),
                    );
                }
            }

            let _ = CloseClipboard();
        }
    }
}

/// Run a shell command via cmd.exe.
#[cfg(windows)]
fn run_command(cmd: &str, keep_open: bool) {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    let flag = if keep_open { "/k" } else { "/c" };
    let mut command = Command::new("cmd.exe");
    command.args([flag, cmd]);

    if !keep_open {
        command.creation_flags(CREATE_NO_WINDOW);
    }

    let _ = command.spawn();
}

/// Execute a system command (shutdown, lock, etc.) using Win32 APIs.
#[cfg(windows)]
fn execute_system_command(cmd: &SystemCmd) {
    match cmd {
        SystemCmd::Shutdown => {
            spawn_hidden("shutdown", &["/s", "/t", "0"]);
        }
        SystemCmd::Restart => {
            spawn_hidden("shutdown", &["/r", "/t", "0"]);
        }
        SystemCmd::Lock => {
            lock_workstation();
        }
        SystemCmd::Sleep => {
            set_suspend_state(false);
        }
        SystemCmd::Hibernate => {
            set_suspend_state(true);
        }
        SystemCmd::Logout => {
            use windows::Win32::System::Shutdown::ExitWindowsEx;
            use windows::Win32::System::Shutdown::{EXIT_WINDOWS_FLAGS, SHUTDOWN_REASON};
            unsafe {
                let _ = ExitWindowsEx(
                    EXIT_WINDOWS_FLAGS(0x00), // EWX_LOGOFF
                    SHUTDOWN_REASON(0),
                );
            }
        }
        SystemCmd::EmptyRecycleBin => {
            use windows::Win32::UI::Shell::SHEmptyRecycleBinW;
            use windows::core::PCWSTR;
            unsafe {
                let _ = SHEmptyRecycleBinW(None, PCWSTR::null(), 0);
            }
        }
    }
}

#[cfg(windows)]
fn spawn_hidden(program: &str, args: &[&str]) {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    let _ = Command::new(program)
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
}

#[cfg(windows)]
fn lock_workstation() {
    // user32.dll LockWorkStation
    use windows::Win32::System::Shutdown::LockWorkStation;
    unsafe {
        let _ = LockWorkStation();
    }
}

#[cfg(windows)]
fn set_suspend_state(hibernate: bool) {
    // powrprof.dll SetSuspendState
    use windows::Win32::System::Power::SetSuspendState;
    unsafe {
        let _ = SetSuspendState(hibernate, false, false);
    }
}
