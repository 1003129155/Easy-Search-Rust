# EasySearch 项目进度

> 最后更新: 2026-07-03

## 项目概述

EasySearch 是一个 Windows 本地文件搜索工具，目标对标 Everything/Flow Launcher，基于 Rust 构建。采用 GUI 内嵌搜索引擎 + 可选守护进程架构，搜索窗口使用 Win32 + Direct2D 原生渲染，设置窗口基于 iced。

---

## 架构总览

```
┌──────────────────────────────────────────────────────────────┐
│  easysearch-gui (主进程)                                      │
│  ┌─────────────────┐  ┌─────────────────┐  ┌──────────────┐ │
│  │ Search Window   │  │ Settings Window │  │ Welcome      │ │
│  │ (Win32+D2D)     │  │ (iced 0.13)    │  │ Wizard(iced) │ │
│  └────────┬────────┘  └────────────────┘  └──────────────┘ │
│           │ 内嵌 easysearch-engine (in-process)              │
│           │ + easysearch-plugins (同步插件路由)               │
└───────────┼──────────────────────────────────────────────────┘
            │
┌───────────▼──────────────────────────────────────────────────┐
│  easysearch-engine (嵌入式搜索引擎)                           │
│  ┌──────────────┐  ┌───────────────────┐  ┌──────────────┐ │
│  │ SearchEngine │  │ DriveManager      │  │ USN Poller   │ │
│  │ (public API) │  │ (multi-drive idx) │  │ (1s 间隔)    │ │
│  └──────────────┘  └───────────────────┘  └──────────────┘ │
│                          │                                   │
│                    ┌─────▼──────────────────────────────┐   │
│                    │       easysearch-core              │   │
│                    │  (Index / Cache / Search)          │   │
│                    └───────────────────────────────────-┘   │
└──────────────────────────────────────────────────────────────┘
                              │
              ┌───────────────┼───────────────┐
              ▼               ▼               ▼
       easysearch-mft   easysearch-security   uffs-polars/uffs-text
       (MFT 解析)        (管道安全/加密)       (数据处理/文本)

┌──────────────────────────────────────────────────────────────┐
│  easysearch (独立守护进程，可选)                                │
│  ┌───────────┐  ┌──────────────┐  ┌───────────────────────┐ │
│  │ IPC Server│  │ EsService    │  │ USN Journal Poller    │ │
│  │(Named Pipe│  │(handle_json) │  │ (1s 间隔)             │ │
│  └───────────┘  └──────────────┘  └───────────────────────┘ │
│  ※ 提供 CLI 客户端接口，GUI 目前不通过 IPC 连接守护进程         │
└──────────────────────────────────────────────────────────────┘
```

### 架构说明

GUI 主进程通过 `easysearch-engine` crate **直接内嵌搜索引擎**（in-process），而不是通过 IPC 连接独立守护进程。这意味着：

- GUI 启动时自行构建 MFT 索引、轮询 USN Journal
- 搜索请求走进程内通道（mpsc + PostMessage），延迟极低
- `easysearch`（守护进程二进制）仍然存在，可供 CLI 客户端使用
- `ipc_client.rs` 已实现完整的 Named Pipe 客户端，预留给未来 daemon 模式切换

---

## 已完成 ✅

### 基础设施
- [x] Workspace 搭建：9 个 crate，nightly-2026-06-26 工具链
- [x] 全 workspace `cargo check` 通过（仅 dead_code 警告）
- [x] `.cargo/config.toml` 配置 target-cpu=native 优化

### 搜索引擎 (easysearch-engine)
- [x] `SearchEngine` 公共 API：search / enumerate / add_drive / remove_drive / rebuild_drive
- [x] 后台索引构建：启动时扫描配置盘符的 MFT（线程 `engine-index`）
- [x] USN Journal 增量轮询（1s 间隔，线程内循环）
- [x] 事件通道：`EngineEvent` 推送 DriveIndexing / DriveReady / AllReady / UsnUpdate / Shutdown
- [x] 热插拔盘符：运行时 add_drive / remove_drive
- [x] 指标系统：搜索延迟、USN 事件计数、构建耗时
- [x] 结构化查询：`SearchQuery` 支持 filter / sort / limit / Everything 兼容模式
- [x] 查询规范化：`*.txt` → `.txt`、`>regex` → 子串提取

