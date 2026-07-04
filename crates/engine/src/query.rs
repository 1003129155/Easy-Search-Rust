// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Rich search query types with filtering, sorting, and Everything-compatible
//! pattern normalization.

/// A structured search query with optional filters and sort order.
#[derive(Debug, Clone)]
pub struct SearchQuery {
    /// The raw search pattern (filename substring, glob, etc.).
    pub pattern: String,
    /// Maximum number of results to return.
    pub limit: usize,
    /// Optional filter to narrow results.
    pub filter: SearchFilter,
    /// Sort order for results.
    pub sort: SortOrder,
}

impl SearchQuery {
    /// Create a simple query with defaults.
    #[must_use]
    pub fn new(pattern: impl Into<String>, limit: usize) -> Self {
        Self {
            pattern: pattern.into(),
            limit,
            filter: SearchFilter::default(),
            sort: SortOrder::Score,
        }
    }

    /// Builder: set the filter.
    #[must_use]
    pub fn with_filter(mut self, filter: SearchFilter) -> Self {
        self.filter = filter;
        self
    }

    /// Builder: set the sort order.
    #[must_use]
    pub fn with_sort(mut self, sort: SortOrder) -> Self {
        self.sort = sort;
        self
    }
}

/// Filter criteria applied after scoring.
#[derive(Debug, Clone, Default)]
pub struct SearchFilter {
    /// Only return results from these drives (uppercase letters).
    /// `None` means all loaded drives.
    pub drives: Option<Vec<char>>,
    /// Only return files matching these extensions (without leading dot).
    /// e.g. `["txt", "rs", "toml"]`
    pub extensions: Option<Vec<String>>,
    /// Bitwise flag mask: only return records whose flags contain all these bits.
    /// Use `record::flags::DIRECTORY` to search only directories.
    pub require_flags: Option<u16>,
    /// Bitwise flag mask: exclude records whose flags contain any of these bits.
    /// Use `record::flags::HIDDEN | record::flags::SYSTEM` to hide system files.
    pub exclude_flags: Option<u16>,
    /// Only files/dirs under this path prefix (case-insensitive).
    pub path_prefix: Option<String>,
}

impl SearchFilter {
    /// Returns `true` if the filter is empty (no restrictions).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.drives.is_none()
            && self.extensions.is_none()
            && self.require_flags.is_none()
            && self.exclude_flags.is_none()
            && self.path_prefix.is_none()
    }

    /// Check whether a result passes this filter.
    #[must_use]
    pub fn matches(&self, path: &str, name: &str, is_directory: bool, flags: u16) -> bool {
        // Drive filter
        if let Some(ref drives) = self.drives {
            let first_char = path.chars().next().map(|c| c.to_ascii_uppercase());
            if let Some(drive_char) = first_char {
                if !drives.contains(&drive_char) {
                    return false;
                }
            } else {
                return false;
            }
        }

        // Extension filter (only applies to files)
        if !is_directory {
            if let Some(ref exts) = self.extensions {
                let has_match = name
                    .rsplit_once('.')
                    .map(|(_, ext)| exts.iter().any(|e| e.eq_ignore_ascii_case(ext)))
                    .unwrap_or(false);
                if !has_match {
                    return false;
                }
            }
        }

        // Require flags
        if let Some(required) = self.require_flags {
            if flags & required != required {
                return false;
            }
        }

        // Exclude flags
        if let Some(excluded) = self.exclude_flags {
            if flags & excluded != 0 {
                return false;
            }
        }

        // Path prefix
        if let Some(ref prefix) = self.path_prefix {
            if !path
                .to_lowercase()
                .starts_with(&prefix.to_lowercase())
            {
                return false;
            }
        }

        true
    }

    /// Convenience: filter for directories only.
    #[must_use]
    pub fn directories_only() -> Self {
        Self {
            require_flags: Some(easysearch_core::record::flags::DIRECTORY),
            ..Default::default()
        }
    }

    /// Convenience: filter for files only (exclude directories).
    #[must_use]
    pub fn files_only() -> Self {
        Self {
            exclude_flags: Some(easysearch_core::record::flags::DIRECTORY),
            ..Default::default()
        }
    }

    /// Convenience: exclude hidden and system files.
    #[must_use]
    pub fn no_hidden_system() -> Self {
        Self {
            exclude_flags: Some(
                easysearch_core::record::flags::HIDDEN
                    | easysearch_core::record::flags::SYSTEM,
            ),
            ..Default::default()
        }
    }
}

