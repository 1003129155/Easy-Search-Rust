// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Windows Settings plugin: open system settings pages with `s ` prefix.

use easysearch_core::{Action, Plugin, PluginResult};

pub struct WinSettingsPlugin;

struct SettingsEntry {
    names: &'static [&'static str],
    title: &'static str,
    uri: &'static str,
}

const SETTINGS: &[SettingsEntry] = &[
    SettingsEntry { names: &["display", "显示", "屏幕"], title: "显示设置", uri: "ms-settings:display" },
    SettingsEntry { names: &["sound", "声音", "音量"], title: "声音设置", uri: "ms-settings:sound" },
    SettingsEntry { names: &["bluetooth", "蓝牙"], title: "蓝牙设置", uri: "ms-settings:bluetooth" },
    SettingsEntry { names: &["wifi", "网络", "network", "wlan"], title: "网络和 Internet", uri: "ms-settings:network" },
    SettingsEntry { names: &["vpn"], title: "VPN 设置", uri: "ms-settings:network-vpn" },
    SettingsEntry { names: &["apps", "应用", "程序"], title: "应用和功能", uri: "ms-settings:appsfeatures" },
    SettingsEntry { names: &["default", "默认应用", "默认"], title: "默认应用", uri: "ms-settings:defaultapps" },
    SettingsEntry { names: &["wallpaper", "壁纸", "背景", "桌面"], title: "桌面背景", uri: "ms-settings:personalization-background" },
    SettingsEntry { names: &["theme", "主题"], title: "主题设置", uri: "ms-settings:themes" },
    SettingsEntry { names: &["color", "颜色", "accent"], title: "颜色设置", uri: "ms-settings:personalization-colors" },
    SettingsEntry { names: &["lock", "锁屏"], title: "锁屏设置", uri: "ms-settings:lockscreen" },
    SettingsEntry { names: &["storage", "存储"], title: "存储设置", uri: "ms-settings:storagesense" },
    SettingsEntry { names: &["power", "电源", "电池", "battery"], title: "电源设置", uri: "ms-settings:powersleep" },
    SettingsEntry { names: &["keyboard", "键盘", "输入法"], title: "键盘设置", uri: "ms-settings:keyboard" },
    SettingsEntry { names: &["mouse", "鼠标"], title: "鼠标设置", uri: "ms-settings:mousetouchpad" },
    SettingsEntry { names: &["time", "时间", "日期", "date"], title: "日期和时间", uri: "ms-settings:dateandtime" },
    SettingsEntry { names: &["language", "语言", "region", "地区"], title: "语言和地区", uri: "ms-settings:regionlanguage" },
    SettingsEntry { names: &["update", "更新", "windows update"], title: "Windows Update", uri: "ms-settings:windowsupdate" },
    SettingsEntry { names: &["privacy", "隐私"], title: "隐私设置", uri: "ms-settings:privacy" },
    SettingsEntry { names: &["about", "关于", "系统信息"], title: "关于此电脑", uri: "ms-settings:about" },
    SettingsEntry { names: &["proxy", "代理"], title: "代理设置", uri: "ms-settings:network-proxy" },
    SettingsEntry { names: &["startup", "启动", "开机"], title: "启动应用", uri: "ms-settings:startupapps" },
    SettingsEntry { names: &["notification", "通知"], title: "通知设置", uri: "ms-settings:notifications" },
];

impl Plugin for WinSettingsPlugin {
    fn default_keyword(&self) -> Option<&str> {
        Some("s ")
    }

    fn query(&self, query: &str) -> Vec<PluginResult> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return SETTINGS
                .iter()
                .take(8)
                .map(|s| setting_to_result(s))
                .collect();
        }

        SETTINGS
            .iter()
            .filter(|s| s.names.iter().any(|n| n.contains(&q.as_str()) || q.contains(n)))
            .take(8)
            .map(|s| setting_to_result(s))
            .collect()
    }

    fn name(&self) -> &str {
        "WindowsSettings"
    }
}

fn setting_to_result(s: &SettingsEntry) -> PluginResult {
    PluginResult {
        title: s.title.to_string(),
        subtitle: s.uri.to_string(),
        icon: String::from("settings"),
        action: Action::Open(s.uri.to_string()),
        score: 750,
    }
}