### 守护进程 (easysearch，可选独立运行)
- [x] Named Pipe IPC 服务端（NDJSON 协议，ACL 限制当前用户）
- [x] `EsService` 请求处理：status / search / enumerate / rebuild / shutdown
- [x] 后台索引构建：启动时扫描配置盘符的 MFT
- [x] USN Journal 增量轮询（1s 间隔）
- [x] 重复实例检测（pipe 已占用则退出）
- [x] CLI 客户端模式（参数触发 CLI 而非守护）
- [x] 非 Windows 平台回退 stdin/stdout 模式

### 核心 (easysearch-core)
- [x] Index 数据结构与构建器 (builder.rs)
- [x] Cache 序列化/反序列化 (cache.rs, cache_header.rs)
- [x] 搜索引擎：trigram / prefix / postings / case fold
- [x] Delta 增量更新
- [x] Record / Path 类型定义
- [x] USN cursor 管理

### MFT 解析 (easysearch-mft)
- [x] 完整 NTFS MFT 解析（从 UFFS 项目移植）
- [x] USN Journal 读取
- [x] 目录树重建
- [x] zstd 压缩缓存
- [x] IOCP 异步 IO
- [x] Polars DataFrame 缓存格式

### 安全 (easysearch-security)
- [x] Named Pipe ACL 权限控制（OwnerOnlySd, PipeName）
- [x] AES-GCM 加密 keystore
- [x] Authenticode 签名验证
- [x] 安全目录创建 (runtime_dir / log_dir)

### GUI — 搜索窗口 (Win32 + Direct2D)
- [x] Win32 窗口创建（WS_POPUP + WS_EX_TOOLWINDOW + WS_EX_TOPMOST）
- [x] Direct2D 渲染管线（factory → render target → brushes → text format）
- [x] DWM 圆角 + 阴影（Win11 DWMWCP_ROUND + ExtendFrameIntoClientArea）
- [x] 全局热键 Alt+Space 唤起/隐藏（WM_HOTKEY + RegisterHotKey）
- [x] 多显示器支持（MonitorFromPoint + 居中定位到活动屏幕）
- [x] 输入框：完整文本编辑（光标、选择、Home/End/Delete/Backspace）
- [x] 输入框：Ctrl+A/C/V/X 快捷键、Tab 自动补全
- [x] IME 支持：WM_IME_STARTCOMPOSITION / COMPOSITION / ENDCOMPOSITION
- [x] IME 窗口定位：CFS_FORCE_POSITION + CFS_EXCLUDE（修复 TSF IME 候选窗定位）
- [x] 搜索结果列表：动态窗口高度、上下键导航（循环）、选中高亮
- [x] 搜索结果高亮匹配字符：daemon 返回 highlight 区间，renderer 渲染
- [x] 图标缓存：从文件路径提取系统图标（SHGetFileInfoW）
- [x] 结果项动画：淡入动画（10帧 × 16ms = 160ms，对标 Flow.Launcher）
- [x] 防抖搜索：150ms debounce timer → 搜索线程（drain 到最新请求）
- [x] 序列号机制：current_search_seq 防止旧结果覆盖新请求
- [x] 失焦自动隐藏（WM_ACTIVATE → hide_window）
- [x] 系统主题变更检测（WM_SETTINGCHANGE → 重刷主题）
- [x] 索引状态指示：indexing 阶段显示 placeholder_indexing，就绪后显示 placeholder_ready

### GUI — 结果操作
- [x] Enter 打开文件/URL（ShellExecuteW "open"）
- [x] Ctrl+Enter 打开所在文件夹（explorer.exe /select,）
- [x] 使用频率记录（history.rs → history.json，用于排序提升）
- [x] 操作执行后自动隐藏窗口

### GUI — 托盘
- [x] 系统托盘图标（Shell_NotifyIconW）
- [x] 左键双击唤起搜索窗口
- [x] 右键菜单：Settings / Exit
- [x] Settings 菜单项打开设置窗口
- [x] Exit 菜单项销毁主窗口退出