/// Sort order for search results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortOrder {
    /// Sort by match score (descending). Default.
    #[default]
    Score,
    /// Sort by filename (ascending, case-insensitive).
    Name,
    /// Sort by full path (ascending, case-insensitive).
    Path,
}

/// Normalize a raw query pattern (Everything-compatible) into a substring for
/// the core search engine.
///
/// - `*.txt`     → `.txt` (extension match)
/// - `readme`    → `readme` (substring match, as-is)
/// - `>regex`    → `regex` (regex prefix stripped, used as substring)
/// - `*`         → `` (match all)
/// - `foo*.bar`  → `foo` (longest non-wildcard segment)
#[must_use]
pub fn normalize_query(raw: &str) -> String {
    let trimmed = raw.trim();

    if trimmed.is_empty() || trimmed == "*" {
        return String::new();
    }

    // Regex prefix ">" → strip and extract useful substring
    if let Some(rest) = trimmed.strip_prefix('>') {
        let cleaned = rest
            .trim_start_matches('\\')
            .trim_start_matches('.')
            .trim_end_matches('$');
        if !cleaned.is_empty() {
            return format!(".{cleaned}");
        }
        return String::new();
    }

    // `*.ext` pattern → `.ext`
    if let Some(ext) = trimmed.strip_prefix("*.") {
        if !ext.contains('*') && !ext.contains('?') {
            return format!(".{ext}");
        }
    }

    // Strip leading/trailing wildcards
    let s = trimmed.trim_start_matches('*').trim_end_matches('*');

    // If there are internal wildcards, take the longest non-wildcard segment
    if s.contains('*') || s.contains('?') {
        let parts: Vec<&str> = s.split(&['*', '?'][..]).filter(|p| !p.is_empty()).collect();
        return parts
            .iter()
            .max_by_key(|p| p.len())
            .unwrap_or(&"")
            .to_string();
    }

    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_basic() {
        assert_eq!(normalize_query("readme"), "readme");
        assert_eq!(normalize_query("*.txt"), ".txt");
        assert_eq!(normalize_query("*"), "");
        assert_eq!(normalize_query(""), "");
        assert_eq!(normalize_query("  *.rs  "), ".rs");
    }

    #[test]
    fn normalize_glob_middle() {
        assert_eq!(normalize_query("foo*.bar"), "foo");
    }

    #[test]
    fn normalize_regex_prefix() {
        assert_eq!(normalize_query(">\\.log$"), ".log");
    }

    #[test]
    fn filter_extension() {
        let filter = SearchFilter {
            extensions: Some(vec!["txt".to_string(), "rs".to_string()]),
            ..Default::default()
        };
        assert!(filter.matches(r"C:\foo.txt", "foo.txt", false, 0));
        assert!(filter.matches(r"C:\bar.rs", "bar.rs", false, 0));
        assert!(!filter.matches(r"C:\baz.py", "baz.py", false, 0));
        // Directories bypass extension filter
        assert!(filter.matches(r"C:\src", "src", true, 0x10));
    }

    #[test]
    fn filter_drives() {
        let filter = SearchFilter {
            drives: Some(vec!['C', 'D']),
            ..Default::default()
        };
        assert!(filter.matches(r"C:\foo", "foo", false, 0));
        assert!(filter.matches(r"D:\bar", "bar", false, 0));
        assert!(!filter.matches(r"E:\baz", "baz", false, 0));
    }
}
