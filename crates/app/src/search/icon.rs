// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Icon extraction and D2D bitmap rendering.
//!
//! Search results can point at:
//! - file-system paths, which are resolved through the Windows shell
//! - embedded named icons backed by Flow.Launcher PNG assets
//!
//! Decoded pixel content is hashed before we create a D2D bitmap so identical
//! icons can reuse the same GPU resource across different cache keys.

#[cfg(windows)]
use std::collections::{HashMap, HashSet, VecDeque};

#[cfg(windows)]
use crate::shared::icon_assets;

#[cfg(windows)]
use windows::Win32::Foundation::SIZE;
#[cfg(windows)]
use windows::Win32::Graphics::Direct2D::{ID2D1Bitmap, ID2D1HwndRenderTarget};
#[cfg(windows)]
use windows::Win32::Graphics::Gdi::{DeleteObject, HBITMAP, HPALETTE};
#[cfg(windows)]
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppBGRA, GUID_WICPixelFormat32bppPBGRA,
    IWICBitmapSource, IWICFormatConverter, IWICImagingFactory, WICBitmapAlphaChannelOption,
    WICBitmapDitherTypeNone, WICBitmapPaletteTypeMedianCut, WICDecodeMetadataCacheOnLoad,
};
#[cfg(windows)]
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoUninitialize,
};
#[cfg(windows)]
use windows::Win32::UI::Shell::{
    IShellItem, IShellItemImageFactory, SHCreateItemFromParsingName, SHFILEINFOW, SHGFI_ICON,
    SHGFI_LARGEICON, SHGFI_USEFILEATTRIBUTES, SHGetFileInfoW, SIIGBF,
};
#[cfg(windows)]
use windows::core::{Interface as _, PCWSTR};

#[cfg(windows)]
const ICON_REQUEST_SIZE: i32 = 64;
#[cfg(windows)]
const MAX_PATH_CACHE: usize = 128;

#[cfg(windows)]
const BUILTIN_PROGRAM: &str = "builtin:program";
#[cfg(windows)]
const BUILTIN_FILE: &str = "builtin:file";
#[cfg(windows)]
const BUILTIN_FOLDER: &str = "builtin:folder";
#[cfg(windows)]
const BUILTIN_MISSING: &str = "builtin:missing";

#[cfg(windows)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IconCacheKey {
    Ext(String),
    Path(String),
}

#[cfg(windows)]
#[derive(Debug, Clone)]
pub struct IconLoadRequest {
    pub key: IconCacheKey,
    pub path: String,
    pub is_directory: bool,
}

#[cfg(windows)]
pub struct IconPixels {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

#[cfg(windows)]
pub enum IconLookup<'a> {
    Ready(&'a ID2D1Bitmap),
    Loading,
    Missing,
}

#[cfg(windows)]
pub struct IconCache {
    wic_factory: Option<IWICImagingFactory>,
    named_cache: HashMap<String, Option<ID2D1Bitmap>>,
    ext_cache: HashMap<String, Option<ID2D1Bitmap>>,
    path_cache: HashMap<String, Option<ID2D1Bitmap>>,
    path_lru: VecDeque<String>,
    hash_cache: HashMap<u64, ID2D1Bitmap>,
    pending_loads: HashSet<IconCacheKey>,
    load_requests: VecDeque<IconLoadRequest>,
}

#[cfg(windows)]
impl IconCache {
    pub fn new() -> Self {
        let wic_factory = unsafe {
            CoCreateInstance::<_, IWICImagingFactory>(
                &CLSID_WICImagingFactory,
                None,
                CLSCTX_INPROC_SERVER,
            )
        }
        .ok();

        Self {
            wic_factory,
            named_cache: HashMap::with_capacity(24),
            ext_cache: HashMap::with_capacity(64),
            path_cache: HashMap::with_capacity(MAX_PATH_CACHE),
            path_lru: VecDeque::with_capacity(MAX_PATH_CACHE),
            hash_cache: HashMap::with_capacity(128),
            pending_loads: HashSet::new(),
            load_requests: VecDeque::new(),
        }
    }

