// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! UWP/MSIX app enumeration via PowerShell fallback.
//!
//! Runs `Get-AppxPackage` and parses the JSON output to discover
//! installed UWP/MSIX applications.

use crate::{ProgramEntry, ProgramSource};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::process::Command;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Scan installed UWP/MSIX apps using PowerShell.
pub fn scan_uwp_apps() -> Vec<ProgramEntry> {
    let mut command = Command::new("powershell");
    command.args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            r#"Get-AppxPackage | Where-Object { $_.IsFramework -eq $false -and $_.SignatureKind -eq 'Store' } | Select-Object Name,InstallLocation | ConvertTo-Json -Compress"#,
        ]);

    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);

    let output = command.output();

    let Ok(output) = output else {
        return Vec::new();
    };

    if !output.status.success() {
        return Vec::new();
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_uwp_json(&stdout)
}

/// Parse the JSON output from Get-AppxPackage.
fn parse_uwp_json(json_str: &str) -> Vec<ProgramEntry> {
    let trimmed = json_str.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    // Output can be a single object or an array
    if let Ok(apps) = serde_json::from_str::<Vec<UwpAppRaw>>(trimmed) {
        return apps.into_iter().filter_map(raw_to_entry).collect();
    }

    // Single object case
    if let Ok(app) = serde_json::from_str::<UwpAppRaw>(trimmed) {
        return raw_to_entry(app).into_iter().collect();
    }

    Vec::new()
}

/// Raw JSON shape from PowerShell.
#[derive(serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
struct UwpAppRaw {
    name: Option<String>,
    install_location: Option<String>,
}

/// Convert a raw PowerShell entry into a `ProgramEntry`.
fn raw_to_entry(raw: UwpAppRaw) -> Option<ProgramEntry> {
    let name = raw.name?;

    // Skip framework/runtime packages that aren't real apps
    if is_framework_package(&name) {
        return None;
    }

    // Make the name more human-readable: take the last segment after dots
    let display_name = humanize_package_name(&name);
    let path = raw.install_location.unwrap_or_default();

    if display_name.is_empty() {
        return None;
    }

    Some(ProgramEntry {
        name: display_name,
        path,
        source: ProgramSource::Uwp,
    })
}

/// Filter out known framework/runtime packages.
fn is_framework_package(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("framework")
        || lower.contains(".net")
        || lower.contains("vclibs")
        || lower.contains("appinstaller")
        || lower.contains("windowsstore")
        || lower.contains("purchaseapp")
        || lower.contains("advertising")
        || lower.contains("extension")
        || lower.contains("inputapp")
}

/// Convert a package name like "Microsoft.WindowsCalculator" to "Windows Calculator".
fn humanize_package_name(name: &str) -> String {
    // Take everything after the first dot (publisher prefix)
    let meaningful = match name.find('.') {
        Some(idx) => &name[idx + 1..],
        None => name,
    };

    // Split on dots and camelCase boundaries, join with spaces
    let mut result = String::with_capacity(meaningful.len());
    for ch in meaningful.chars() {
        if ch == '.' || ch == '-' || ch == '_' {
            if !result.ends_with(' ') {
                result.push(' ');
            }
        } else if ch.is_uppercase() && !result.is_empty() && !result.ends_with(' ') {
            // CamelCase split
            result.push(' ');
            result.push(ch);
        } else {
            result.push(ch);
        }
    }

    result.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_humanize_package_name() {
        assert_eq!(
            humanize_package_name("Microsoft.WindowsCalculator"),
            "Windows Calculator"
        );
        assert_eq!(humanize_package_name("Microsoft.ZuneMusic"), "Zune Music");
        assert_eq!(humanize_package_name("Simple"), "Simple");
    }

    #[test]
    fn test_is_framework() {
        assert!(is_framework_package("Microsoft.NET.Native.Framework"));
        assert!(is_framework_package("Microsoft.VCLibs.140.00"));
        assert!(!is_framework_package("Microsoft.WindowsCalculator"));
    }

    #[test]
    fn test_parse_empty() {
        assert!(parse_uwp_json("").is_empty());
        assert!(parse_uwp_json("  ").is_empty());
    }

    #[test]
    fn test_parse_single() {
        let json = r#"{"Name":"Microsoft.WindowsCalculator","InstallLocation":"C:\\Program Files\\WindowsApps\\calc"}"#;
        let entries = parse_uwp_json(json);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "Windows Calculator");
    }

    #[test]
    fn test_parse_array() {
        let json = r#"[{"Name":"Microsoft.WindowsCalculator","InstallLocation":"C:\\calc"},{"Name":"Microsoft.ZuneMusic","InstallLocation":"C:\\music"}]"#;
        let entries = parse_uwp_json(json);
        assert_eq!(entries.len(), 2);
    }
}
