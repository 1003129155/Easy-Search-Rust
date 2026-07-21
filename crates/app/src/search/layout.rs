// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Layout constants and calculations for the search window.
//! Values are aligned with Flow.Launcher Win11Light theme.

#[cfg(windows)]
use windows::Win32::Foundation::HWND;

/// Get DPI scale factor for a given window.
/// Returns 1.0 for 96 DPI (100%), 1.25 for 120 DPI (125%), 1.5 for 144 DPI (150%), etc.
#[cfg(windows)]
pub fn dpi_scale(hwnd: HWND) -> f32 {
    use windows::Win32::UI::HiDpi::GetDpiForWindow;
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    if dpi == 0 { 1.0 } else { dpi as f32 / 96.0 }
}

/// Scale a logical pixel value to physical pixels for the given window's DPI.
#[cfg(windows)]
pub fn scale(value: f32, hwnd: HWND) -> i32 {
    (value * dpi_scale(hwnd)).round() as i32
}

/// Scale a logical pixel value with a pre-computed DPI scale factor.
#[cfg(windows)]
pub fn scale_with(value: f32, dpi_factor: f32) -> i32 {
    (value * dpi_factor).round() as i32
}

/// Height of the search input bar in pixels.
/// Flow.Launcher: QueryBox Height=42 + padding = ~48px total area.
pub const SEARCH_BAR_HEIGHT: f32 = 48.0;

/// Height of each result item in pixels.
/// Flow.Launcher Win11Light: ResultItemHeight=58.
pub const ITEM_HEIGHT: f32 = 58.0;

/// Maximum number of visible result items.
/// Flow.Launcher default is 5 (configurable up to 17).
pub const MAX_VISIBLE_ITEMS: usize = 5;

/// Window width in pixels.
/// Flow.Launcher default: 600px. We keep slightly wider for CJK text.
pub const WINDOW_WIDTH: f32 = 600.0;

/// Horizontal padding inside the search bar.
/// Flow.Launcher: QueryBox Margin="16 7 0 7".
pub const PADDING_H: f32 = 16.0;

/// Icon area width (column 0 in Flow.Launcher result item).
/// Flow.Launcher: ImageAreaWidth = 60px.
#[allow(dead_code)]
pub const ICON_AREA_WIDTH: f32 = 60.0;

/// Left padding for text (after icon area).
/// Flow.Launcher: Grid Column="1" with Margin="6 0 10 0".
pub const TEXT_LEFT: f32 = 66.0;

/// Icon size (square).
/// Flow.Launcher: ImageIconStyle Height/Width = 32.
pub const ICON_SIZE: f32 = 32.0;

/// Icon left margin within the icon area.
/// Flow.Launcher: Bullet(4px) + Border Margin(9,0,0,0) = icon at ~13px.
pub const ICON_LEFT: f32 = 13.0;

/// Indicator bar (bullet) width for selected item.
/// Flow.Launcher Win11Light: BulletStyle Width=4.
pub const INDICATOR_WIDTH: f32 = 4.0;

/// Indicator bar (bullet) height.
/// Flow.Launcher Win11Light: BulletStyle Height=38.
pub const INDICATOR_HEIGHT: f32 = 38.0;

/// Indicator bar corner radius.
/// Flow.Launcher Win11Light: ItemBulletSelectedStyle CornerRadius=2.
pub const INDICATOR_CORNER_RADIUS: f32 = 2.0;

/// Corner radius for the window border.
/// Flow.Launcher: BaseWindowBorderStyle CornerRadius=5.
#[allow(dead_code)]
pub const CORNER_RADIUS: f32 = 5.0;

/// Corner radius for selected item highlight.
/// Flow.Launcher Win11Light: ItemRadius=5.
pub const ITEM_CORNER_RADIUS: f32 = 5.0;

/// Item horizontal margin (left/right).
/// Flow.Launcher Win11Light: ItemMargin="10 0 10 0".
pub const ITEM_MARGIN_H: f32 = 10.0;

/// Result list vertical margin (top/bottom padding).
/// Flow.Launcher Win11Light: ResultMargin="0 10 0 10".
pub const RESULT_MARGIN_V: f32 = 10.0;

/// Separator line height.
/// Flow.Launcher Win11Light: SeparatorStyle Height=1.
pub const SEPARATOR_HEIGHT: f32 = 1.0;

/// Title font size.
/// Flow.Launcher: BaseItemTitleStyle FontSize=16.
pub const TITLE_FONT_SIZE: f32 = 16.0;

/// Subtitle font size.
/// Flow.Launcher: BaseItemSubTitleStyle FontSize=13.
pub const SUBTITLE_FONT_SIZE: f32 = 13.0;

/// Input font size.
/// Flow.Launcher Win11Light: QueryBoxStyle FontSize=16.
pub const INPUT_FONT_SIZE: f32 = 16.0;

/// Hotkey font size.
/// Flow.Launcher Win11Light: ItemHotkeyStyle FontSize=11.
#[allow(dead_code)]
pub const HOTKEY_FONT_SIZE: f32 = 11.0;

/// Preview panel height (filename + 3 detail lines + padding).
pub const PREVIEW_PANEL_HEIGHT: f32 = 88.0;

/// Calculate total window height based on the number of results.
pub fn window_height(result_count: usize) -> f32 {
    let items = result_count.min(MAX_VISIBLE_ITEMS) as f32;
    if result_count == 0 {
        SEARCH_BAR_HEIGHT
    } else {
        SEARCH_BAR_HEIGHT
            + SEPARATOR_HEIGHT
            + RESULT_MARGIN_V
            + items * ITEM_HEIGHT
            + RESULT_MARGIN_V
    }
}

/// Calculate total window height including preview panel.
pub fn window_height_with_preview(result_count: usize, has_preview: bool) -> f32 {
    let base = window_height(result_count);
    if has_preview && result_count > 0 {
        base + PREVIEW_PANEL_HEIGHT
    } else {
        base
    }
}

/// Calculate total window height with preview (physical pixels) scaled for DPI.
#[cfg(windows)]
pub fn window_height_with_preview_scaled(
    result_count: usize,
    has_preview: bool,
    hwnd: HWND,
) -> i32 {
    scale(window_height_with_preview(result_count, has_preview), hwnd)
}

/// Window width in physical pixels scaled for DPI.
#[cfg(windows)]
pub fn window_width_scaled(hwnd: HWND) -> i32 {
    scale(WINDOW_WIDTH, hwnd)
}