    #[allow(dead_code)]
    pub fn get_icon(
        &mut self,
        icon_ref: &str,
        is_directory: bool,
        rt: &ID2D1HwndRenderTarget,
    ) -> Option<&ID2D1Bitmap> {
        if icon_assets::is_named_icon(icon_ref) {
            return self.get_from_named_cache(icon_ref, rt);
        }

        let ext = extract_extension(icon_ref);
        let use_path_cache = !is_directory && is_unique_icon_ext(&ext);

        if use_path_cache {
            self.get_from_path_cache(icon_ref, rt)
        } else {
            let cache_key = if is_directory {
                "::dir".to_string()
            } else if ext.is_empty() {
                "::noext".to_string()
            } else {
                ext
            };
            self.get_from_ext_cache(&cache_key, icon_ref, is_directory, rt)
        }
    }

    pub fn get_icon_nonblocking(
        &mut self,
        icon_ref: &str,
        is_directory: bool,
        rt: &ID2D1HwndRenderTarget,
    ) -> IconLookup<'_> {
        if icon_assets::is_named_icon(icon_ref) {
            return match self.get_from_named_cache(icon_ref, rt) {
                Some(bitmap) => IconLookup::Ready(bitmap),
                None => IconLookup::Missing,
            };
        }

        let ext = extract_extension(icon_ref);
        let use_path_cache = !is_directory && is_unique_icon_ext(&ext);
        let key = if use_path_cache {
            IconCacheKey::Path(icon_ref.to_string())
        } else {
            IconCacheKey::Ext(if is_directory {
                "::dir".to_string()
            } else if ext.is_empty() {
                "::noext".to_string()
            } else {
                ext
            })
        };

        match &key {
            IconCacheKey::Path(path) if self.path_cache.contains_key(path) => {
                self.touch_lru(path);
                if self
                    .path_cache
                    .get(path)
                    .and_then(|bitmap| bitmap.as_ref())
                    .is_some()
                {
                    return IconLookup::Ready(
                        self.path_cache
                            .get(path)
                            .and_then(|bitmap| bitmap.as_ref())
                            .expect("checked cached path bitmap"),
                    );
                }
                return match self.builtin_or_missing(BUILTIN_MISSING, rt) {
                    Some(bitmap) => IconLookup::Ready(bitmap),
                    None => IconLookup::Missing,
                };
            }
            IconCacheKey::Ext(ext_key) if self.ext_cache.contains_key(ext_key) => {
                if self
                    .ext_cache
                    .get(ext_key)
                    .and_then(|bitmap| bitmap.as_ref())
                    .is_some()
                {
                    return IconLookup::Ready(
                        self.ext_cache
                            .get(ext_key)
                            .and_then(|bitmap| bitmap.as_ref())
                            .expect("checked cached extension bitmap"),
                    );
                }
                return match self.builtin_or_missing(BUILTIN_MISSING, rt) {
                    Some(bitmap) => IconLookup::Ready(bitmap),
                    None => IconLookup::Missing,
                };
            }
            _ => {}
        }

        if self.pending_loads.insert(key.clone()) {
            self.load_requests.push_back(IconLoadRequest {
                key,
                path: icon_ref.to_string(),
                is_directory,
            });
        }

        IconLookup::Loading
    }

    pub fn take_load_requests(&mut self) -> Vec<IconLoadRequest> {
        self.load_requests.drain(..).collect()
    }

    pub fn has_pending_loads(&self) -> bool {
        !self.pending_loads.is_empty()
    }

