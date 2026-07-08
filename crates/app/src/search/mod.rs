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
mod render_bridge;
mod search_flow;
mod settings_sync;
mod visibility;
#[allow(dead_code)]
pub mod preview;
pub mod renderer;
pub mod shell_context_menu;
pub mod window;

// Re-export the main entry point so callers can use `search::run()`.
pub use window::run;
