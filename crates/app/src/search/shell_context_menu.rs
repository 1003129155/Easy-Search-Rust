// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Native Windows shell context menu integration for file and folder paths.

#[cfg(windows)]
use std::mem::size_of;

#[cfg(windows)]
use windows::Win32::Foundation::{HWND, POINT};
#[cfg(windows)]
use windows::Win32::System::Com::CoTaskMemFree;
#[cfg(windows)]
use windows::Win32::UI::Shell::{
    CMF_EXPLORE, CMF_NORMAL, CMINVOKECOMMANDINFO, IContextMenu, IShellFolder, SHBindToParent,
    SHParseDisplayName,
};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    CreatePopupMenu, DestroyMenu, GetCursorPos, PostMessageW, SetForegroundWindow, TPM_RETURNCMD,
    TPM_RIGHTBUTTON, TrackPopupMenu, WM_NULL,
};
#[cfg(windows)]
use windows::core::{PCSTR, PCWSTR};

#[cfg(windows)]
pub fn show_for_path(hwnd: HWND, path: &str, point: Option<POINT>) -> Result<(), String> {
    unsafe {
        let path_wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        let mut absolute_pidl = std::ptr::null_mut();
        SHParseDisplayName(
            PCWSTR(path_wide.as_ptr()),
            None,
            &mut absolute_pidl,
            0,
            None,
        )
        .map_err(|err| format!("SHParseDisplayName failed: {err}"))?;
        let _pidl_guard = PidlGuard(absolute_pidl);

        let mut child_pidl = std::ptr::null_mut();
        let parent_folder: IShellFolder = SHBindToParent(absolute_pidl, Some(&mut child_pidl))
            .map_err(|err| format!("SHBindToParent failed: {err}"))?;

        let child_items = [child_pidl as *const _];
        let context_menu: IContextMenu = parent_folder
            .GetUIObjectOf(hwnd, &child_items, None)
            .map_err(|err| format!("GetUIObjectOf(IContextMenu) failed: {err}"))?;

        let popup_menu =
            CreatePopupMenu().map_err(|err| format!("CreatePopupMenu failed: {err}"))?;
        let _menu_guard = MenuGuard(popup_menu);

        context_menu
            .QueryContextMenu(popup_menu, 0, 1, 0x7FFF, CMF_NORMAL | CMF_EXPLORE)
            .ok()
            .map_err(|err| format!("QueryContextMenu failed: {err}"))?;

        let screen_point = point.unwrap_or_else(cursor_point);
        let _ = SetForegroundWindow(hwnd);

        let selected = TrackPopupMenu(
            popup_menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            screen_point.x,
            screen_point.y,
            None,
            hwnd,
            None,
        )
        .0 as u32;

        if selected > 0 {
            let command_offset = (selected - 1) as usize;
            let invoke = CMINVOKECOMMANDINFO {
                cbSize: size_of::<CMINVOKECOMMANDINFO>() as u32,
                hwnd,
                lpVerb: PCSTR(command_offset as *const u8),
                nShow: 1,
                ..Default::default()
            };
            context_menu
                .InvokeCommand(&invoke)
                .map_err(|err| format!("InvokeCommand failed: {err}"))?;
        }

        let _ = PostMessageW(Some(hwnd), WM_NULL, Default::default(), Default::default());
        Ok(())
    }
}

#[cfg(windows)]
fn cursor_point() -> POINT {
    let mut point = POINT::default();
    let _ = unsafe { GetCursorPos(&mut point) };
    point
}

#[cfg(windows)]
struct PidlGuard(*mut windows::Win32::UI::Shell::Common::ITEMIDLIST);

#[cfg(windows)]
impl Drop for PidlGuard {
    fn drop(&mut self) {
        unsafe {
            CoTaskMemFree(Some(self.0 as _));
        }
    }
}

#[cfg(windows)]
struct MenuGuard(windows::Win32::UI::WindowsAndMessaging::HMENU);

#[cfg(windows)]
impl Drop for MenuGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyMenu(self.0);
        }
    }
}
