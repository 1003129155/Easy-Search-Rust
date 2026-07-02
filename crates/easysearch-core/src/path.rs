// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Path normalization helpers used by lookup and enumeration.

/// Normalize a user-supplied Windows path for index lookup.
///
/// Rules:
/// - Forward slashes converted to backslashes.
/// - `C:` (bare drive prefix) and `C:\` are both normalised to `C:\`.
/// - All other paths have trailing backslashes stripped.
/// - Drive letter is uppercased.
#[must_use]
pub fn normalize_path_for_lookup(path: &str) -> String {
    let replaced = path.replace('/', r"\");
    let trimmed = replaced.trim();
    // "C:\" or "C:/" → "C:\"
    if is_drive_root(trimmed) {
        return trimmed[..2].to_ascii_uppercase() + r"\";
    }
    // Bare "C:" → "C:\"
    if is_drive_prefix(trimmed) {
        return trimmed[..2].to_ascii_uppercase() + r"\";
    }
    // Strip trailing backslash for all other paths.
    let stripped = trimmed.trim_end_matches('\\');
    // Uppercase drive letter if present.
    if stripped.len() >= 2 && stripped.as_bytes()[1] == b':' {
        let drive = stripped[..1].to_ascii_uppercase();
        return drive + &stripped[1..];
    }
    stripped.to_string()
}

/// Returns `true` when `path` is shaped like `C:\` or `C:/`.
#[must_use]
pub fn is_drive_root(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() == 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
}

/// Returns `true` when `path` is shaped like `C:`.
#[must_use]
pub fn is_drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() == 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

#[cfg(test)]
mod tests {
    use super::normalize_path_for_lookup;

    #[test]
    fn root_gets_trailing_separator() {
        assert_eq!(normalize_path_for_lookup("c:/"), r"C:\");
    }

    #[test]
    fn non_root_trims_trailing_separator() {
        assert_eq!(normalize_path_for_lookup(r"C:\Users\"), r"C:\Users");
    }
}
