// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Icon extraction and D2D bitmap rendering.
//!
//! ## Memory Management Strategy
//!
//! Icons are cached in a two-tier system:
//!
//! **Tier 1 — Extension cache** (persistent, small footprint):
//! - Standard file types (`.txt`, `.pdf`, `.doc`, etc.) share the same icon
//!   per extension. These are cached indefinitely since there are only ~50-100
//!   unique extensions a user encounters.
//! - Directories use a single shared "folder" icon.
//!
//! **Tier 2 — Path cache** (LRU eviction, bounded size):
//! - `.exe` and `.lnk` files have unique icons per file, so they're cached by
//!   full path. This tier is bounded to MAX_PATH_CACHE entries with LRU eviction
//!   to prevent unbounded memory growth.
//!
//! ## COM Object Lifetime
//!
//! - `IWICImagingFactory`: Created once, held for the lifetime of `IconCache`.
//!   Released automatically by `windows-rs` Drop impl when IconCache is dropped.
//! - `ID2D1Bitmap`: Each cached bitmap is a GPU-side resource. Released via COM
//!   Release() when evicted from cache or when IconCache is dropped.
//! - `HICON`: Temporary handle from `SHGetFileInfoW`. Always destroyed via
//!   `DestroyIcon()` immediately after conversion to D2D bitmap, regardless
//!   of conversion success/failure.
//!
//! ## Approximate Memory Usage
//!
//! - Extension cache: ~50 entries × 32×32×4 bytes ≈ 200 KB (GPU-side)
//! - Path cache: 128 entries × 32×32×4 bytes ≈ 512 KB (GPU-side)
//! - Total icon memory: < 1 MB
//!
//! Reference: Flow.Launcher uses WPF's BitmapSource caching with weak references
//! and periodic GC. Our approach is simpler but bounded.

#[cfg(windows)]
use std::collections::{HashMap, VecDeque};

#[cfg(windows)]
use windows::Win32::Graphics::Direct2D::{ID2D1Bitmap, ID2D1HwndRenderTarget};
#[cfg(windows)]
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, IWICImagingFactory, WICBitmapDitherTypeNone,
    WICBitmapPaletteTypeMedianCut,
};
#[cfg(windows)]
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
#[cfg(windows)]
use windows::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_SMALLICON};

/// Maximum number of per-path icon entries (for .exe/.lnk).
/// Each entry is ~4KB of GPU memory (32x32 BGRA), so 128 entries ≈ 512KB.
#[cfg(windows)]
const MAX_PATH_CACHE: usize = 128;

/// Icon cache with two-tier caching strategy.
///
/// Tier 1: Extension-based (permanent, shared icons for same file types)
/// Tier 2: Path-based with LRU eviction (unique icons for .exe/.lnk)
#[cfg(windows)]
pub struct IconCache {
    /// WIC factory for HICON → D2D bitmap conversion. Held for entire lifetime.
    wic_factory: Option<IWICImagingFactory>,

    /// Tier 1: Extension → bitmap (e.g. ".txt" → text file icon).
    /// Never evicted since the set of extensions is naturally bounded.
    ext_cache: HashMap<String, Option<ID2D1Bitmap>>,

    /// Tier 2: Full path → bitmap (for .exe and .lnk with unique icons).
    /// Bounded to MAX_PATH_CACHE entries with LRU eviction.
    path_cache: HashMap<String, Option<ID2D1Bitmap>>,

    /// LRU order for path_cache. Front = oldest (evict first).
    path_lru: VecDeque<String>,
}

