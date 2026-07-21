// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Direct2D renderer for the search window.
//! Supports selection highlight, cursor blinking, and index status display.

#[cfg(windows)]
use windows::Win32::Foundation::{D2DERR_RECREATE_TARGET, HWND};
#[cfg(windows)]
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D_SIZE_U, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
};
#[cfg(windows)]
use windows::Win32::Graphics::Direct2D::{
    D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_FACTORY_TYPE_SINGLE_THREADED,
    D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_PROPERTIES, D2D1_ROUNDED_RECT,
    D2D1CreateFactory, ID2D1Factory1, ID2D1HwndRenderTarget, ID2D1SolidColorBrush,
};
#[cfg(windows)]
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_MEASURING_MODE_NATURAL,
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat,
};
#[cfg(windows)]
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
#[cfg(windows)]
use windows::core::Interface;
#[cfg(windows)]
use windows::core::PCWSTR;
#[cfg(windows)]
use windows_numerics;

use super::layout;
// TODO: Replace legacy crate::theme::{Color, Theme} with ThemeEngine::current_theme().colors
// when fully integrated. The legacy module provides the same colors as the new engine's builtins.
use crate::theme::{Color, Theme};

/// Display item (either from daemon or from local plugin).
#[derive(Debug, Clone)]
pub struct DisplayItem {
    pub title: String,
    pub subtitle: String,
    #[allow(dead_code)]
    pub icon: String,
    pub shortcut: String,
    pub action: easysearch_core::Action,
    pub context_actions: Vec<easysearch_core::ContextAction>,
    pub context_data: Option<easysearch_core::ContextData>,
    /// File path for icon extraction (None for plugin-only results).
    pub icon_path: Option<String>,
    /// Whether this item represents a directory.
    pub is_directory: bool,
    /// Highlight ranges in the title as `[start_byte, len_bytes]`.
    pub highlight: Vec<[u32; 2]>,
    /// Score for unified ranking (higher = shown first).
    pub score: u32,
}

/// Renderer state holding D2D and DWrite resources.
#[cfg(windows)]
pub struct Renderer {
    factory: ID2D1Factory1,
    dwrite_factory: IDWriteFactory,
    text_format_title: IDWriteTextFormat,
    text_format_subtitle: IDWriteTextFormat,
    text_format_input: IDWriteTextFormat,
    pub theme: Theme,
    target: TargetState,
    device: Option<DeviceResources>,
}

/// Resources whose lifetime is tied to the current Direct2D render target.
///
/// Keep every render-target-domain object in this one owner so a device loss
/// can discard the complete resource domain with a single `Option::take`.
#[cfg(windows)]
struct DeviceResources {
    // Drop target-created bitmaps before releasing the render target itself.
    icon_cache: super::icon::IconCache,
    render_target: ID2D1HwndRenderTarget,
}

#[cfg(windows)]
#[derive(Clone, Copy)]
struct TargetState {
    hwnd: HWND,
    width: u32,
    height: u32,
    dpi_x: u32,
    dpi_y: u32,
}

