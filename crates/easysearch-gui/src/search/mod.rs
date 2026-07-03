// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Search window module — Win32 + Direct2D frontend.
//!
//! Contains the search window implementation including window creation,
//! rendering, input handling, layout, icon caching, preview, actions,
//! history tracking, and pinyin matching.

pub mod action;
pub mod history;
pub mod icon;
pub mod input;
pub mod layout;
pub mod pinyin;
pub mod preview;
pub mod renderer;
pub mod window;

// Re-export the main entry point so callers can use `search::run()`.
pub use window::run;
