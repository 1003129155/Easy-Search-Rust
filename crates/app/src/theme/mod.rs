// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Theme engine — JSON-based theme loading, parsing, and switching.
//!
//! The `types` module defines the new type system (Color, ThemeMeta, ThemeColors, Theme)
//! with serde support for JSON serialization.
//!
//! Legacy types are re-exported for backward compatibility with the search module.

#[allow(dead_code)]
pub mod engine;
#[allow(dead_code)]
pub mod types;

// Re-export types used by search module (backward compat)
#[allow(dead_code)]
mod legacy;
pub use legacy::{Color, Theme};
