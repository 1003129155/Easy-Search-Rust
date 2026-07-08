// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! System tray icon management with localized context menu.

#[cfg(windows)]
use windows::Win32::Foundation::HWND;
#[cfg(windows)]
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{LoadIconW, LoadImageW, IDI_APPLICATION, IMAGE_ICON, LR_DEFAULTSIZE, LR_SHARED};
#[cfg(windows)]
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
#[cfg(windows)]
use windows::core::PCWSTR;

pub const WM_TRAY_ICON: u32 = 0x0400 + 100;
pub const IDM_SETTINGS: u32 = 2001;
pub const IDM_EXIT: u32 = 2002;

const TRAY_ID: u32 = 1;

/// Load the embedded app icon (resource ID 1) from the current exe.
/// Falls back to IDI_APPLICATION if not available.
#[cfg(windows)]
pub fn load_app_icon() -> windows::Win32::UI::WindowsAndMessaging::HICON {
    unsafe {
        let hinstance = GetModuleHandleW(PCWSTR::null()).unwrap_or_default();
        // MAKEINTRESOURCE(1) = resource ID 1
        let icon_id = PCWSTR(1 as *const u16);
        let hicon = LoadImageW(
            Some(hinstance.into()),
            icon_id,
            IMAGE_ICON,
            0, 0,
            LR_DEFAULTSIZE | LR_SHARED,
        );
        match hicon {
            Ok(h) => windows::Win32::UI::WindowsAndMessaging::HICON(h.0),
            Err(_) => LoadIconW(None, IDI_APPLICATION).unwrap_or_default(),
        }
    }
}

fn current_i18n() -> crate::i18n::engine::I18nEngine {
    let locale = crate::SHARED_SETTINGS
        .get()
        .and_then(|settings| settings.read().ok().map(|s| s.language.clone()))
        .filter(|locale| !locale.is_empty());

    match locale {
        Some(locale) => crate::i18n::engine::I18nEngine::with_locale(&locale),
        None => crate::i18n::engine::I18nEngine::new(),
    }
}

#[cfg(windows)]
pub fn add_tray_icon(hwnd: HWND) -> bool {
    let i18n = current_i18n();

    unsafe {
        let icon = load_app_icon();

        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_ID,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
            uCallbackMessage: WM_TRAY_ICON,
            hIcon: icon,
            ..Default::default()
        };

        let tip_wide: Vec<u16> = i18n.get("tray_tooltip").encode_utf16().collect();
        let len = tip_wide.len().min(nid.szTip.len() - 1);
        nid.szTip[..len].copy_from_slice(&tip_wide[..len]);

        Shell_NotifyIconW(NIM_ADD, &nid).as_bool()
    }
}

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

#[cfg(windows)]
pub fn show_context_menu(hwnd: HWND) {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, MF_STRING, SetForegroundWindow,
        TPM_BOTTOMALIGN, TPM_LEFTALIGN, TrackPopupMenu,
    };

    let i18n = current_i18n();
    let settings_label: Vec<u16> = i18n
        .get("tray_settings")
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let exit_label: Vec<u16> = i18n
        .get("tray_exit")
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let hmenu = CreatePopupMenu().unwrap_or_default();
        let _ = AppendMenuW(
            hmenu,
            MF_STRING,
            IDM_SETTINGS as usize,
            windows::core::PCWSTR(settings_label.as_ptr()),
        );
        let _ = AppendMenuW(
            hmenu,
            MF_STRING,
            IDM_EXIT as usize,
            windows::core::PCWSTR(exit_label.as_ptr()),
        );

        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);

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
