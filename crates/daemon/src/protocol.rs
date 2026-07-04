// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! NDJSON protocol used between clients and `easysearch`.

use serde::{Deserialize, Serialize};
use easysearch_core::EsSearchResult;

/// Request sent by the client.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Request {
    /// Client-chosen request id echoed in the response.
    pub(crate) id: u64,
    /// Method name: `status`, `search`, `enumerate`, `rebuild`, or `shutdown`.
    pub(crate) method: String,
    /// Filename query for `search` and `enumerate`.
    pub(crate) query: Option<String>,
    /// Maximum number of results.
    pub(crate) limit: Option<usize>,
    /// Directory path for `enumerate`.
    pub(crate) path: Option<String>,
    /// Whether `enumerate` should recurse.
    pub(crate) recursive: Option<bool>,
    /// Drive label for `rebuild`.
    pub(crate) drive: Option<String>,
}

/// Response returned to the client.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Response {
    /// Request id from the client.
    pub(crate) id: u64,
    /// Whether the request succeeded.
    pub(crate) ok: bool,
    /// Whether all loaded indexes are query-ready.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) ready: Option<bool>,
    /// Loaded drives.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) drives: Option<Vec<String>>,
    /// Total records across all loaded drives.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) records: Option<u64>,
    /// Whether indexing is in progress.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) indexing: Option<bool>,
    /// Last applied USN.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) last_usn: Option<i64>,
    /// Search or enumeration results.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) items: Option<Vec<ResponseItem>>,
    /// Rebuild request acknowledgement.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) started: Option<bool>,
    /// Error text for failed requests.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
}

impl Response {
    /// Build an error response.
    #[must_use]
    pub(crate) fn error(id: u64, message: impl Into<String>) -> Self {
        Self {
            id,
            ok: false,
            ready: None,
            drives: None,
            records: None,
            indexing: None,
            last_usn: None,
            items: None,
            started: None,
            error: Some(message.into()),
        }
    }
}

/// Slim item returned by `search` and `enumerate`.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ResponseItem {
    /// Full path.
    pub(crate) path: String,
    /// Basename.
    pub(crate) name: String,
    /// Whether this item is a directory.
    pub(crate) is_directory: bool,
    /// Ranking score.
    pub(crate) score: u32,
    /// Highlight ranges as `[start, len]`.
    pub(crate) highlight: Vec<[u32; 2]>,
}

impl From<EsSearchResult> for ResponseItem {
    fn from(result: EsSearchResult) -> Self {
        Self {
            path: result.path,
            name: result.name,
            is_directory: result.is_directory,
            score: result.score,
            highlight: result.highlight,
        }
    }
}
