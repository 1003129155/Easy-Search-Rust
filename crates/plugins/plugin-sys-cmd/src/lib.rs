// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! System commands plugin: shutdown, restart, lock, sleep, etc.

use easysearch_core::{Action, Plugin, PluginResult, SystemCmd};

pub struct SysCmdPlugin;

struct CmdEntry {
    names: &'static [&'static str],
    title: &'static str,
    subtitle: &'static str,
    cmd: SystemCmd,
}

const COMMANDS: &[CmdEntry] = &[
    CmdEntry {
        names: &["shutdown", "关机", "关闭"],
        title: "关机",
        subtitle: "关闭计算机",
        cmd: SystemCmd::Shutdown,
    },
    CmdEntry {
        names: &["restart", "reboot", "重启"],
        title: "重启",
        subtitle: "重启计算机",
        cmd: SystemCmd::Restart,
    },
    CmdEntry {
        names: &["lock", "锁定", "锁屏"],
        title: "锁定",
        subtitle: "锁定计算机",
        cmd: SystemCmd::Lock,
    },
    CmdEntry {
        names: &["sleep", "休眠", "睡眠"],
        title: "睡眠",
        subtitle: "进入睡眠模式",
        cmd: SystemCmd::Sleep,
    },
    CmdEntry {
        names: &["hibernate", "休眠模式"],
        title: "休眠",
        subtitle: "进入休眠模式",
        cmd: SystemCmd::Hibernate,
    },
    CmdEntry {
        names: &["logout", "logoff", "注销", "登出"],
        title: "注销",
        subtitle: "注销当前用户",
        cmd: SystemCmd::Logout,
    },
    CmdEntry {
        names: &["recycle", "回收站", "清空回收站", "emptyrecyclebin"],
        title: "清空回收站",
        subtitle: "永久删除回收站中的所有文件",
        cmd: SystemCmd::EmptyRecycleBin,
    },
];

impl Plugin for SysCmdPlugin {
    fn default_keyword(&self) -> Option<&str> {
        None // match by content
    }

    fn matches(&self, query: &str) -> bool {
        let q = query.trim().to_lowercase();
        COMMANDS
            .iter()
            .any(|c| c.names.iter().any(|n| n.contains(&q.as_str()) || q.contains(n)))
    }

    fn query(&self, query: &str) -> Vec<PluginResult> {
        let q = query.trim().to_lowercase();
        COMMANDS
            .iter()
            .filter(|c| c.names.iter().any(|n| n.contains(&q.as_str()) || q.contains(n)))
            .map(|c| PluginResult {
                title: c.title.to_string(),
                subtitle: c.subtitle.to_string(),
                icon: String::from("system"),
                action: Action::SystemCommand(c.cmd.clone()),
                score: 850,
            })
            .collect()
    }

    fn name(&self) -> &str {
        "SystemCommand"
    }
}