### GUI — 设置窗口 (iced)
- [x] 独立线程运行（不阻塞搜索窗口）
- [x] 单例控制（SETTINGS_OPEN AtomicBool，不重复打开）
- [x] panic 隔离（catch_unwind，设置窗口崩溃不影响主窗口）
- [x] MVVM 架构：5 页独立 ViewModel + View
- [x] 左导航栏 240px + 右内容区
- [x] 页面：通用设置 / 插件管理 / 外观主题 / 快捷键设置 / 关于
- [x] Escape 关闭窗口
- [x] 最小窗口 940×600

### GUI — 欢迎向导
- [x] 首次运行检测（settings.json 不存在）
- [x] iced 向导窗口

### GUI — 主题 & 国际化
- [x] 主题引擎：JSON 主题文件、base 继承、系统深色模式检测
- [x] 内置主题：base / win11_light / win11_dark
- [x] I18n 引擎：JSON 语言资源、locale 匹配、回退链
- [x] 语言支持：en / zh-CN / ja
- [x] DPI 感知（Per-Monitor Aware V2）

### GUI — 设置持久化
- [x] JSON settings.json、原子写入
- [x] Settings 结构体含 index_drives / language / theme 等配置
- [x] 启动时从 settings.json 读取 index_drives 配置引擎

### GUI — IPC 客户端（预留）
- [x] Named Pipe 客户端完整实现（connect / search / disconnect）
- [x] NDJSON 请求/响应协议（DaemonRequest / DaemonResponse / DaemonItem）
- [x] 注：当前未使用，GUI 直接嵌入引擎。预留给 daemon 模式切换

### 插件系统 (easysearch-plugins)
- [x] 插件接口定义（Plugin trait + Router 分发）
- [x] 与搜索窗口集成：plugin 结果同步展示 → file search 结果异步追加
- [x] 计算器插件：完整递归下降解析器，支持 +−×÷^% 和 15 个函数
- [x] URL 检测插件：自动识别 http/https/ftp/域名，一键在浏览器打开
- [x] 网页搜索插件：15 个搜索引擎（g/bd/bi/gh/wiki/yt/so/ddg/maps/translate/npm/crates/zhihu/tb/jd）
- [x] 书签插件：读取 Chrome/Edge/Firefox 书签，`b ` 前缀搜索
- [x] 程序启动器插件：扫描 Start Menu .lnk/.exe，按名称匹配
- [x] 系统命令插件：关机/重启/锁定/睡眠/休眠/注销/清空回收站
- [x] Shell 命令插件：`> ` 前缀执行任意 cmd 命令
- [x] 进程管理插件：`kill ` 前缀搜索进程并 taskkill
- [x] Windows 设置插件：`s ` 前缀搜索 23 个 ms-settings: URI

### 拼音匹配
- [x] 拼音首字母匹配（pinyin.rs）

---

## 进行中 🔧

### 设置窗口 ↔ 搜索行为联动
- [x] 设置窗口修改 theme 后实时切换搜索窗口主题（SettingsChange::ThemeChanged → poll_settings_changes）
- [x] 设置窗口修改 hotkey 后重新注册全局热键（parse_hotkey_string → RegisterHotKey）
- [x] 设置窗口修改 language 后切换 i18n 文本（I18nEngine::set_locale）
- [x] 设置窗口修改 index_drives 后通知引擎 add_drive / remove_drive
- [x] 通用设置页数据实际持久化到 settings.json（SettingsApp 回写 Arc<RwLock<Settings>>）

### 搜索结果打磨
- [x] 搜索结果频率提升：history.boost_score 已接入搜索线程排序逻辑
- [x] 文件预览面板接入：preview.rs → renderer.render_preview，Up/Down 键触发更新

---

## 待开发 📋

### 阶段 1：设置联动 & 体验完善（当前重点）

