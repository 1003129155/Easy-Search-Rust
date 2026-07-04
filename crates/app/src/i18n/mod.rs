// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Internationalization engine — multi-language support with JSON locale files.
//!
//! The `engine` module provides the new JSON-based I18nEngine.
//! Legacy types are kept for backward compatibility during the transition.

pub mod engine;

// Re-export types used by search module (legacy bridge — kept for reference)
#[allow(dead_code)]
mod legacy;
#[allow(unused_imports)]
pub use legacy::I18n;