    pub fn finish_load(
        &mut self,
        request: IconLoadRequest,
        pixels: Option<IconPixels>,
        rt: &ID2D1HwndRenderTarget,
    ) {
        self.pending_loads.remove(&request.key);
        let bitmap = pixels.and_then(|pixels| self.bitmap_from_pixels(&pixels, rt));

        match request.key {
            IconCacheKey::Path(path) => {
                if self.path_cache.len() >= MAX_PATH_CACHE && !self.path_cache.contains_key(&path) {
                    self.evict_oldest();
                }
                self.path_cache.insert(path.clone(), bitmap);
                self.path_lru.push_back(path);
            }
            IconCacheKey::Ext(key) => {
                self.ext_cache.insert(key, bitmap);
            }
        }
    }

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.named_cache.clear();
        self.ext_cache.clear();
        self.path_cache.clear();
        self.path_lru.clear();
        self.hash_cache.clear();
        self.pending_loads.clear();
        self.load_requests.clear();
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.named_cache.len() + self.ext_cache.len() + self.path_cache.len()
    }

    fn get_from_named_cache(
        &mut self,
        key: &str,
        rt: &ID2D1HwndRenderTarget,
    ) -> Option<&ID2D1Bitmap> {
        if !self.named_cache.contains_key(key) {
            let bitmap = self.load_named_icon_fallible(key, rt);
            self.named_cache.insert(key.to_string(), bitmap);
        }

        if self
            .named_cache
            .get(key)
            .and_then(|bitmap| bitmap.as_ref())
            .is_some()
        {
            return self.named_cache.get(key).and_then(|bitmap| bitmap.as_ref());
        }

        self.builtin_or_missing(BUILTIN_MISSING, rt)
    }

    #[allow(dead_code)]
    fn get_from_ext_cache(
        &mut self,
        key: &str,
        path: &str,
        is_directory: bool,
        rt: &ID2D1HwndRenderTarget,
    ) -> Option<&ID2D1Bitmap> {
        if !self.ext_cache.contains_key(key) {
            let bitmap = self.load_path_icon_fallible(path, is_directory, rt);
            self.ext_cache.insert(key.to_string(), bitmap);
        }

        if self
            .ext_cache
            .get(key)
            .and_then(|bitmap| bitmap.as_ref())
            .is_some()
        {
            return self.ext_cache.get(key).and_then(|bitmap| bitmap.as_ref());
        }

        self.builtin_or_missing(BUILTIN_MISSING, rt)
    }

    #[allow(dead_code)]
    fn get_from_path_cache(
        &mut self,
        path: &str,
        rt: &ID2D1HwndRenderTarget,
    ) -> Option<&ID2D1Bitmap> {
        if self.path_cache.contains_key(path) {
            self.touch_lru(path);
        } else {
            if self.path_cache.len() >= MAX_PATH_CACHE {
                self.evict_oldest();
            }
            let bitmap = self.load_path_icon_fallible(path, false, rt);
            self.path_cache.insert(path.to_string(), bitmap);
            self.path_lru.push_back(path.to_string());
        }

        if self
            .path_cache
            .get(path)
            .and_then(|bitmap| bitmap.as_ref())
            .is_some()
        {
            return self.path_cache.get(path).and_then(|bitmap| bitmap.as_ref());
        }

        self.builtin_or_missing(BUILTIN_MISSING, rt)
    }

    fn touch_lru(&mut self, key: &str) {
        if let Some(pos) = self.path_lru.iter().position(|entry| entry == key) {
            self.path_lru.remove(pos);
            self.path_lru.push_back(key.to_string());
        }
    }

    fn evict_oldest(&mut self) {
        if let Some(oldest_key) = self.path_lru.pop_front() {
            self.path_cache.remove(&oldest_key);
        }
    }

    fn load_named_icon_fallible(
        &mut self,
        key: &str,
        rt: &ID2D1HwndRenderTarget,
    ) -> Option<ID2D1Bitmap> {
        let bytes = icon_assets::named_icon_bytes(key)?;
        let wic = self.wic_factory.clone()?;

        unsafe {
            let stream = wic.CreateStream().ok()?;
            stream.InitializeFromMemory(bytes).ok()?;
            let decoder = wic
                .CreateDecoderFromStream(&stream, std::ptr::null(), WICDecodeMetadataCacheOnLoad)
                .ok()?;
            let frame = decoder.GetFrame(0).ok()?;
            let source: IWICBitmapSource = frame.cast().ok()?;
            self.bitmap_from_wic_source(&source, rt, &wic)
        }
    }

    #[allow(dead_code)]
    fn load_path_icon_fallible(
        &mut self,
        path: &str,
        is_directory: bool,
        rt: &ID2D1HwndRenderTarget,
    ) -> Option<ID2D1Bitmap> {
        let wic = self.wic_factory.clone()?;

        if let Some(bitmap) = self.try_shell_image_factory(path, is_directory, rt, &wic) {
            return Some(bitmap);
        }

        if let Some(bitmap) = self.try_sh_get_file_info(path, rt, &wic) {
            return Some(bitmap);
        }

        None
    }

    #[allow(dead_code)]
    fn try_shell_image_factory(
        &mut self,
        path: &str,
        is_directory: bool,
        rt: &ID2D1HwndRenderTarget,
        wic: &IWICImagingFactory,
    ) -> Option<ID2D1Bitmap> {
        unsafe {
            let path_wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
            let shell_item: IShellItem =
                SHCreateItemFromParsingName(PCWSTR(path_wide.as_ptr()), None).ok()?;
            let image_factory: IShellItemImageFactory = shell_item.cast().ok()?;

            let size = SIZE {
                cx: ICON_REQUEST_SIZE,
                cy: ICON_REQUEST_SIZE,
            };

            const SIIGBF_ICONONLY_VAL: i32 = 0x00000004;

            let flags = if is_directory {
                SIIGBF(SIIGBF_ICONONLY_VAL)
            } else {
                SIIGBF(0)
            };

            let hbitmap = match image_factory.GetImage(size, flags) {
                Ok(bitmap) => bitmap,
                Err(_) if !is_directory => image_factory
                    .GetImage(size, SIIGBF(SIIGBF_ICONONLY_VAL))
                    .ok()?,
                Err(_) => return None,
            };

            let bitmap = self.hbitmap_to_d2d_bitmap(hbitmap, rt, wic);
            let _ = DeleteObject(hbitmap.into());
            bitmap
        }
    }

    #[allow(dead_code)]
    fn try_sh_get_file_info(
        &mut self,
        path: &str,
        rt: &ID2D1HwndRenderTarget,
        wic: &IWICImagingFactory,
    ) -> Option<ID2D1Bitmap> {
        unsafe {
            let path_wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
            let mut shfi = SHFILEINFOW::default();

            let result = SHGetFileInfoW(
                PCWSTR(path_wide.as_ptr()),
                windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL,
                Some(&mut shfi),
                std::mem::size_of::<SHFILEINFOW>() as u32,
                SHGFI_ICON | SHGFI_LARGEICON | SHGFI_USEFILEATTRIBUTES,
            );

            if result == 0 || shfi.hIcon.is_invalid() {
                return None;
            }

            let bitmap = self.hicon_to_d2d_bitmap(shfi.hIcon, rt, wic);
            let _ = windows::Win32::UI::WindowsAndMessaging::DestroyIcon(shfi.hIcon);
            bitmap
        }
    }

    fn builtin_or_missing(
        &mut self,
        key: &str,
        rt: &ID2D1HwndRenderTarget,
    ) -> Option<&ID2D1Bitmap> {
        if !self.named_cache.contains_key(key) {
            let bitmap = self
                .load_named_icon_fallible(key, rt)
                .or_else(|| self.create_builtin_bitmap(key, rt));
            self.named_cache.insert(key.to_string(), bitmap);
        }

        if self
            .named_cache
            .get(key)
            .and_then(|bitmap| bitmap.as_ref())
            .is_some()
        {
            return self.named_cache.get(key).and_then(|bitmap| bitmap.as_ref());
        }

        self.named_cache.remove(BUILTIN_MISSING);
        let bitmap = self.create_builtin_bitmap(BUILTIN_MISSING, rt);
        self.named_cache.insert(BUILTIN_MISSING.to_string(), bitmap);
        self.named_cache
            .get(BUILTIN_MISSING)
            .and_then(|bitmap| bitmap.as_ref())
    }

    fn create_builtin_bitmap(&self, key: &str, rt: &ID2D1HwndRenderTarget) -> Option<ID2D1Bitmap> {
        let wic = self.wic_factory.as_ref()?;
        let (r, g, b): (u8, u8, u8) = match key {
            BUILTIN_PROGRAM => (0x00, 0x78, 0xD4),
            BUILTIN_FILE => (0x88, 0x88, 0x88),
            BUILTIN_FOLDER => (0xDC, 0xB4, 0x00),
            _ => (0xD4, 0x40, 0x40),
        };

        let width: u32 = 32;
        let height: u32 = 32;
        let stride = width * 4;
        let mut pixels = vec![0u8; (stride * height) as usize];

        for y in 0..height {
            for x in 0..width {
                let idx = (y * stride + x * 4) as usize;
                let is_border = x < 2 || x >= width - 2 || y < 2 || y >= height - 2;
                if is_border {
                    pixels[idx] = b.saturating_sub(40);
                    pixels[idx + 1] = g.saturating_sub(40);
                    pixels[idx + 2] = r.saturating_sub(40);
                } else {
                    pixels[idx] = b;
                    pixels[idx + 1] = g;
                    pixels[idx + 2] = r;
                }
                pixels[idx + 3] = 255;
            }
        }

        unsafe {
            let wic_bitmap = wic
                .CreateBitmapFromMemory(
                    width,
                    height,
                    &GUID_WICPixelFormat32bppBGRA,
                    stride,
                    &pixels,
                )
                .ok()?;

            let converter: IWICFormatConverter = wic.CreateFormatConverter().ok()?;
            converter
                .Initialize(
                    &wic_bitmap,
                    &GUID_WICPixelFormat32bppPBGRA,
                    WICBitmapDitherTypeNone,
                    None,
                    0.0,
                    WICBitmapPaletteTypeMedianCut,
                )
                .ok()?;

            rt.CreateBitmapFromWicBitmap(&converter, None).ok()
        }
    }

    fn bitmap_from_pixels(
        &mut self,
        pixels: &IconPixels,
        rt: &ID2D1HwndRenderTarget,
    ) -> Option<ID2D1Bitmap> {
        let wic = self.wic_factory.as_ref()?;
        let stride = pixels.width.saturating_mul(4);
        if stride == 0 || pixels.height == 0 || pixels.pixels.is_empty() {
            return None;
        }

        let hash = hash_icon_pixels(pixels.width, pixels.height, &pixels.pixels);
        if let Some(bitmap) = self.hash_cache.get(&hash) {
            return Some(bitmap.clone());
        }

        unsafe {
            let wic_bitmap = wic
                .CreateBitmapFromMemory(
                    pixels.width,
                    pixels.height,
                    &GUID_WICPixelFormat32bppPBGRA,
                    stride,
                    &pixels.pixels,
                )
                .ok()?;
            let bitmap = rt.CreateBitmapFromWicBitmap(&wic_bitmap, None).ok()?;
            self.hash_cache.insert(hash, bitmap.clone());
            Some(bitmap)
        }
    }

    fn bitmap_from_wic_source(
        &mut self,
        source: &IWICBitmapSource,
        rt: &ID2D1HwndRenderTarget,
        wic: &IWICImagingFactory,
    ) -> Option<ID2D1Bitmap> {
        unsafe {
            let converter: IWICFormatConverter = wic.CreateFormatConverter().ok()?;
            converter
                .Initialize(
                    source,
                    &GUID_WICPixelFormat32bppPBGRA,
                    WICBitmapDitherTypeNone,
                    None,
                    0.0,
                    WICBitmapPaletteTypeMedianCut,
                )
                .ok()?;

            let mut width = 0u32;
            let mut height = 0u32;
            converter.GetSize(&mut width, &mut height).ok()?;
            if width == 0 || height == 0 {
                return None;
            }

            let stride = width.saturating_mul(4);
            let mut pixels = vec![0u8; (stride as usize).saturating_mul(height as usize)];
            converter
                .CopyPixels(std::ptr::null(), stride, pixels.as_mut_slice())
                .ok()?;

            let hash = hash_icon_pixels(width, height, &pixels);
            if let Some(bitmap) = self.hash_cache.get(&hash) {
                return Some(bitmap.clone());
            }

            let bitmap = rt.CreateBitmapFromWicBitmap(&converter, None).ok()?;
            self.hash_cache.insert(hash, bitmap.clone());
            Some(bitmap)
        }
    }

    #[allow(dead_code)]
    fn hbitmap_to_d2d_bitmap(
        &mut self,
        hbitmap: HBITMAP,
        rt: &ID2D1HwndRenderTarget,
        wic: &IWICImagingFactory,
    ) -> Option<ID2D1Bitmap> {
        unsafe {
            let wic_bitmap = wic
                .CreateBitmapFromHBITMAP(
                    hbitmap,
                    HPALETTE::default(),
                    WICBitmapAlphaChannelOption::default(),
                )
                .ok()?;
            let source: IWICBitmapSource = wic_bitmap.cast().ok()?;
            self.bitmap_from_wic_source(&source, rt, wic)
        }
    }

    #[allow(dead_code)]
    fn hicon_to_d2d_bitmap(
        &mut self,
        hicon: windows::Win32::UI::WindowsAndMessaging::HICON,
        rt: &ID2D1HwndRenderTarget,
        wic: &IWICImagingFactory,
    ) -> Option<ID2D1Bitmap> {
        unsafe {
            let wic_bitmap = wic.CreateBitmapFromHICON(hicon).ok()?;
            let source: IWICBitmapSource = wic_bitmap.cast().ok()?;
            self.bitmap_from_wic_source(&source, rt, wic)
        }
    }
}