| 优先级 | 任务 | 说明 |
|--------|------|------|
| ~~P0~~ | ~~设置变更实时生效~~ | ✅ theme/hotkey/language/drives 修改后无需重启 |
| ~~P0~~ | ~~通用设置页数据回写~~ | ✅ autostart / drives / language 保存到 settings.json |
| ~~P1~~ | ~~索引进度通知~~ | ✅ EngineEvent::DriveIndexing/Ready/Error → WM_ENGINE_EVENT → placeholder 展示 |
| ~~P1~~ | ~~错误处理 & 重试~~ | ✅ DriveError 事件时 GUI 展示错误提示（index_error 字段） |
| ~~P1~~ | ~~自启动管理接入~~ | ✅ autostart.rs 从设置页 AutostartToggled 触发 enable/disable |

### 阶段 2：功能完善

| 优先级 | 任务 | 说明 |
|--------|------|------|
| ~~P1~~ | ~~文件预览面板~~ | ✅ preview.rs → render_preview，Up/Down 键触发 |
| P1 | 搜索历史面板 | history.rs 已有持久化，需 UI 展示最近使用 |
| ~~P1~~ | ~~频率排序接入~~ | ✅ history.boost_score 注入搜索结果排序 |
| P2 | 自定义主题导入 | 用户 themes/ 目录加载自定义 JSON 主题 |
| P2 | 插件关键词自定义 | 设置页允许修改各插件的触发关键词 |

### 阶段 3：打磨 & 发布

| 优先级 | 任务 | 说明 |
|--------|------|------|
| P2 | 性能调优 | 搜索延迟 < 50ms，内存占用优化 |
| P2 | 安装包制作 | MSI 或便携版 zip |
| P3 | 自动更新 | 检查新版本并升级 |
| P3 | 正则表达式搜索 | 高级搜索模式（query 前缀 `>` 已做子串提取，需完整 regex 支持）|
| P3 | 文件内容搜索 | 全文检索（可选功能） |
| P3 | 清理 dead_code 警告 | 当前约 42 条，逐步接入或删除 |
| P3 | 守护进程模式切换 | GUI 可选通过 IPC 连接 daemon 而非内嵌引擎 |

---

## 已知问题

1. **iced 依赖冗余** — 设置窗口和欢迎向导使用 iced，拉入大量 wgpu/winit 依赖，编译时间较长。可考虑将设置窗口也迁移为 Win32 原生渲染。
2. **约 42 条 dead_code 警告** — GUI 模块中部分函数/结构体已定义但未接入调用路径（如 `show_window` 旧版本、`BookmarkPlugin::reload`）。
3. **设置窗口不回写** — ViewModel 数据修改后不持久化到 settings.json，也不通知搜索窗口。需要跨线程通信机制（可用 Arc<RwLock<Settings>> 轮询或 channel）。
4. **程序插件启动慢** — `ProgramPlugin::new()` 在构造时同步扫描 Start Menu，如果文件多会阻塞搜索窗口初始化。应改为异步加载。

---

## 代码统计（粗估）

| Crate | 源文件数 | 角色 |
|-------|---------|------|
| easysearch-core | 12+ | 索引 / 缓存 / 搜索算法 |
| easysearch-mft | 20+ | MFT/USN 底层解析 |
| easysearch-security | 8+ | 安全模块（ACL / AES-GCM / Authenticode） |
| easysearch (daemon) | 10 | 独立守护进程（可选） |
| easysearch-engine | 6 | 嵌入式搜索引擎（GUI 主要依赖） |
| easysearch-gui | 30+ | GUI 前端（搜索+设置+向导+托盘） |
| easysearch-plugins | 10 | 插件系统（9 个功能完整的内置插件） |
| uffs-polars | 1 | Polars 封装 |
| uffs-text | 2 | Unicode 处理 |

---

## 技术亮点

- **搜索延迟**：150ms debounce + 进程内 mpsc 通道 + drain-to-latest → 输入即搜体验
- **IME 兼容**：CFS_FORCE_POSITION + CFS_EXCLUDE 修复 TSF IME 候选窗定位问题
- **RefCell 安全**：Win32 消息可重入（WM_PAINT / IME），所有 Win32 API 调用在释放 borrow 后执行
- **动画**：结果列表 160ms 淡入（10帧 × 16ms），对标 Flow.Launcher 的 CircleEase
- **插件同步 + 搜索异步**：本地插件（计算器、URL）0ms 同步展示，文件搜索 150ms debounce 后异步追加
