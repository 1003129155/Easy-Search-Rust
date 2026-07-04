// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Trigram key generation for basename search.

/// Generate overlapping trigram strings from folded text.
#[must_use]
pub fn trigrams(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    chars
        .windows(3)
        .map(|window| window.iter().collect())
        .collect()
}
