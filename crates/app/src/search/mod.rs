// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Search window module — Win32 + Direct2D frontend.
//!
//! Contains the search window implementation including window creation,
//! rendering, input handling, layout, icon caching, preview, actions,
//! history tracking, and pinyin matching.

pub mod action;
mod app_state;
pub mod clipboard;
pub mod context;
mod engine_bridge;
mod execution;
pub mod fs_actions;
pub mod history;
pub mod icon;
pub mod input;
mod key_command;
pub mod layout;
mod messages;
#[allow(dead_code)]
pub mod pinyin;
mod plugin_bridge;
#[allow(dead_code)]
pub mod preview;
mod render_bridge;
pub mod renderer;
mod search_flow;
mod settings_sync;
pub mod shell_context_menu;
mod visibility;
pub mod window;

// Re-export the main entry point so callers can use `search::run()`.
pub use window::run;
