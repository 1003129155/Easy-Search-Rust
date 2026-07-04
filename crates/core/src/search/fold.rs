// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Query/name folding for case-insensitive matching.

/// Fold a query or basename for case-insensitive matching.
#[must_use]
pub fn fold_text(text: &str) -> String {
    text.to_lowercase()
}
