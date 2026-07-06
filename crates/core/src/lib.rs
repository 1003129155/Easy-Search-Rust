// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! EasySearch file-search core.
//!
//! This crate provides the in-memory index, search scoring, cache persistence,
//! and USN-driven delta overlay for fast Windows filename search.

pub mod builder;
pub mod cache;
pub mod cache_header;
pub mod delta;
pub mod error;
pub mod index;
pub mod path;
pub mod paths;
pub mod plugin;
pub mod record;
pub mod search;
pub mod status;
pub mod usn;

#[cfg(test)]
mod audit_tests;

pub use builder::EsIndexBuilder;
pub use error::{EsError, Result};
pub use index::{EsIndex, FileRefEntry, FileRefMap};
pub use plugin::{
    Action, CancelToken, ContextAction, ContextData, Plugin, PluginResult, PluginRouterInfo,
    Router, SettingControl, SettingItem, SystemCmd,
};
pub use record::{ES_RECORD_BYTES, EsRecord};
pub use search::{EsSearchIndex, EsSearchResult};
pub use status::{EsIndexState, EsIndexStatus};