#[cfg(windows)]
fn hash_icon_pixels(width: u32, height: u32, pixels: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;

    let mut hash = OFFSET;
    for byte in width
        .to_le_bytes()
        .into_iter()
        .chain(height.to_le_bytes())
        .chain(pixels.iter().copied())
    {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

#[cfg(windows)]
fn is_unique_icon_ext(ext: &str) -> bool {
    matches!(
        ext.to_ascii_lowercase().as_str(),
        ".exe"
            | ".lnk"
            | ".ico"
            | ".url"
            | ".png"
            | ".jpg"
            | ".jpeg"
            | ".webp"
            | ".bmp"
            | ".gif"
            | ".svg"
    )
}

#[cfg(windows)]
fn extract_extension(path: &str) -> String {
    let filename_start = path.rfind(['\\', '/']).map_or(0, |index| index + 1);
    let filename = &path[filename_start..];

    match filename.rfind('.') {
        Some(dot_pos) if dot_pos > 0 => filename[dot_pos..].to_ascii_lowercase(),
        _ => String::new(),
    }
}

#[cfg(windows)]
impl Default for IconCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(windows)]
impl Drop for IconCache {
    fn drop(&mut self) {
        self.clear();
    }
}

#[cfg(windows)]
pub fn load_icon_pixels(request: &IconLoadRequest) -> Option<IconPixels> {
    let com_initialized = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok() };
    let result = load_icon_pixels_inner(&request.path, request.is_directory);
    if com_initialized {
        unsafe { CoUninitialize() };
    }
    result
}

