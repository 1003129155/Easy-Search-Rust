// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Direct2D renderer for the search window.
//! Supports selection highlight, cursor blinking, and index status display.

#[cfg(windows)]
use windows::Win32::Foundation::HWND;
#[cfg(windows)]
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT, D2D_RECT_F, D2D_SIZE_U,
};
#[cfg(windows)]
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateFactory, ID2D1Factory1, ID2D1HwndRenderTarget, ID2D1SolidColorBrush,
    D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_FACTORY_TYPE_SINGLE_THREADED,
    D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_PROPERTIES, D2D1_ROUNDED_RECT,
};
#[cfg(windows)]
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, DWRITE_FACTORY_TYPE_SHARED,
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_NORMAL,
    DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_MEASURING_MODE_NATURAL,
};
#[cfg(windows)]
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
#[cfg(windows)]
use windows::core::PCWSTR;
#[cfg(windows)]
use windows::core::Interface;
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
    pub icon: String,
    pub shortcut: String,
    pub action: easysearch_core::Action,
    /// File path for icon extraction (None for plugin-only results).
    pub icon_path: Option<String>,
    /// Whether this item represents a directory.
    pub is_directory: bool,
    /// Highlight ranges in the title as `[start_byte, len_bytes]`.
    pub highlight: Vec<[u32; 2]>,
}

/// Renderer state holding D2D and DWrite resources.
#[cfg(windows)]
pub struct Renderer {
    pub factory: ID2D1Factory1,
    pub render_target: Option<ID2D1HwndRenderTarget>,
    pub dwrite_factory: IDWriteFactory,
    pub text_format_title: IDWriteTextFormat,
    pub text_format_subtitle: IDWriteTextFormat,
    pub text_format_input: IDWriteTextFormat,
    pub theme: Theme,
}

#[cfg(windows)]
impl Renderer {
    /// Initialize renderer with D2D factory and DWrite.
    pub fn new() -> Result<Self, String> {
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

        Ok(Self {
            factory,
            render_target: None,
            dwrite_factory,
            text_format_title,
            text_format_subtitle,
            text_format_input,
            theme: Theme::system(),
        })
    }

    /// Create or recreate the render target for the given window.
    pub fn create_render_target(
        &mut self,
        hwnd: HWND,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        let render_props = D2D1_RENDER_TARGET_PROPERTIES {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            ..Default::default()
        };

        let hwnd_props = D2D1_HWND_RENDER_TARGET_PROPERTIES {
            hwnd,
            pixelSize: D2D_SIZE_U { width, height },
            ..Default::default()
        };

        let rt = unsafe { self.factory.CreateHwndRenderTarget(&render_props, &hwnd_props) }
            .map_err(|e| format!("CreateHwndRenderTarget failed: {e}"))?;

        self.render_target = Some(rt);
        Ok(())
    }

    /// Resize the render target.
    pub fn resize(&mut self, width: u32, height: u32) {
        if let Some(ref rt) = self.render_target {
            let size = D2D_SIZE_U { width, height };
            let _ = unsafe { rt.Resize(&size) };
        }
    }

    /// Render the full search UI.
    pub fn render(
        &self,
        input_text: &str,
        cursor_pos: usize,
        selection_range: (usize, usize),
        has_selection: bool,
        items: &[DisplayItem],
        selected_index: usize,
        placeholder: &str,
        icon_cache: &mut super::icon::IconCache,
        anim_progress: f32,
    ) {
        let Some(ref rt) = self.render_target else {
            return;
        };

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

                    let selection_color = Color { r: 0.26, g: 0.56, b: 0.96, a: 0.3 };
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
                    let cursor_visible = (tick / 530) % 2 == 0;

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

            // Draw result items with scroll offset
            // Calculate which items are visible based on selected_index
            let total_items = items.len();
            let max_visible = layout::MAX_VISIBLE_ITEMS;
            let scroll_offset = if selected_index >= max_visible {
                selected_index - max_visible + 1
            } else {
                0
            };
            let visible_start = scroll_offset;
            let visible_end = (scroll_offset + max_visible).min(total_items);
            let visible_count = visible_end - visible_start;

            let results_y_start = layout::SEARCH_BAR_HEIGHT + layout::SEPARATOR_HEIGHT + layout::RESULT_MARGIN_V;

            for vi in 0..visible_count {
                let item_index = visible_start + vi;
                let item = &items[item_index];
                let opacity = 1.0_f32; // Animation disabled

                let y = results_y_start + vi as f32 * layout::ITEM_HEIGHT;
                let is_selected = item_index == selected_index;

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
                    if let Some(bitmap) = icon_cache.get_icon(icon_path, item.is_directory, rt) {
                        let icon_x = layout::ICON_LEFT + layout::ITEM_MARGIN_H;
                        let icon_y = y + (layout::ITEM_HEIGHT - layout::ICON_SIZE) / 2.0;
                        let icon_rect = D2D_RECT_F {
                            left: icon_x,
                            top: icon_y,
                            right: icon_x + layout::ICON_SIZE,
                            bottom: icon_y + layout::ICON_SIZE,
                        };
                        rt.DrawBitmap(
                            bitmap,
                            Some(&icon_rect),
                            opacity,
                            windows::Win32::Graphics::Direct2D::D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                            None,
                        );
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
                } else if let Ok(brush) = self.create_brush_alpha(rt, &self.theme.text_primary, opacity) {
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
                    if let Ok(brush) = self.create_brush_alpha(rt, &self.theme.text_secondary, opacity) {
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
                let track_top = layout::SEARCH_BAR_HEIGHT + layout::SEPARATOR_HEIGHT + layout::RESULT_MARGIN_V;
                let track_height = max_visible as f32 * layout::ITEM_HEIGHT;
                let track_right = layout::WINDOW_WIDTH - 3.0;
                let track_left = track_right - 3.0;

                // Thumb size and position
                let thumb_ratio = max_visible as f32 / total_items as f32;
                let thumb_height = (track_height * thumb_ratio).max(20.0);
                let scroll_ratio = if total_items > 1 {
                    selected_index as f32 / (total_items - 1) as f32
                } else {
                    0.0
                };
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

            let _ = rt.EndDraw(None, None);
        }
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
        use windows::Win32::Graphics::DirectWrite::DWRITE_TEXT_RANGE;
        use windows::Win32::Graphics::Direct2D::ID2D1Brush;

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
        let Ok(layout) = layout else { return; };

        // Set base color on entire text
        if let Ok(brush) = self.create_brush_alpha(rt, base_color, opacity) {
            let range = DWRITE_TEXT_RANGE {
                startPosition: 0,
                length: wide.len() as u32,
            };
            unsafe {
                let brush_ref: &ID2D1Brush = &brush.cast().unwrap();
                let _ = layout.SetDrawingEffect(brush_ref, range);
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
                unsafe {
                    let brush_ref: &ID2D1Brush = &brush.cast().unwrap();
                    let _ = layout.SetDrawingEffect(brush_ref, range);
                }
            }
        }

        // Draw the layout
        unsafe {
            let origin = windows_numerics::Vector2 { X: rect.left, Y: rect.top };
            rt.DrawTextLayout(
                origin,
                &layout,
                &self.create_brush_alpha(rt, base_color, opacity).unwrap(),
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
            );
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

    /// Render preview info panel below the result list.
    /// Called separately after the main render when a preview is available.
    pub fn render_preview(&self, preview: &super::preview::PreviewInfo, y_offset: f32) {
        let Some(ref rt) = self.render_target else { return; };

        unsafe {
            rt.BeginDraw();

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

            let _ = rt.EndDraw(None, None);
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