#[cfg(windows)]
impl Renderer {
    /// Initialize renderer with D2D factory and DWrite.
    pub fn new(
        hwnd: HWND,
        width: u32,
        height: u32,
        dpi_x: u32,
        dpi_y: u32,
    ) -> Result<Self, String> {
        let factory: ID2D1Factory1 =
            unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None) }
                .map_err(|e| format!("D2D1CreateFactory failed: {e}"))?;

        let dwrite_factory: IDWriteFactory =
            unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }
                .map_err(|e| format!("DWriteCreateFactory failed: {e}"))?;

        let font_family = wide_str("Microsoft YaHei UI");
        let locale = wide_str("zh-cn");

        // Title: 16px semi-bold (Flow.Launcher BaseItemTitleStyle)
        let text_format_title = unsafe {
            dwrite_factory.CreateTextFormat(
                PCWSTR(font_family.as_ptr()),
                None,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                layout::TITLE_FONT_SIZE,
                PCWSTR(locale.as_ptr()),
            )
        }
        .map_err(|e| format!("CreateTextFormat (title) failed: {e}"))?;

        // Subtitle: 13px normal (Flow.Launcher BaseItemSubTitleStyle)
        let text_format_subtitle = unsafe {
            dwrite_factory.CreateTextFormat(
                PCWSTR(font_family.as_ptr()),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                layout::SUBTITLE_FONT_SIZE,
                PCWSTR(locale.as_ptr()),
            )
        }
        .map_err(|e| format!("CreateTextFormat (subtitle) failed: {e}"))?;

        // Input: 16px normal (Flow.Launcher Win11Light QueryBoxStyle)
        let text_format_input = unsafe {
            dwrite_factory.CreateTextFormat(
                PCWSTR(font_family.as_ptr()),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                layout::INPUT_FONT_SIZE,
                PCWSTR(locale.as_ptr()),
            )
        }
        .map_err(|e| format!("CreateTextFormat (input) failed: {e}"))?;

        let mut renderer = Self {
            factory,
            dwrite_factory,
            text_format_title,
            text_format_subtitle,
            text_format_input,
            theme: Theme::system(),
            target: TargetState {
                hwnd,
                width,
                height,
                dpi_x,
                dpi_y,
            },
            device: None,
        };
        renderer.ensure_device_resources()?;
        Ok(renderer)
    }

    fn build_device_resources(&self) -> Result<DeviceResources, String> {
        let target = self.target;
        let render_props = D2D1_RENDER_TARGET_PROPERTIES {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            ..Default::default()
        };

        let hwnd_props = D2D1_HWND_RENDER_TARGET_PROPERTIES {
            hwnd: target.hwnd,
            pixelSize: D2D_SIZE_U {
                width: target.width,
                height: target.height,
            },
            ..Default::default()
        };

        let rt = unsafe {
            self.factory
                .CreateHwndRenderTarget(&render_props, &hwnd_props)
        }
        .map_err(|e| format!("CreateHwndRenderTarget failed: {e}"))?;

        unsafe {
            rt.SetDpi(target.dpi_x as f32, target.dpi_y as f32);
        }

        Ok(DeviceResources {
            icon_cache: super::icon::IconCache::new(),
            render_target: rt,
        })
    }

    fn ensure_device_resources(&mut self) -> Result<(), String> {
        if self.device.is_none() && self.target.width > 0 && self.target.height > 0 {
            self.device = Some(self.build_device_resources()?);
        }
        Ok(())
    }

    /// Resize the render target.
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }

        self.target.width = width;
        self.target.height = height;

        let resize_error = self.device.as_ref().and_then(|device| {
            let size = D2D_SIZE_U { width, height };
            unsafe { device.render_target.Resize(&size) }.err()
        });
        if let Some(error) = resize_error {
            easysearch_core::log_warn!(
                "Direct2D Resize failed ({}); discarding device resources",
                error.code()
            );
            self.device.take();
        }
    }

    /// Keep Direct2D's DIP-to-pixel mapping in sync with the HWND.
    ///
    /// Resizing the render target alone does not update its DPI. Without this,
    /// a live Windows scale change (for example 100% -> 125%) enlarges the
    /// window but Direct2D continues drawing at the old scale.
    pub fn set_dpi(&mut self, dpi_x: u32, dpi_y: u32) {
        self.target.dpi_x = dpi_x;
        self.target.dpi_y = dpi_y;
        if let Some(ref device) = self.device {
            unsafe {
                device.render_target.SetDpi(dpi_x as f32, dpi_y as f32);
            }
        }
    }

    /// Render the full search UI.
    pub fn render(
        &mut self,
        input_text: &str,
        cursor_pos: usize,
        selection_range: (usize, usize),
        has_selection: bool,
        items: &[DisplayItem],
        selected_index: usize,
        scroll_offset: usize,
        placeholder: &str,
        anim_progress: f32,
        search_active: bool,
        preview: Option<(&super::preview::PreviewInfo, f32)>,
        input_focused: bool,
        cursor_moved_at: u128,
    ) -> Result<(), String> {
        for attempt in 0..2 {
            self.ensure_device_resources()?;
            let Some(mut device) = self.device.take() else {
                return Ok(());
            };

            let result = self.render_frame(
                &mut device,
                input_text,
                cursor_pos,
                selection_range,
                has_selection,
                items,
                selected_index,
                scroll_offset,
                placeholder,
                anim_progress,
                search_active,
                preview,
                input_focused,
                cursor_moved_at,
            );

            match result {
                Ok(()) => {
                    self.device = Some(device);
                    return Ok(());
                }
                Err(error) if error.code() == D2DERR_RECREATE_TARGET => {
                    easysearch_core::log_warn!(
                        "Direct2D device lost during EndDraw; rebuilding device resources"
                    );
                    // `device` is deliberately not put back. Its render target
                    // and every target-created bitmap are discarded together.
                    if attempt == 1 {
                        return Err(format!(
                            "EndDraw still reports device loss after rebuild: {error}"
                        ));
                    }
                }
                Err(error) => {
                    return Err(format!("EndDraw failed: {error}"));
                }
            }
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn render_frame(
        &self,
        device: &mut DeviceResources,
        input_text: &str,
        cursor_pos: usize,
        selection_range: (usize, usize),
        has_selection: bool,
        items: &[DisplayItem],
        selected_index: usize,
        scroll_offset: usize,
        placeholder: &str,
        anim_progress: f32,
        search_active: bool,
        preview: Option<(&super::preview::PreviewInfo, f32)>,
        input_focused: bool,
        cursor_moved_at: u128,
    ) -> windows::core::Result<()> {
        let rt = &device.render_target;
        let icon_cache = &mut device.icon_cache;

        unsafe {
            rt.BeginDraw();

            // Clear background
            rt.Clear(Some(&color_to_d2d(&self.theme.background)));

            // Draw search bar background
            let search_rect = D2D_RECT_F {
                left: 0.0,
                top: 0.0,
                right: layout::WINDOW_WIDTH,
                bottom: layout::SEARCH_BAR_HEIGHT,
            };
            if let Ok(brush) = self.create_brush(rt, &self.theme.search_bg) {
                rt.FillRectangle(&search_rect, &brush);
            }

            // Draw input text or placeholder
            // Vertically center: 48px bar, ~22px text height → top = (48-22)/2 = 13
            let text_top = (layout::SEARCH_BAR_HEIGHT - 22.0) / 2.0;
            let text_rect = D2D_RECT_F {
                left: layout::PADDING_H,
                top: text_top,
                right: layout::WINDOW_WIDTH - layout::PADDING_H,
                bottom: text_top + 22.0,
            };

            if input_text.is_empty() {
                // Show placeholder text (localized via i18n)
                if let Ok(brush) = self.create_brush(rt, &self.theme.placeholder) {
                    let text_wide = wide_str(placeholder);
                    rt.DrawText(
                        &text_wide[..text_wide.len() - 1],
                        &self.text_format_input,
                        &text_rect,
                        &brush,
                        D2D1_DRAW_TEXT_OPTIONS_NONE,
                        DWRITE_MEASURING_MODE_NATURAL,
                    );
                }
            } else {
                // ── Draw selection highlight ──────────────────────────────────
                if has_selection {
                    let (sel_start, sel_end) = selection_range;
                    let x_start = self.measure_text_width(input_text, sel_start);
                    let x_end = self.measure_text_width(input_text, sel_end);

                    let selection_color = Color {
                        r: 0.26,
                        g: 0.56,
                        b: 0.96,
                        a: 0.3,
                    };
                    if let Ok(brush) = self.create_brush(rt, &selection_color) {
                        let sel_rect = D2D_RECT_F {
                            left: layout::PADDING_H + x_start,
                            top: (layout::SEARCH_BAR_HEIGHT - 24.0) / 2.0,
                            right: layout::PADDING_H + x_end,
                            bottom: (layout::SEARCH_BAR_HEIGHT + 24.0) / 2.0,
                        };
                        rt.FillRectangle(&sel_rect, &brush);
                    }
                }

                // ── Draw input text ──────────────────────────────────────────
                if let Ok(brush) = self.create_brush(rt, &self.theme.text_primary) {
                    let text_wide = wide_str(input_text);
                    rt.DrawText(
                        &text_wide[..text_wide.len() - 1],
                        &self.text_format_input,
                        &text_rect,
                        &brush,
                        D2D1_DRAW_TEXT_OPTIONS_NONE,
                        DWRITE_MEASURING_MODE_NATURAL,
                    );
                }

                // ── Draw cursor (blinking) ───────────────────────────────────
                if !has_selection {
                    let tick = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis();
                    // Keep cursor visible for 530ms after last movement, then blink
                    let recently_moved = tick.saturating_sub(cursor_moved_at) < 530;
                    let cursor_visible = recently_moved || (tick / 530) % 2 == 0;

                    if cursor_visible {
                        let cursor_x = self.measure_text_width(input_text, cursor_pos);
                        if let Ok(brush) = self.create_brush(rt, &self.theme.accent) {
                            let cursor_rect = D2D_RECT_F {
                                left: layout::PADDING_H + cursor_x,
                                top: (layout::SEARCH_BAR_HEIGHT - 22.0) / 2.0,
                                right: layout::PADDING_H + cursor_x + 1.5,
                                bottom: (layout::SEARCH_BAR_HEIGHT + 22.0) / 2.0,
                            };
                            rt.FillRectangle(&cursor_rect, &brush);
                        }
                    }
                }
            }

            // Draw separator
            if !items.is_empty() {
                if let Ok(brush) = self.create_brush(rt, &self.theme.separator) {
                    let sep_rect = D2D_RECT_F {
                        left: layout::PADDING_H,
                        top: layout::SEARCH_BAR_HEIGHT - 1.0,
                        right: layout::WINDOW_WIDTH - layout::PADDING_H,
                        bottom: layout::SEARCH_BAR_HEIGHT,
                    };
                    rt.FillRectangle(&sep_rect, &brush);
                }
            }

            if search_active && !input_text.is_empty() {
                self.draw_search_progress(rt, anim_progress);
            }

            // Draw result items with the viewport maintained by the window.
            let total_items = items.len();
            let max_visible = layout::MAX_VISIBLE_ITEMS;
            let scroll_offset = scroll_offset.min(total_items.saturating_sub(max_visible));
            let visible_start = scroll_offset;
            let visible_end = (scroll_offset + max_visible).min(total_items);
            let visible_count = visible_end - visible_start;

            let results_y_start =
                layout::SEARCH_BAR_HEIGHT + layout::SEPARATOR_HEIGHT + layout::RESULT_MARGIN_V;

            for vi in 0..visible_count {
                let item_index = visible_start + vi;
                let item = &items[item_index];
                let opacity = 1.0_f32; // Animation disabled

                let y = results_y_start + vi as f32 * layout::ITEM_HEIGHT;
                let is_selected = !input_focused && item_index == selected_index;

                // Selected item background with rounded corners
                // Flow.Launcher: ItemMargin="10 0 10 0", ItemRadius=5
                if is_selected {
                    if let Ok(brush) = self.create_brush(rt, &self.theme.selected_bg) {
                        let sel_rect = D2D1_ROUNDED_RECT {
                            rect: D2D_RECT_F {
                                left: layout::ITEM_MARGIN_H,
                                top: y,
                                right: layout::WINDOW_WIDTH - layout::ITEM_MARGIN_H,
                                bottom: y + layout::ITEM_HEIGHT,
                            },
                            radiusX: layout::ITEM_CORNER_RADIUS,
                            radiusY: layout::ITEM_CORNER_RADIUS,
                        };
                        rt.FillRoundedRectangle(&sel_rect, &brush);
                    }

                    // Left accent bullet indicator
                    // Flow.Launcher: ItemBulletSelectedStyle Width=4, Height=38, CornerRadius=2
                    if let Ok(brush) = self.create_brush(rt, &self.theme.accent) {
                        let bullet_top = y + (layout::ITEM_HEIGHT - layout::INDICATOR_HEIGHT) / 2.0;
                        let bullet_rect = D2D1_ROUNDED_RECT {
                            rect: D2D_RECT_F {
                                left: layout::ITEM_MARGIN_H,
                                top: bullet_top,
                                right: layout::ITEM_MARGIN_H + layout::INDICATOR_WIDTH,
                                bottom: bullet_top + layout::INDICATOR_HEIGHT,
                            },
                            radiusX: layout::INDICATOR_CORNER_RADIUS,
                            radiusY: layout::INDICATOR_CORNER_RADIUS,
                        };
                        rt.FillRoundedRectangle(&bullet_rect, &brush);
                    }
                }

                // ── Icon rendering ────────────────────────────────────────
                // Flow.Launcher: 32x32 icon in the 60px icon area
                if let Some(ref icon_path) = item.icon_path {
                    let icon_x = layout::ICON_LEFT + layout::ITEM_MARGIN_H;
                    let icon_y = y + (layout::ITEM_HEIGHT - layout::ICON_SIZE) / 2.0;
                    let icon_rect = D2D_RECT_F {
                        left: icon_x,
                        top: icon_y,
                        right: icon_x + layout::ICON_SIZE,
                        bottom: icon_y + layout::ICON_SIZE,
                    };
                    match icon_cache.get_icon_nonblocking(icon_path, item.is_directory, rt) {
                        super::icon::IconLookup::Ready(bitmap) => {
                            rt.DrawBitmap(
                                bitmap,
                                Some(&icon_rect),
                                opacity,
                                windows::Win32::Graphics::Direct2D::D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                                None,
                            );
                        }
                        super::icon::IconLookup::Loading => {
                            self.draw_loading_spinner(rt, icon_x, icon_y, anim_progress, opacity);
                        }
                        super::icon::IconLookup::Missing => {}
                    }
                }

                // Title — positioned in Flow.Launcher's column 1 area
                // Flow.Launcher: Grid Column="1" Margin="6 0 10 0", Title at row 0
                let title_x = layout::TEXT_LEFT;
                let title_y = y + 10.0; // Vertically align title to upper portion

                let title_rect = D2D_RECT_F {
                    left: title_x,
                    top: title_y,
                    right: layout::WINDOW_WIDTH - layout::ITEM_MARGIN_H - 60.0,
                    bottom: title_y + 22.0,
                };

                // Draw title with highlight ranges (matched characters in accent color)
                if !item.highlight.is_empty() {
                    self.draw_highlighted_text(
                        rt,
                        &item.title,
                        &item.highlight,
                        &title_rect,
                        &self.text_format_title,
                        &self.theme.text_primary,
                        &self.theme.accent,
                        opacity,
                    );
                } else if let Ok(brush) =
                    self.create_brush_alpha(rt, &self.theme.text_primary, opacity)
                {
                    let text_wide = wide_str(&item.title);
                    rt.DrawText(
                        &text_wide[..text_wide.len() - 1],
                        &self.text_format_title,
                        &title_rect,
                        &brush,
                        D2D1_DRAW_TEXT_OPTIONS_CLIP,
                        DWRITE_MEASURING_MODE_NATURAL,
                    );
                }

                // Subtitle — below title
                // Flow.Launcher: SubTitle at row 1, FontSize=13
                let subtitle_y = title_y + 22.0;
                let subtitle_rect = D2D_RECT_F {
                    left: title_x,
                    top: subtitle_y,
                    right: layout::WINDOW_WIDTH - layout::ITEM_MARGIN_H - 10.0,
                    bottom: subtitle_y + 18.0,
                };
                if !item.subtitle.is_empty() {
                    if let Ok(brush) =
                        self.create_brush_alpha(rt, &self.theme.text_secondary, opacity)
                    {
                        let text_wide = wide_str(&item.subtitle);
                        rt.DrawText(
                            &text_wide[..text_wide.len() - 1],
                            &self.text_format_subtitle,
                            &subtitle_rect,
                            &brush,
                            D2D1_DRAW_TEXT_OPTIONS_CLIP,
                            DWRITE_MEASURING_MODE_NATURAL,
                        );
                    }
                }

                // Shortcut hint (right-aligned, with badge background)
                // Flow.Launcher: ItemHotkeyBGStyle Margin="12 0 12 0", Padding="6 4 6 4"
                if !item.shortcut.is_empty() {
                    let hotkey_right = layout::WINDOW_WIDTH - layout::ITEM_MARGIN_H - 12.0;
                    let hotkey_rect = D2D_RECT_F {
                        left: hotkey_right - 44.0,
                        top: y + (layout::ITEM_HEIGHT - 20.0) / 2.0,
                        right: hotkey_right,
                        bottom: y + (layout::ITEM_HEIGHT + 20.0) / 2.0,
                    };

                    // Badge background
                    if let Ok(brush) = self.create_brush(rt, &self.theme.hotkey_bg) {
                        let badge_rect = D2D1_ROUNDED_RECT {
                            rect: hotkey_rect,
                            radiusX: 4.0,
                            radiusY: 4.0,
                        };
                        rt.FillRoundedRectangle(&badge_rect, &brush);
                    }

                    // Hotkey text
                    if let Ok(brush) = self.create_brush(rt, &self.theme.hotkey_text) {
                        let text_wide = wide_str(&item.shortcut);
                        rt.DrawText(
                            &text_wide[..text_wide.len() - 1],
                            &self.text_format_subtitle,
                            &hotkey_rect,
                            &brush,
                            D2D1_DRAW_TEXT_OPTIONS_NONE,
                            DWRITE_MEASURING_MODE_NATURAL,
                        );
                    }
                }
            }

            // ── Scroll indicator (right side thin bar) ──────────────────────
            if total_items > max_visible {
                let track_top =
                    layout::SEARCH_BAR_HEIGHT + layout::SEPARATOR_HEIGHT + layout::RESULT_MARGIN_V;
                let track_height = max_visible as f32 * layout::ITEM_HEIGHT;
                let track_right = layout::WINDOW_WIDTH - 3.0;
                let track_left = track_right - 3.0;

                // Thumb size and position
                let thumb_ratio = max_visible as f32 / total_items as f32;
                let thumb_height = (track_height * thumb_ratio).max(20.0);
                let scroll_ratio =
                    scroll_offset as f32 / total_items.saturating_sub(max_visible) as f32;
                let thumb_top = track_top + scroll_ratio * (track_height - thumb_height);

                // Draw track (very faint)
                if let Ok(brush) = self.create_brush_alpha(rt, &self.theme.text_secondary, 0.15) {
                    let track_rect = D2D_RECT_F {
                        left: track_left,
                        top: track_top,
                        right: track_right,
                        bottom: track_top + track_height,
                    };
                    rt.FillRectangle(&track_rect, &brush);
                }
                // Draw thumb
                if let Ok(brush) = self.create_brush_alpha(rt, &self.theme.text_secondary, 0.5) {
                    let thumb_rect = D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: track_left,
                            top: thumb_top,
                            right: track_right,
                            bottom: thumb_top + thumb_height,
                        },
                        radiusX: 1.5,
                        radiusY: 1.5,
                    };
                    rt.FillRoundedRectangle(&thumb_rect, &brush);
                }
            }

            // Draw preview panel inline (same BeginDraw/EndDraw frame)
            if let Some((preview_info, y_offset)) = preview {
                self.draw_preview_content(rt, preview_info, y_offset);
            }

            rt.EndDraw(None, None)
        }
    }

    pub fn finish_icon_load(
        &mut self,
        request: super::icon::IconLoadRequest,
        pixels: Option<super::icon::IconPixels>,
    ) {
        if let Some(device) = self.device.as_mut() {
            device
                .icon_cache
                .finish_load(request, pixels, &device.render_target);
        }
    }

    pub fn has_pending_icon_loads(&self) -> bool {
        self.device
            .as_ref()
            .is_some_and(|device| device.icon_cache.has_pending_loads())
    }

    pub fn take_icon_load_requests(&mut self) -> Vec<super::icon::IconLoadRequest> {
        self.device
            .as_mut()
            .map_or_else(Vec::new, |device| device.icon_cache.take_load_requests())
    }

    /// Draw text with highlighted (accent-colored) character ranges.
    /// Uses DWrite text layout with colored text ranges for precise rendering.
    fn draw_highlighted_text(
        &self,
        rt: &ID2D1HwndRenderTarget,
        text: &str,
        highlights: &[[u32; 2]],
        rect: &D2D_RECT_F,
        format: &IDWriteTextFormat,
        base_color: &Color,
        highlight_color: &Color,
        opacity: f32,
    ) {
        use windows::Win32::Graphics::Direct2D::ID2D1Brush;
        use windows::Win32::Graphics::DirectWrite::DWRITE_TEXT_RANGE;

        let wide: Vec<u16> = text.encode_utf16().collect();
        if wide.is_empty() {
            return;
        }

        let layout = unsafe {
            self.dwrite_factory.CreateTextLayout(
                &wide,
                format,
                rect.right - rect.left,
                rect.bottom - rect.top,
            )
        };
        let Ok(layout) = layout else {
            return;
        };

        // Set base color on entire text
        if let Ok(brush) = self.create_brush_alpha(rt, base_color, opacity) {
            let range = DWRITE_TEXT_RANGE {
                startPosition: 0,
                length: wide.len() as u32,
            };
            if let Ok(brush_ref) = brush.cast::<ID2D1Brush>() {
                unsafe {
                    let _ = layout.SetDrawingEffect(&brush_ref, range);
                }
            }
        }

        // Apply highlight color to matched ranges
        // Highlights are in byte offsets; convert to UTF-16 positions
        for &[start_byte, len_bytes] in highlights {
            let start_byte = start_byte as usize;
            let end_byte = (start_byte + len_bytes as usize).min(text.len());
            if start_byte >= text.len() {
                continue;
            }

            // Convert byte offset to UTF-16 position
            let utf16_start = text[..start_byte].encode_utf16().count() as u32;
            let utf16_len = text[start_byte..end_byte].encode_utf16().count() as u32;

            if utf16_len == 0 {
                continue;
            }

            if let Ok(brush) = self.create_brush_alpha(rt, highlight_color, opacity) {
                let range = DWRITE_TEXT_RANGE {
                    startPosition: utf16_start,
                    length: utf16_len,
                };
                if let Ok(brush_ref) = brush.cast::<ID2D1Brush>() {
                    unsafe {
                        let _ = layout.SetDrawingEffect(&brush_ref, range);
                    }
                }
            }
        }

        // Draw the layout
        if let Ok(brush) = self.create_brush_alpha(rt, base_color, opacity) {
            let origin = windows_numerics::Vector2 {
                X: rect.left,
                Y: rect.top,
            };
            unsafe {
                rt.DrawTextLayout(origin, &layout, &brush, D2D1_DRAW_TEXT_OPTIONS_CLIP);
            }
        }
    }

    fn create_brush(
        &self,
        rt: &ID2D1HwndRenderTarget,
        color: &Color,
    ) -> Result<ID2D1SolidColorBrush, String> {
        unsafe { rt.CreateSolidColorBrush(&color_to_d2d(color), None) }
            .map_err(|e| format!("CreateSolidColorBrush failed: {e}"))
    }

    /// Create a brush with modified opacity (for animation fade-in).
    fn create_brush_alpha(
        &self,
        rt: &ID2D1HwndRenderTarget,
        color: &Color,
        opacity: f32,
    ) -> Result<ID2D1SolidColorBrush, String> {
        let mut c = color_to_d2d(color);
        c.a *= opacity;
        unsafe { rt.CreateSolidColorBrush(&c, None) }
            .map_err(|e| format!("CreateSolidColorBrush failed: {e}"))
    }

    fn draw_search_progress(&self, rt: &ID2D1HwndRenderTarget, phase: f32) {
        let track_left = layout::PADDING_H;
        let track_right = layout::WINDOW_WIDTH - layout::PADDING_H;
        let track_width = track_right - track_left;
        let segment_width = track_width * 0.28;
        let travel = track_width + segment_width;
        let x = track_left - segment_width + travel * phase.fract();
        let y = layout::SEARCH_BAR_HEIGHT - 2.0;

        if let Ok(brush) = self.create_brush_alpha(rt, &self.theme.accent, 0.22) {
            unsafe {
                rt.FillRectangle(
                    &D2D_RECT_F {
                        left: track_left,
                        top: y,
                        right: track_right,
                        bottom: y + 2.0,
                    },
                    &brush,
                );
            }
        }

        if let Ok(brush) = self.create_brush_alpha(rt, &self.theme.accent, 0.85) {
            let left = x.max(track_left);
            let right = (x + segment_width).min(track_right);
            if right > left {
                unsafe {
                    rt.FillRectangle(
                        &D2D_RECT_F {
                            left,
                            top: y,
                            right,
                            bottom: y + 2.0,
                        },
                        &brush,
                    );
                }
            }
        }
    }

    fn draw_loading_spinner(
        &self,
        rt: &ID2D1HwndRenderTarget,
        x: f32,
        y: f32,
        phase: f32,
        opacity: f32,
    ) {
        let center_x = x + layout::ICON_SIZE / 2.0;
        let center_y = y + layout::ICON_SIZE / 2.0;
        let step = ((phase.fract() * 8.0) as usize) % 8;

        for i in 0..8 {
            let idx = (i + step) % 8;
            let alpha = (0.18 + idx as f32 * 0.08).min(0.82) * opacity;
            let (dx, dy) = match i {
                0 => (0.0, -10.0),
                1 => (7.0, -7.0),
                2 => (10.0, 0.0),
                3 => (7.0, 7.0),
                4 => (0.0, 10.0),
                5 => (-7.0, 7.0),
                6 => (-10.0, 0.0),
                _ => (-7.0, -7.0),
            };

            if let Ok(brush) = self.create_brush_alpha(rt, &self.theme.accent, alpha) {
                unsafe {
                    rt.FillRectangle(
                        &D2D_RECT_F {
                            left: center_x + dx - 2.0,
                            top: center_y + dy - 2.0,
                            right: center_x + dx + 2.0,
                            bottom: center_y + dy + 2.0,
                        },
                        &brush,
                    );
                }
            }
        }
    }

    /// Draw preview panel content (no BeginDraw/EndDraw).
    /// Can be called from within an existing BeginDraw/EndDraw pair.
    pub fn draw_preview_content(
        &self,
        rt: &ID2D1HwndRenderTarget,
        preview: &super::preview::PreviewInfo,
        y_offset: f32,
    ) {
        unsafe {
            // Preview panel background (slightly different shade)
            let panel_rect = D2D_RECT_F {
                left: 0.0,
                top: y_offset,
                right: layout::WINDOW_WIDTH,
                bottom: y_offset + layout::PREVIEW_PANEL_HEIGHT,
            };
            if let Ok(brush) = self.create_brush(rt, &self.theme.search_bg) {
                rt.FillRectangle(&panel_rect, &brush);
            }

            // Separator line
            if let Ok(brush) = self.create_brush(rt, &self.theme.separator) {
                let sep = D2D_RECT_F {
                    left: layout::PADDING_H,
                    top: y_offset,
                    right: layout::WINDOW_WIDTH - layout::PADDING_H,
                    bottom: y_offset + 1.0,
                };
                rt.FillRectangle(&sep, &brush);
            }

            let text_left = layout::PADDING_H + 8.0;
            let mut y = y_offset + 8.0;
            let line_height = 18.0;

            // Filename (bold)
            if let Ok(brush) = self.create_brush(rt, &self.theme.text_primary) {
                let text_wide = wide_str(&preview.filename);
                let rect = D2D_RECT_F {
                    left: text_left,
                    top: y,
                    right: layout::WINDOW_WIDTH - layout::PADDING_H,
                    bottom: y + 20.0,
                };
                rt.DrawText(
                    &text_wide[..text_wide.len() - 1],
                    &self.text_format_title,
                    &rect,
                    &brush,
                    D2D1_DRAW_TEXT_OPTIONS_CLIP,
                    DWRITE_MEASURING_MODE_NATURAL,
                );
            }
            y += 22.0;

            // File details in secondary text color
            if let Ok(brush) = self.create_brush(rt, &self.theme.text_secondary) {
                let lines = [
                    format!("Size: {}", preview.file_size),
                    format!("Modified: {}", preview.modified_at),
                    format!("Created: {}", preview.created_at),
                ];
                for line in &lines {
                    let text_wide = wide_str(line);
                    let rect = D2D_RECT_F {
                        left: text_left,
                        top: y,
                        right: layout::WINDOW_WIDTH - layout::PADDING_H,
                        bottom: y + line_height,
                    };
                    rt.DrawText(
                        &text_wide[..text_wide.len() - 1],
                        &self.text_format_subtitle,
                        &rect,
                        &brush,
                        D2D1_DRAW_TEXT_OPTIONS_CLIP,
                        DWRITE_MEASURING_MODE_NATURAL,
                    );
                    y += line_height;
                }
            }
        }
    }

    /// Measure the pixel width of text up to a byte offset (for cursor positioning).
    /// Public so that window.rs can use it for IME positioning.
    pub fn measure_text_width(&self, text: &str, byte_offset: usize) -> f32 {
        use windows::Win32::Graphics::DirectWrite::DWRITE_HIT_TEST_METRICS;

        let prefix = &text[..byte_offset.min(text.len())];
        if prefix.is_empty() {
            return 0.0;
        }

        let wide: Vec<u16> = prefix.encode_utf16().collect();
        let full_wide: Vec<u16> = text.encode_utf16().collect();

        // Create a text layout to measure
        let layout = unsafe {
            self.dwrite_factory.CreateTextLayout(
                &full_wide,
                &self.text_format_input,
                layout::WINDOW_WIDTH - layout::PADDING_H * 2.0,
                layout::SEARCH_BAR_HEIGHT,
            )
        };

        let Ok(layout) = layout else {
            // Fallback: rough estimate
            return wide.len() as f32 * 10.0;
        };

        // Use HitTestTextPosition to get the x coordinate at the cursor position
        let text_pos = wide.len() as u32;
        let mut x = 0.0f32;
        let mut y = 0.0f32;
        let mut metrics = DWRITE_HIT_TEST_METRICS::default();

        unsafe {
            let _ = layout.HitTestTextPosition(text_pos, false, &mut x, &mut y, &mut metrics);
        }

        x
    }
}

#[cfg(windows)]
fn color_to_d2d(c: &Color) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: c.r,
        g: c.g,
        b: c.b,
        a: c.a,
    }
}

/// Convert a Rust &str to a null-terminated wide string.
fn wide_str(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