#[cfg(windows)]
fn load_icon_pixels_inner(path: &str, is_directory: bool) -> Option<IconPixels> {
    let wic = unsafe {
        CoCreateInstance::<_, IWICImagingFactory>(
            &CLSID_WICImagingFactory,
            None,
            CLSCTX_INPROC_SERVER,
        )
    }
    .ok()?;

    try_shell_image_factory_pixels(path, is_directory, &wic)
        .or_else(|| try_sh_get_file_info_pixels(path, &wic))
}

#[cfg(windows)]
fn try_shell_image_factory_pixels(
    path: &str,
    is_directory: bool,
    wic: &IWICImagingFactory,
) -> Option<IconPixels> {
    unsafe {
        let path_wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        let shell_item: IShellItem =
            SHCreateItemFromParsingName(PCWSTR(path_wide.as_ptr()), None).ok()?;
        let image_factory: IShellItemImageFactory = shell_item.cast().ok()?;

        let size = SIZE {
            cx: ICON_REQUEST_SIZE,
            cy: ICON_REQUEST_SIZE,
        };

        const SIIGBF_ICONONLY_VAL: i32 = 0x00000004;
        let flags = if is_directory {
            SIIGBF(SIIGBF_ICONONLY_VAL)
        } else {
            SIIGBF(0)
        };

        let hbitmap = match image_factory.GetImage(size, flags) {
            Ok(bitmap) => bitmap,
            Err(_) if !is_directory => image_factory
                .GetImage(size, SIIGBF(SIIGBF_ICONONLY_VAL))
                .ok()?,
            Err(_) => return None,
        };

        let pixels = hbitmap_to_pixels(hbitmap, wic);
        let _ = DeleteObject(hbitmap.into());
        pixels
    }
}

