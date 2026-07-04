# EasySearch

Windows 本地文件搜索工具，对标 Everything / Flow Launcher。基于 Rust 构建。

## 产物

编译后输出**一个 exe**（`easysearch.exe`），前后端一体化：

- 搜索窗口：Win32 + Direct2D 原生渲染
- 搜索引擎：进程内嵌入，MFT 解析 + USN 增量更新
- 设置窗口：iced 框架（`--settings` 子进程模式启动）
- 插件系统：11 个内置插件（计算器、URL、网页搜索、书签、程序启动器等）
- 系统托盘：Alt+Space 全局热键唤起

```
cargo build --release -p easysearch
→ target/release/easysearch.exe  (单文件，即开即用)
```

## 架构

```
easysearch.exe (单体进程)
├── 搜索窗口 (Win32 + Direct2D)
├── 搜索引擎 (easysearch-engine, 进程内嵌入)
│   ├── MFT 索引构建 (easysearch-mft)
│   ├── USN Journal 增量轮询
│   └── 插件路由 (plugins/*)
├── 设置窗口 (iced, --settings 子进程)
├── 欢迎向导 (iced, 首次启动)
└── 系统托盘
```

## Crate 说明

| Crate | 类型 | 说明 |
|-------|------|------|
| `easysearch` | bin | **主程序**（前后端一体化单 exe） |
| `easysearch-daemon` | bin | 独立守护进程（可选，供 CLI/外部客户端通过 Named Pipe 连接） |
| `easysearch-core` | lib | 索引数据结构、缓存、搜索算法 |
| `easysearch-engine` | lib | 嵌入式搜索引擎（多盘符管理、USN 轮询、事件通道） |
| `easysearch-mft` | lib | NTFS MFT/USN 底层解析 |
| `easysearch-security` | lib | Named Pipe ACL、AES-GCM 加密、Authenticode 验证 |
| `uffs-polars` | lib | Polars 封装（easysearch-mft 内部依赖，不在 workspace 成员中） |
| `uffs-text` | lib | Unicode 大小写折叠（同上，保留原命名避免改动 6000+ 行代码） |
| `plugins/*` | lib | 11 个内置插件 |

## 构建

```bash
# 需要 nightly 工具链 (rust-toolchain.toml 已配置)
# 需要 sccache (.cargo/config.toml 已配置)
cargo build --release -p easysearch
```

## 开发

```bash
# 只编译主程序
cargo build -p easysearch

# 只编译守护进程（CLI 调试用）
cargo build -p easysearch-daemon

# 检查全 workspace
cargo check
```
