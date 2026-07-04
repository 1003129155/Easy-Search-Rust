// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Error type for `easysearch`.

use thiserror::Error;

/// Errors that can terminate `easysearch`.
#[derive(Debug, Error)]
pub(crate) enum EsError {
    /// Standard I/O failed while reading or writing NDJSON.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization failed.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