#[cfg(windows)]
fn try_sh_get_file_info_pixels(path: &str, wic: &IWICImagingFactory) -> Option<IconPixels> {
    unsafe {
        let path_wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        let mut shfi = SHFILEINFOW::default();

        let result = SHGetFileInfoW(
            PCWSTR(path_wide.as_ptr()),
            windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL,
            Some(&mut shfi),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON | SHGFI_USEFILEATTRIBUTES,
        );

        if result == 0 || shfi.hIcon.is_invalid() {
            return None;
        }

        let pixels = hicon_to_pixels(shfi.hIcon, wic);
        let _ = windows::Win32::UI::WindowsAndMessaging::DestroyIcon(shfi.hIcon);
        pixels
    }
}

#[cfg(windows)]
fn hbitmap_to_pixels(hbitmap: HBITMAP, wic: &IWICImagingFactory) -> Option<IconPixels> {
    unsafe {
        let wic_bitmap = wic
            .CreateBitmapFromHBITMAP(
                hbitmap,
                HPALETTE::default(),
                WICBitmapAlphaChannelOption::default(),
            )
            .ok()?;
        let source: IWICBitmapSource = wic_bitmap.cast().ok()?;
        wic_source_to_pixels(&source, wic)
    }
}

#[cfg(windows)]
fn hicon_to_pixels(
    hicon: windows::Win32::UI::WindowsAndMessaging::HICON,
    wic: &IWICImagingFactory,
) -> Option<IconPixels> {
    unsafe {
        let wic_bitmap = wic.CreateBitmapFromHICON(hicon).ok()?;
        let source: IWICBitmapSource = wic_bitmap.cast().ok()?;
        wic_source_to_pixels(&source, wic)
    }
}

#[cfg(windows)]
fn wic_source_to_pixels(source: &IWICBitmapSource, wic: &IWICImagingFactory) -> Option<IconPixels> {
    unsafe {
        let converter: IWICFormatConverter = wic.CreateFormatConverter().ok()?;
        converter
            .Initialize(
                source,
                &GUID_WICPixelFormat32bppPBGRA,
                WICBitmapDitherTypeNone,
                None,
                0.0,
                WICBitmapPaletteTypeMedianCut,
            )
            .ok()?;

        let mut width = 0u32;
        let mut height = 0u32;
        converter.GetSize(&mut width, &mut height).ok()?;
        if width == 0 || height == 0 {
            return None;
        }

        let stride = width.saturating_mul(4);
        let mut pixels = vec![0u8; (stride as usize).saturating_mul(height as usize)];
        converter
            .CopyPixels(std::ptr::null(), stride, pixels.as_mut_slice())
            .ok()?;

        Some(IconPixels {
            width,
            height,
            pixels,
        })
    }
}
