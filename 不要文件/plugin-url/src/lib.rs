// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! URL plugin: detects and opens URLs.

use easysearch_core::{Action, Plugin, PluginResult};

pub struct UrlPlugin;

impl Plugin for UrlPlugin {
    fn default_keyword(&self) -> Option<&str> {
        None // auto-detect URLs
    }

    fn matches(&self, query: &str) -> bool {
        let q = query.trim().to_lowercase();
        q.starts_with("http://")
            || q.starts_with("https://")
            || q.starts_with("ftp://")
            || (q.contains('.') && !q.contains(' ') && looks_like_domain(&q))
    }

    fn query(&self, query: &str) -> Vec<PluginResult> {
        let q = query.trim();
        let url = if q.starts_with("http://") || q.starts_with("https://") || q.starts_with("ftp://") {
            q.to_string()
        } else {
            format!("https://{q}")
        };

        vec![PluginResult {
            title: format!("打开 {q}"),
            subtitle: String::from("在浏览器中打开"),
            icon: String::from("url"),
            action: Action::Open(url),
            score: 900,
        }]
    }

    fn name(&self) -> &str {
        "URL"
    }
}

fn looks_like_domain(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() < 2 {
        return false;
    }
    let tld = parts.last().unwrap();
    tld.len() >= 2 && tld.len() <= 10 && tld.chars().all(|c| c.is_ascii_alphabetic())
}