#[cfg(windows)]
impl IconCache {
    /// Create a new icon cache.
    ///
    /// Initializes the WIC factory via COM. If COM initialization fails,
    /// the cache will still work but `get_icon` will always return None.
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
            ext_cache: HashMap::with_capacity(64),
            path_cache: HashMap::with_capacity(MAX_PATH_CACHE),
            path_lru: VecDeque::with_capacity(MAX_PATH_CACHE),
        }
    }

    /// Get or load an icon bitmap for a file path.
    ///
    /// Returns a reference to the cached D2D bitmap, or None if the icon
    /// could not be loaded (e.g. file doesn't exist, WIC failure).
    ///
    /// Caching rules:
    /// - `.exe` / `.lnk` → cached by full path (unique icon per program)
    /// - directories → cached as "::dir" (all folders share one icon)
    /// - everything else → cached by extension (e.g. ".pdf" → PDF icon)
    pub fn get_icon(
        &mut self,
        path: &str,
        is_directory: bool,
        rt: &ID2D1HwndRenderTarget,
    ) -> Option<&ID2D1Bitmap> {
        let ext = extract_extension(path);
        let use_path_cache = !is_directory && is_unique_icon_ext(&ext);

        if use_path_cache {
            // Tier 2: per-path caching for .exe/.lnk
            self.get_from_path_cache(path, rt)
        } else {
            // Tier 1: per-extension caching
            let cache_key = if is_directory {
                "::dir".to_string()
            } else if ext.is_empty() {
                "::noext".to_string()
            } else {
                ext
            };
            self.get_from_ext_cache(&cache_key, path, rt)
        }
    }

    /// Look up in extension cache (Tier 1). Load on miss.
    fn get_from_ext_cache(
        &mut self,
        key: &str,
        path: &str,
        rt: &ID2D1HwndRenderTarget,
    ) -> Option<&ID2D1Bitmap> {
        if !self.ext_cache.contains_key(key) {
            let bitmap = self.load_icon(path, rt);
            self.ext_cache.insert(key.to_string(), bitmap);
        }
        self.ext_cache.get(key).and_then(|opt| opt.as_ref())
    }

    /// Look up in path cache (Tier 2) with LRU eviction. Load on miss.
    fn get_from_path_cache(
        &mut self,
        path: &str,
        rt: &ID2D1HwndRenderTarget,
    ) -> Option<&ID2D1Bitmap> {
        if self.path_cache.contains_key(path) {
            // Move to back of LRU (most recently used)
            self.touch_lru(path);
        } else {
            // Evict oldest if at capacity
            if self.path_cache.len() >= MAX_PATH_CACHE {
                self.evict_oldest();
            }
            let bitmap = self.load_icon(path, rt);
            self.path_cache.insert(path.to_string(), bitmap);
            self.path_lru.push_back(path.to_string());
        }
        self.path_cache.get(path).and_then(|opt| opt.as_ref())
    }

    /// Move a key to the back of LRU (mark as recently used).
    fn touch_lru(&mut self, key: &str) {
        if let Some(pos) = self.path_lru.iter().position(|k| k == key) {
            self.path_lru.remove(pos);
            self.path_lru.push_back(key.to_string());
        }
    }

    /// Evict the least recently used entry from path cache.
    /// The evicted `ID2D1Bitmap` COM object is dropped here, triggering Release().
    fn evict_oldest(&mut self) {
        if let Some(oldest_key) = self.path_lru.pop_front() {
            // Dropping the Option<ID2D1Bitmap> triggers COM Release()
            self.path_cache.remove(&oldest_key);
        }
    }

    /// Clear all cached icons. Useful when render target is recreated.
    /// All ID2D1Bitmap COM objects are released via Drop.
    pub fn clear(&mut self) {
        self.ext_cache.clear();
        self.path_cache.clear();
        self.path_lru.clear();
    }

    /// Current total number of cached icons (both tiers).
    pub fn len(&self) -> usize {
        self.ext_cache.len() + self.path_cache.len()
    }

    /// Load an icon from a file path using SHGetFileInfoW.
    ///
    /// Memory flow:
    /// 1. SHGetFileInfoW → HICON (GDI handle, kernel-managed)
    /// 2. WIC CreateBitmapFromHICON → IWICBitmap (system memory)
    /// 3. WIC FormatConverter → 32bpp PBGRA (system memory, temporary)
    /// 4. D2D CreateBitmapFromWicBitmap → ID2D1Bitmap (GPU memory)
    /// 5. HICON destroyed immediately after step 2-4
    /// 6. WIC intermediaries (IWICBitmap, IWICFormatConverter) released
    ///    when they go out of scope (COM Release via Drop)
    ///
    /// The only long-lived allocation is the ID2D1Bitmap in step 4.
    fn load_icon(&self, path: &str, rt: &ID2D1HwndRenderTarget) -> Option<ID2D1Bitmap> {
        let wic = self.wic_factory.as_ref()?;

        unsafe {
            let path_wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
            let mut shfi = SHFILEINFOW::default();

            let result = SHGetFileInfoW(
                windows::core::PCWSTR(path_wide.as_ptr()),
                windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL,
                Some(&mut shfi),
                std::mem::size_of::<SHFILEINFOW>() as u32,
                SHGFI_ICON | SHGFI_SMALLICON,
            );

            if result == 0 {
                return None;
            }

            let hicon = shfi.hIcon;
            if hicon.is_invalid() {
                return None;
            }

            // Convert HICON → D2D bitmap. HICON is destroyed regardless of result.
            let bitmap = hicon_to_d2d_bitmap(hicon, rt, wic);

            // Always destroy the HICON to prevent GDI handle leak.
            // This is safe even if hicon_to_d2d_bitmap returned None.
            let _ = windows::Win32::UI::WindowsAndMessaging::DestroyIcon(hicon);

            bitmap
        }
    }
}

