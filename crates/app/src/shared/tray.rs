// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! System tray icon management with context menu (Settings, Exit).

#[cfg(windows)]
use windows::Win32::Foundation::HWND;
#[cfg(windows)]
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{LoadIconW, IDI_APPLICATION};

/// Custom message for tray icon callbacks.
pub const WM_TRAY_ICON: u32 = 0x0400 + 100; // WM_USER + 100

/// Context menu command IDs.
pub const IDM_SETTINGS: u32 = 2001;
pub const IDM_EXIT: u32 = 2002;

/// Tray icon ID.
const TRAY_ID: u32 = 1;

/// Add a system tray icon for the window.
#[cfg(windows)]
pub fn add_tray_icon(hwnd: HWND) -> bool {
    unsafe {
        let icon = LoadIconW(None, IDI_APPLICATION).unwrap_or_default();

        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_ID,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
            uCallbackMessage: WM_TRAY_ICON,
            hIcon: icon,
            ..Default::default()
        };

        // Set tooltip: "EasySearch"
        let tip = "EasySearch";
        let tip_wide: Vec<u16> = tip.encode_utf16().collect();
        let len = tip_wide.len().min(nid.szTip.len() - 1);
        nid.szTip[..len].copy_from_slice(&tip_wide[..len]);

        Shell_NotifyIconW(NIM_ADD, &nid).as_bool()
    }
}

/// Remove the system tray icon.
#[cfg(windows)]
pub fn remove_tray_icon(hwnd: HWND) {
    unsafe {
        let nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_ID,
            ..Default::default()
        };
        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
    }
}

/// Show the tray right-click context menu with "Settings" and "Exit" items.
#[cfg(windows)]
pub fn show_context_menu(hwnd: HWND) {
    use windows::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, SetForegroundWindow,
        TrackPopupMenu, MF_STRING, TPM_BOTTOMALIGN, TPM_LEFTALIGN,
    };
    use windows::Win32::Foundation::POINT;
    use windows::core::w;

    unsafe {
        let hmenu = CreatePopupMenu().unwrap_or_default();
        let _ = AppendMenuW(hmenu, MF_STRING, IDM_SETTINGS as usize, w!("Settings"));
        let _ = AppendMenuW(hmenu, MF_STRING, IDM_EXIT as usize, w!("Exit"));

        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);

        // Required: SetForegroundWindow so the menu dismisses when clicking away
        let _ = SetForegroundWindow(hwnd);
        let _ = TrackPopupMenu(
            hmenu,
            TPM_LEFTALIGN | TPM_BOTTOMALIGN,
            pt.x,
            pt.y,
            Some(0),
            hwnd,
            None,
        );
        let _ = DestroyMenu(hmenu);
    }
}
