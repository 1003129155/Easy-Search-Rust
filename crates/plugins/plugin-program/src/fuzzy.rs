// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Fuzzy matching with scored ranking.
//!
//! Scoring tiers:
//! - Exact match:     1000
//! - Prefix match:    800 + length bonus
//! - Word-start match: 600
//! - Contains:        400 + position bonus
//! - Initials match:  200
//! - No match:        0

/// Compute a fuzzy match score between `query` (lowercase) and `target` (lowercase).
/// Returns 0 if no match.
pub fn fuzzy_score(query: &str, target: &str) -> u32 {
    if query.is_empty() || target.is_empty() {
        return 0;
    }

    // Exact match
    if target == query {
        return 1000;
    }

    // Prefix match (target starts with query)
    if target.starts_with(query) {
        // Bonus for how much of the target is covered
        let coverage = (query.len() * 100 / target.len()) as u32;
        return 800 + coverage;
    }

    // Word-start match: query matches the start of a word boundary in target
    if word_start_match(query, target) {
        return 600;
    }

    // Contains match
    if let Some(pos) = target.find(query) {
        // Earlier position = better score
        let position_bonus = 100u32.saturating_sub(pos as u32 * 5);
        return 400 + position_bonus;
    }

    // Initials match: each char in query matches the first char of consecutive words
    if initials_match(query, target) {
        return 200;
    }

    // Subsequence match (weakest)
    if subsequence_match(query, target) {
        return 100;
    }

    0
}

/// Check if query matches at word boundaries in target.
/// E.g., "wc" matches "Windows Calculator" (W + C).
fn word_start_match(query: &str, target: &str) -> bool {
    let words: Vec<&str> = target
        .split(|c: char| c == ' ' || c == '-' || c == '_' || c == '.')
        .filter(|w| !w.is_empty())
        .collect();

    if words.is_empty() {
        return false;
    }

    // Check if the query matches the concatenation of word prefixes
    let mut query_chars = query.chars().peekable();
    for word in &words {
        for ch in word.chars() {
            match query_chars.peek() {
                Some(&qc) if qc == ch => {
                    query_chars.next();
                }
                Some(_) => break,    // move to next word
                None => return true, // consumed all query chars
            }
        }
    }

    query_chars.peek().is_none()
}

/// Check if each character in query matches the first character of words in target.
/// E.g., "np" matches "Notepad Plus" or "np++"
fn initials_match(query: &str, target: &str) -> bool {
    let initials: Vec<char> = target
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .filter_map(|w| w.chars().next())
        .collect();

    if query.len() > initials.len() {
        return false;
    }

    query.chars().zip(initials.iter()).all(|(q, &i)| q == i)
}

/// Check if query is a subsequence of target.
fn subsequence_match(query: &str, target: &str) -> bool {
    let mut target_chars = target.chars();
    for qc in query.chars() {
        loop {
            match target_chars.next() {
                Some(tc) if tc == qc => break,
                Some(_) => continue,
                None => return false,
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        assert_eq!(fuzzy_score("notepad", "notepad"), 1000);
    }

    #[test]
    fn test_prefix_match() {
        let score = fuzzy_score("note", "notepad");
        assert!(score >= 800 && score < 900);
    }

    #[test]
    fn test_contains_match() {
        let score = fuzzy_score("pad", "notepad");
        assert!(score >= 400 && score < 500);
    }

    #[test]
    fn test_initials_match() {
        let score = fuzzy_score("wc", "windows calculator");
        // Should match as word-start or initials
        assert!(score >= 200);
    }

    #[test]
    fn test_no_match() {
        assert_eq!(fuzzy_score("xyz", "notepad"), 0);
    }

    #[test]
    fn test_empty_query() {
        assert_eq!(fuzzy_score("", "notepad"), 0);
    }

    #[test]
    fn test_subsequence() {
        let score = fuzzy_score("ntp", "notepad");
        assert!(score > 0); // n-t-p is a subsequence of n-o-t-e-p-a-d
    }

    #[test]
    fn test_word_start() {
        let score = fuzzy_score("vs", "visual studio");
        assert!(score >= 600);
    }
}