/// Convert an HICON to an ID2D1Bitmap using WIC.
///
/// Intermediate COM objects (IWICBitmap, IWICFormatConverter) are released
/// automatically when they go out of scope at the end of this function.
#[cfg(windows)]
unsafe fn hicon_to_d2d_bitmap(
    hicon: windows::Win32::UI::WindowsAndMessaging::HICON,
    rt: &ID2D1HwndRenderTarget,
    wic: &IWICImagingFactory,
) -> Option<ID2D1Bitmap> {
    use windows::Win32::Graphics::Imaging::IWICFormatConverter;

    unsafe {
        // Step 1: HICON → IWICBitmap (system memory, pixel data copied from GDI)
        let wic_bitmap = wic.CreateBitmapFromHICON(hicon).ok()?;

        // Step 2: Convert pixel format to 32bpp pre-multiplied BGRA
        // This creates a temporary pipeline object, not a full copy.
        let converter: IWICFormatConverter = wic.CreateFormatConverter().ok()?;
        converter
            .Initialize(
                &wic_bitmap,
                &windows::Win32::Graphics::Imaging::GUID_WICPixelFormat32bppPBGRA,
                WICBitmapDitherTypeNone,
                None,
                0.0,
                WICBitmapPaletteTypeMedianCut,
            )
            .ok()?;

        // Step 3: Upload to GPU as ID2D1Bitmap
        // This copies pixel data to the render target's associated GPU.
        // The returned bitmap lives until its COM ref count drops to 0.
        let d2d_bitmap = rt.CreateBitmapFromWicBitmap(&converter, None).ok()?;

        // converter and wic_bitmap are released here (Drop → COM Release)
        Some(d2d_bitmap)
    }
}

/// Check if an extension requires per-path caching (unique icon per file).
#[cfg(windows)]
fn is_unique_icon_ext(ext: &str) -> bool {
    matches!(ext.to_lowercase().as_str(), ".exe" | ".lnk" | ".ico" | ".url")
}

/// Extract lowercase extension from a path (including the dot).
#[cfg(windows)]
fn extract_extension(path: &str) -> String {
    // Find last '.' that's after the last path separator
    let filename_start = path.rfind(['\\', '/']).map_or(0, |i| i + 1);
    let filename = &path[filename_start..];

    match filename.rfind('.') {
        Some(dot_pos) if dot_pos > 0 => filename[dot_pos..].to_lowercase(),
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
        // Explicitly clear caches to release all COM objects in a controlled order.
        // path_cache bitmaps are released first, then ext_cache, then wic_factory.
        self.path_cache.clear();
        self.path_lru.clear();
        self.ext_cache.clear();
        // wic_factory is dropped last (via struct field drop order)
    }
}
