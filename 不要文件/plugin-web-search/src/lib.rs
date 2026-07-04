// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Web search plugin: constructs search URLs for various engines.

use easysearch_core::{Action, Plugin, PluginResult};

pub struct WebSearchPlugin {
    engines: Vec<SearchEngine>,
}

struct SearchEngine {
    keyword: &'static str,
    name: &'static str,
    url_template: &'static str,
}

impl Default for WebSearchPlugin {
    fn default() -> Self {
        Self {
            engines: vec![
                SearchEngine { keyword: "g ", name: "Google", url_template: "https://www.google.com/search?q={q}" },
                SearchEngine { keyword: "bd ", name: "百度", url_template: "https://www.baidu.com/s?wd={q}" },
                SearchEngine { keyword: "bi ", name: "Bing", url_template: "https://www.bing.com/search?q={q}" },
                SearchEngine { keyword: "gh ", name: "GitHub", url_template: "https://github.com/search?q={q}" },
                SearchEngine { keyword: "wiki ", name: "Wikipedia", url_template: "https://en.wikipedia.org/wiki/{q}" },
                SearchEngine { keyword: "yt ", name: "YouTube", url_template: "https://www.youtube.com/results?search_query={q}" },
                SearchEngine { keyword: "so ", name: "Stack Overflow", url_template: "https://stackoverflow.com/search?q={q}" },
                SearchEngine { keyword: "ddg ", name: "DuckDuckGo", url_template: "https://duckduckgo.com/?q={q}" },
                SearchEngine { keyword: "maps ", name: "Google Maps", url_template: "https://maps.google.com/maps?q={q}" },
                SearchEngine { keyword: "translate ", name: "Google Translate", url_template: "https://translate.google.com/#auto|zh-CN|{q}" },
                SearchEngine { keyword: "npm ", name: "npm", url_template: "https://www.npmjs.com/search?q={q}" },
                SearchEngine { keyword: "crates ", name: "crates.io", url_template: "https://crates.io/search?q={q}" },
                SearchEngine { keyword: "zhihu ", name: "知乎", url_template: "https://www.zhihu.com/search?type=content&q={q}" },
                SearchEngine { keyword: "tb ", name: "淘宝", url_template: "https://s.taobao.com/search?q={q}" },
                SearchEngine { keyword: "jd ", name: "京东", url_template: "https://search.jd.com/Search?keyword={q}" },
            ],
        }
    }
}

impl Plugin for WebSearchPlugin {
    fn default_keyword(&self) -> Option<&str> {
        None // multiple keywords, use matches()
    }

    fn matches(&self, query: &str) -> bool {
        let q = query.trim();
        self.engines.iter().any(|e| q.starts_with(e.keyword))
    }

    fn query(&self, query: &str) -> Vec<PluginResult> {
        let q = query.trim();

        for engine in &self.engines {
            if let Some(search_term) = q.strip_prefix(engine.keyword) {
                let search_term = search_term.trim();
                if search_term.is_empty() {
                    return vec![PluginResult {
                        title: format!("用 {} 搜索...", engine.name),
                        subtitle: String::from("输入搜索内容"),
                        icon: String::from("web_search"),
                        action: Action::None,
                        score: 800,
                    }];
                }
                let url = engine
                    .url_template
                    .replace("{q}", &encode_uri_component(search_term));
                return vec![PluginResult {
                    title: search_term.to_string(),
                    subtitle: format!("用 {} 搜索", engine.name),
                    icon: String::from("web_search"),
                    action: Action::Open(url),
                    score: 900,
                }];
            }
        }

        Vec::new()
    }

    fn name(&self) -> &str {
        "WebSearch"
    }
}

/// Percent-encode a string for use in URLs (RFC 3986).
fn encode_uri_component(s: &str) -> String {
    let mut result = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(b as char);
            }
            b' ' => result.push_str("%20"),
            _ => {
                result.push('%');
                result.push(char::from(HEX_CHARS[(b >> 4) as usize]));
                result.push(char::from(HEX_CHARS[(b & 0x0F) as usize]));
            }
        }
    }
    result
}

const HEX_CHARS: &[u8; 16] = b"0123456789ABCDEF";
